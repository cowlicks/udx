use crate::constants::{HEADER_SIZE, MAGIC_BYTE, VERSION};

#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct PacketHeader {
    pub magic: u8,
    pub version: u8,
    pub type_: u8,
    pub extensions: u8,
    pub remote_id: u32,
    pub recv_window: u32,
    pub seq: u32,
    pub ack: u32,
}

#[derive(Clone, Debug)]
pub struct Packet {
    pub header: PacketHeader,
    pub data: Vec<u8>,
    pub transmits: u8,
    pub seq: u32,
    pub time_sent: std::time::Instant,
    pub lost: bool,
    pub retransmitted: bool,
    pub is_mtu_probe: bool,
    pub size: u16,
}

impl Packet {
    pub fn new(type_: u8, remote_id: u32, seq: u32, ack: u32, recv_window: u32) -> Self {
        Self {
            header: PacketHeader {
                magic: MAGIC_BYTE,
                version: VERSION,
                type_,
                extensions: 0,
                remote_id,
                recv_window,
                seq,
                ack,
            },
            data: Vec::new(),
            transmits: 0,
            seq,
            time_sent: std::time::Instant::now(),
            lost: false,
            retransmitted: false,
            is_mtu_probe: false,
            size: HEADER_SIZE as u16,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_SIZE + self.data.len());

        unsafe {
            let header_slice = std::slice::from_raw_parts(
                &self.header as *const _ as *const u8,
                std::mem::size_of::<PacketHeader>(),
            );
            bytes.extend_from_slice(header_slice);
        }

        bytes.extend_from_slice(&self.data);
        bytes
    }
}
