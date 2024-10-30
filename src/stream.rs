use std::sync::Arc;

use crate::{
    cirbuf::CircularBuffer,
    congestion::CongestionControl,
    error::{Result, UdxError},
    packet::{Packet, PacketHeader},
    queue::Queue,
    UdxSocket, WriteBuffer,
};

pub struct UdxStream {
    local_id: u32,
    remote_id: u32,
    socket: Option<Arc<UdxSocket>>,

    // Connection state
    state: StreamState,
    congestion: CongestionControl,
    mtu: Mtu,

    // Queues and buffers
    outgoing: CircularBuffer<Packet>,
    incoming: CircularBuffer<Packet>,
    write_queue: Queue<WriteBuffer>,

    // Stats and metrics
    stats: StreamStats,
}

struct StreamStats {
    bytes_rx: u64,
    bytes_tx: u64,
    packets_rx: u64,
    packets_tx: u64,
}

#[derive(Debug)]
struct StreamState {
    connected: bool,
    destroyed: bool,
    remote_ending: bool,
    local_ending: bool,
}

#[derive(Debug)]
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
