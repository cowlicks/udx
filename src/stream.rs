use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};

use tokio::sync::Mutex;

use crate::{
    cirbuf::CircularBuffer,
    congestion::CongestionControl,
    constants::{
        CONG_INIT_CWND, CONG_MAX_CWND, DEFAULT_BUFFER_SIZE, HEADER_SIZE, MAGIC_BYTE, MTU_BASE,
        MTU_STEP, STREAM_CLOSED, STREAM_CONNECTED, STREAM_ENDING, VERSION,
    },
    error::Result,
    packet::{Packet, PacketHeader},
    queue::Queue,
    UdxError, UdxSocket, UDX_HEADER_DATA, UDX_HEADER_DESTROY, UDX_HEADER_END,
};

#[derive(Debug, Clone)]
pub struct UdxStream {
    inner: Arc<Mutex<UdxStreamInner>>,
}

struct UdxStreamInner {
    // Stream identification
    local_id: u32,
    remote_id: u32,
    socket: Option<Arc<UdxSocket>>,
    remote_addr: Option<SocketAddr>,

    // Stream state
    state: StreamState,
    status: u32,
    write_wanted: u32,

    // Flow control
    seq: u32,
    ack: u32,
    remote_acked: u32,
    remote_ended: u32,
    send_window: u32,
    recv_window: u32,

    // RTT tracking
    srtt: u32,
    rttvar: u32,
    rto: u32,

    // Congestion control
    congestion: CongestionControl,
    cwnd: u32,
    ssthresh: u32,
    inflight: usize,

    // Packet management
    outgoing: CircularBuffer<Packet>,
    incoming: CircularBuffer<Packet>,
    inflight_queue: Queue<Packet>,
    retransmit_queue: Queue<Packet>,
    write_queue: Queue<WriteBuffer>,

    // MTU discovery
    mtu: u16,
    mtu_probe_wanted: bool,
    mtu_probe_size: u16,
    mtu_probe_count: u8,
    mtu_max: u16,
    mtu_state: MtuState,

    // Statistics
    stats: StreamStats,

    read_callback: Option<Box<dyn Fn(&[u8]) + Send + Sync>>,
    close_callback: Option<Box<dyn Fn(Option<UdxError>) + Send + Sync>>,
    write_callbacks: HashMap<u32, Box<dyn Fn(Result<()>) + Send + Sync>>,
    cwnd_cnt: u32,
    send_wl1: u32,
    send_wl2: u32,
}

impl std::fmt::Debug for UdxStreamInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl UdxStream {
    pub fn new(local_id: u32) -> Self {
        let inner = UdxStreamInner {
            local_id,
            remote_id: 0,
            socket: None,
            remote_addr: None,

            state: StreamState::default(),
            status: 0,
            write_wanted: 0,

            seq: 0,
            ack: 0,
            remote_acked: 0,
            remote_ended: 0,
            send_window: DEFAULT_BUFFER_SIZE as u32,
            recv_window: DEFAULT_BUFFER_SIZE as u32,

            srtt: 0,
            rttvar: 0,
            rto: 1000,

            congestion: CongestionControl::new(),
            cwnd: CONG_INIT_CWND,
            ssthresh: 255,
            inflight: 0,

            outgoing: CircularBuffer::new(16),
            incoming: CircularBuffer::new(16),
            inflight_queue: Queue::new(),
            retransmit_queue: Queue::new(),
            write_queue: Queue::new(),

            mtu: MTU_BASE,
            mtu_probe_size: MTU_BASE,
            mtu_probe_count: 0,
            mtu_state: MtuState::Base,

            stats: StreamStats::default(),

            cwnd_cnt: 0,
            mtu_max: 0,
            close_callback: None,
            read_callback: None,
            write_callbacks: HashMap::new(),
            mtu_probe_wanted: todo!(),
            send_wl1: todo!(),
            send_wl2: todo!(),
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub async fn connect(
        &self,
        socket: Arc<UdxSocket>,
        remote_id: u32,
        addr: SocketAddr,
    ) -> Result<()> {
        let mut inner = self.inner.lock().await;

        if inner.state.connected {
            return Err(UdxError::InvalidState);
        }

        inner.remote_id = remote_id;
        inner.remote_addr = Some(addr);
        inner.socket = Some(socket);
        inner.state.connected = true;
        inner.status |= STREAM_CONNECTED;

        Ok(())
    }

    pub async fn send(&self, data: &[u8]) -> Result<()> {
        let (seq, packet) = {
            let inner = self.inner.lock().await;

            if !inner.state.connected {
                return Err(UdxError::NotConnected);
            }

            if inner.state.local_ending {
                return Err(UdxError::Closed);
            }

            let packet = Packet::new(
                UDX_HEADER_DATA,
                inner.remote_id,
                inner.seq,
                inner.ack,
                inner.recv_window,
            );
            (inner.seq, packet)
        };

        self.inner.lock().await.outgoing.set(seq, packet);
        self.inner.lock().await.seq += 1;

        let write_buf = WriteBuffer {
            data: data.to_vec(),
            offset: 0,
            acked: 0,
            inflight: 0,
        };
        self.inner.lock().await.write_queue.push_back(write_buf);

        // Start transmission if possible
        let mut inner = self.inner.lock().await;
        self.try_transmit(&mut inner);

        Ok(())
    }

    fn try_transmit(&self, inner: &mut UdxStreamInner) -> Result<()> {
        // Check if we can send more data based on congestion and flow control
        let window = std::cmp::min(inner.cwnd, inner.send_window / inner.mtu as u32);

        while inner.inflight < window as usize {
            if let Some(mut packet) = self.get_next_packet(inner)? {
                if let Some(socket) = &inner.socket {
                    if let Some(addr) = inner.remote_addr {
                        packet.time_sent = Instant::now();
                        packet.transmits += 1;
                        inner.inflight += 1;
                        inner.stats.packets_tx += 1;
                        inner.stats.bytes_tx += packet.size as u64;

                        // Move to inflight queue
                        inner.inflight_queue.push_back(packet.clone());

                        // Actual send happens asynchronously
                        let socket = socket.clone();
                        let packet_clone = packet.clone();
                        tokio::spawn(async move { socket.send(packet_clone, addr).await });
                    }
                }
            } else {
                break;
            }
        }

        Ok(())
    }

    fn get_next_packet(&self, inner: &mut UdxStreamInner) -> Result<Option<Packet>> {
        // First check retransmission queue
        if let Some(packet) = inner.retransmit_queue.pop_front() {
            return Ok(Some(packet));
        }

        // Then check write queue
        if let Some(buf) = inner.write_queue.pop_front() {
            let remaining = buf.data.len() - buf.offset;
            let chunk_size = std::cmp::min(remaining, inner.mtu as usize - HEADER_SIZE);

            let mut packet = Packet::new(
                UDX_HEADER_DATA,
                inner.remote_id,
                inner.seq,
                inner.ack,
                inner.recv_window,
            );

            packet.data = buf.data[buf.offset..buf.offset + chunk_size].to_vec();
            packet.size = (HEADER_SIZE + chunk_size) as u16;

            // If not complete, push back to queue
            if buf.offset + chunk_size < buf.data.len() {
                let mut new_buf = buf;
                new_buf.offset += chunk_size;
                inner.write_queue.push_front(new_buf);
            }

            inner.seq += 1;
            return Ok(Some(packet));
        }

        Ok(None)
    }

    pub fn process_ack(&self, inner: &mut UdxStreamInner, ack: u32) -> Result<()> {
        while inner.remote_acked < ack {
            if let Some(packet) = inner.outgoing.remove(inner.remote_acked) {
                self.handle_ack(inner, packet)?;
            }
            inner.remote_acked += 1;
        }

        Ok(())
    }

    fn handle_ack(&self, inner: &mut UdxStreamInner, packet: Packet) -> Result<()> {
        let rtt = packet.time_sent.elapsed().as_millis() as u32;

        // Update RTT estimates
        if inner.srtt == 0 {
            inner.srtt = rtt;
            inner.rttvar = rtt / 2;
        } else {
            let delta = if rtt > inner.srtt {
                rtt - inner.srtt
            } else {
                inner.srtt - rtt
            };
            inner.rttvar = (3 * inner.rttvar + delta) / 4;
            inner.srtt = (7 * inner.srtt + rtt) / 8;
        }

        // Update RTO
        inner.rto = inner.srtt + 4 * inner.rttvar;
        if inner.rto < 1000 {
            inner.rto = 1000;
        }

        inner.inflight -= 1;

        // Update congestion control
        self.update_congestion_control(inner, 1);

        Ok(())
    }

    fn update_congestion_control(&self, inner: &mut UdxStreamInner, acked: u32) {
        if inner.cwnd < inner.ssthresh {
            // Slow start
            inner.cwnd += acked;
            if inner.cwnd > inner.ssthresh {
                inner.cwnd = inner.ssthresh;
            }
        } else {
            // Congestion avoidance
            let now = Instant::now();
            inner.congestion.update(inner.cwnd, acked, now);
            self.increase_cwnd(inner, inner.congestion.cnt, acked);
        }
    }

    fn increase_cwnd(&self, inner: &mut UdxStreamInner, cnt: u32, acked: u32) {
        if inner.cwnd >= CONG_MAX_CWND {
            return;
        }

        // Smooth increase using counters
        if inner.cwnd_cnt >= cnt {
            inner.cwnd_cnt = 0;
            inner.cwnd += 1;
        }

        inner.cwnd_cnt += acked;

        if inner.cwnd_cnt >= cnt {
            let delta = inner.cwnd_cnt / cnt;
            inner.cwnd_cnt -= delta * cnt;
            inner.cwnd += delta;
        }

        if inner.cwnd > CONG_MAX_CWND {
            inner.cwnd = CONG_MAX_CWND;
        }
    }

    pub async fn process_packet(&self, packet: &[u8]) -> Result<()> {
        let mut inner = self.inner.lock().await;

        if packet.len() < HEADER_SIZE {
            return Err(UdxError::InvalidPacket);
        }

        let header = unsafe { &*(packet.as_ptr() as *const PacketHeader) };

        if header.magic != MAGIC_BYTE || header.version != VERSION {
            return Err(UdxError::InvalidPacket);
        }

        inner.stats.packets_rx += 1;
        inner.stats.bytes_rx += packet.len() as u64;

        // Process acks
        if seq_compare(header.ack, inner.remote_acked) >= 0 {
            self.process_ack(&mut inner, header.ack)?;
        }

        // Update receive window
        if seq_compare(inner.send_wl1, header.seq) < 0
            || (inner.send_wl1 == header.seq && seq_compare(inner.send_wl2, header.ack) <= 0)
        {
            inner.send_window = header.recv_window;
            inner.send_wl1 = header.seq;
            inner.send_wl2 = header.ack;
        }

        // Handle data
        if header.type_ & UDX_HEADER_DATA != 0 {
            self.handle_data(&mut inner, header, &packet[HEADER_SIZE..])?;
        }

        // Handle stream end
        if header.type_ & UDX_HEADER_END != 0 {
            inner.state.remote_ending = true;
            inner.remote_ended = header.seq;
        }

        // Handle stream destroy
        if header.type_ & UDX_HEADER_DESTROY != 0 {
            inner.state.destroyed = true;
            return self.close(&mut inner, Some(UdxError::Closed));
        }

        Ok(())
    }

    fn handle_data(
        &self,
        inner: &mut UdxStreamInner,
        header: &PacketHeader,
        data: &[u8],
    ) -> Result<()> {
        if seq_compare(header.seq, inner.ack) < 0 {
            // Old packet, ignore
            return Ok(());
        }

        if header.seq == inner.ack {
            // In-order packet
            inner.ack += 1;
            // Deliver data to user
            if let Some(ref callback) = inner.read_callback {
                callback(data);
            }

            // Process any buffered packets that are now in order
            while let Some(pkt) = inner.incoming.remove(inner.ack) {
                inner.ack += 1;
                if let Some(ref callback) = inner.read_callback {
                    callback(&pkt.data);
                }
            }
        } else {
            // Out of order packet, buffer it
            let packet = Packet::new(
                header.type_,
                header.remote_id,
                header.seq,
                header.ack,
                header.recv_window,
            );
            inner.incoming.set(header.seq, packet);
        }

        Ok(())
    }

    pub async fn write_end(&self) -> Result<()> {
        let (seq, packet) = {
            let mut inner = self.inner.lock().await;

            if !inner.state.connected {
                return Err(UdxError::NotConnected);
            }

            if inner.state.local_ending {
                return Err(UdxError::Closed);
            }

            inner.state.local_ending = true;
            inner.status |= STREAM_ENDING;

            let packet = Packet::new(
                UDX_HEADER_END,
                inner.remote_id,
                inner.seq,
                inner.ack,
                inner.recv_window,
            );
            (inner.seq, packet)
        };

        self.inner.lock().await.outgoing.set(seq, packet);
        let mut inner = self.inner.lock().await;
        inner.seq += 1;

        self.try_transmit(&mut inner)?;

        Ok(())
    }

    pub fn close(&self, inner: &mut UdxStreamInner, err: Option<UdxError>) -> Result<()> {
        if inner.status & STREAM_CLOSED != 0 {
            return Ok(());
        }

        inner.status |= STREAM_CLOSED;
        inner.status &= !STREAM_CONNECTED;

        // Clear queues
        self.clear_queues(inner);

        // Send final destroy packet if needed
        if !inner.state.destroyed {
            let packet = Packet::new(
                UDX_HEADER_DESTROY,
                inner.remote_id,
                inner.seq,
                inner.ack,
                inner.recv_window,
            );

            if let Some(socket) = &inner.socket {
                if let Some(addr) = inner.remote_addr {
                    let _ = socket.send(packet, addr);
                }
            }
        }

        // Notify close callback if set
        if let Some(ref callback) = inner.close_callback {
            callback(err);
        }

        Ok(())
    }

    fn clear_queues(&self, inner: &mut UdxStreamInner) {
        // Clear outgoing packets
        for seq in inner.remote_acked..inner.seq {
            if let Some(packet) = inner.outgoing.remove(seq) {
                if let Some(ref callback) = inner.write_callbacks.get(&seq) {
                    callback(Err(UdxError::Closed));
                }
            }
        }

        // Clear incoming packets
        while let Some(_) = inner.incoming.remove(inner.ack) {
            inner.ack += 1;
        }

        // Clear queues
        inner.inflight_queue = Queue::new();
        inner.retransmit_queue = Queue::new();
        inner.write_queue = Queue::new();
    }

    // MTU Discovery methods
    fn handle_mtu_probe(&self, inner: &mut UdxStreamInner, packet: &Packet) -> Result<()> {
        if !packet.is_mtu_probe {
            return Ok(());
        }

        if inner.mtu_state == MtuState::Search {
            inner.mtu_probe_count = 0;
            inner.mtu = inner.mtu_probe_size;

            if inner.mtu_probe_size == inner.mtu_max {
                inner.mtu_state = MtuState::SearchComplete;
            } else {
                inner.mtu_probe_size += MTU_STEP;
                if inner.mtu_probe_size > inner.mtu_max {
                    inner.mtu_probe_size = inner.mtu_max;
                }
                inner.mtu_probe_wanted = true;
            }
        }

        Ok(())
    }

    fn create_mtu_probe(&self, inner: &mut UdxStreamInner, packet: &mut Packet) -> bool {
        if !inner.mtu_probe_wanted || packet.size >= inner.mtu_probe_size {
            return false;
        }

        let padding_size = inner.mtu_probe_size - packet.size;
        if padding_size > 255 {
            return false;
        }

        packet.data.extend(vec![0u8; padding_size.into()]);
        packet.size = inner.mtu_probe_size;
        packet.is_mtu_probe = true;
        inner.mtu_probe_wanted = false;
        inner.mtu_probe_count += 1;

        true
    }

    // Add callback setters
    pub async fn set_read_callback<F>(&self, callback: F)
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        let mut inner = self.inner.lock().await;
        inner.read_callback = Some(Box::new(callback));
    }

    pub async fn set_close_callback<F>(&self, callback: F)
    where
        F: Fn(Option<UdxError>) + Send + Sync + 'static,
    {
        let mut inner = self.inner.lock().await;
        inner.close_callback = Some(Box::new(callback));
    }
}

// Helper functions for sequence number comparison
fn seq_compare(a: u32, b: u32) -> i32 {
    let d = a.wrapping_sub(b) as i32;
    if d < 0 {
        -1
    } else if d > 0 {
        1
    } else {
        0
    }
}

#[derive(Debug, Clone, Default)]
struct StreamStats {
    bytes_rx: u64,
    bytes_tx: u64,
    packets_rx: u64,
    packets_tx: u64,
}

#[derive(Debug, Clone, Default)]
struct StreamState {
    connected: bool,
    destroyed: bool,
    remote_ending: bool,
    local_ending: bool,
}

#[derive(Debug, Clone)]
struct Mtu {
    current: u16,
    probe_size: u16,
    probe_count: u8,
    state: MtuState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MtuState {
    Base = 1,
    Search = 2,
    Error = 3,
    SearchComplete = 4,
}

/// Write buffer for stream
#[derive(Clone, Debug)]
struct WriteBuffer {
    data: Vec<u8>,
    offset: usize,
    acked: usize,
    inflight: usize,
}
