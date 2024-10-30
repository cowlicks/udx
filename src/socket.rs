use crate::{constants::*, error::*, packet::*, queue::Queue, stream::UdxStream};
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{mpsc, Mutex},
    task::spawn,
};

pub struct UdxSocket {
    inner: Arc<UdxSocketInner>,
}

struct UdxSocketInner {
    socket: UdpSocket,
    state: Mutex<SocketState>,
    streams: Mutex<HashMap<u32, Arc<UdxStream>>>,
    send_queue: Mutex<Queue<Packet>>,
    stats: Arc<Mutex<SocketStats>>,
}

#[derive(Debug, Clone, Default)]
struct SocketState {
    receiving: bool,
    bound: bool,
    closed: bool,
    ttl: u8,
    family: u8, // 4 or 6 for IPv4/IPv6
}

#[derive(Debug, Clone, Default)]
struct SocketStats {
    bytes_rx: u64,
    bytes_tx: u64,
    packets_rx: u64,
    packets_tx: u64,
}

impl UdxSocket {
    pub fn bind(addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(addr)?;

        // Set socket options
        socket.set_nonblocking(true)?;
        //socket.set_recv_buffer_size(DEFAULT_BUFFER_SIZE as usize)?;
        //socket.set_send_buffer_size(DEFAULT_BUFFER_SIZE as usize)?;
        //socket.bind(&addr.into())?;

        let inner = Arc::new(UdxSocketInner {
            socket,
            state: Mutex::new(SocketState {
                ttl: DEFAULT_TTL,
                family: if addr.is_ipv4() { 4 } else { 6 },
                ..Default::default()
            }),
            streams: Mutex::new(HashMap::new()),
            send_queue: Mutex::new(Queue::new()),
            stats: Arc::new(Mutex::new(SocketStats::default())),
        });

        Ok(Self { inner })
    }

    pub async fn set_ttl(&self, ttl: u8) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        state.ttl = ttl;
        self.inner.socket.set_ttl(ttl as u32)?;
        Ok(())
    }

    pub async fn start_receive(&self) -> Result<mpsc::Receiver<(Vec<u8>, SocketAddr)>> {
        let (tx, rx) = mpsc::channel(1000);
        let socket = self.inner.socket.try_clone()?;
        let stats = self.inner.stats.clone();

        spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                // TODO replace with stdlib's udp socket so I can do this without unsafe
                match socket.recv_from(&mut buf) {
                    Ok((n, addr)) => {
                        let packet = buf[..n].to_vec();
                        {
                            let mut s = stats.lock().await;
                            s.bytes_rx += n as u64;
                            s.packets_rx += 1;
                        }

                        if tx.send((packet, addr)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(rx)
    }

    pub async fn send(&self, packet: Packet, addr: SocketAddr) -> Result<()> {
        let bytes = packet.to_bytes();

        match self.inner.socket.send_to(&bytes, addr) {
            Ok(n) => {
                {
                    let mut s = self.inner.stats.lock().await;
                    s.bytes_tx += n as u64;
                    s.packets_tx += 1;
                }

                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Queue for later sending
                self.inner.send_queue.lock().await.push_back(packet);
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}
