pub const HEADER_SIZE: usize = 20;
pub const IPV4_HEADER_SIZE: usize = 20 + 8 + HEADER_SIZE;
pub const IPV6_HEADER_SIZE: usize = 40 + 8 + HEADER_SIZE;

pub const MTU_BASE: u16 = 1200;
pub const MTU_MAX_PROBES: u8 = 3;
pub const MTU_MAX: u16 = 1500;
pub const MTU_STEP: u16 = 32;

pub const MAGIC_BYTE: u8 = 0xFF;
pub const VERSION: u8 = 1;

pub const DEFAULT_TTL: u8 = 64;
pub const DEFAULT_BUFFER_SIZE: usize = 212992;

// Congestion control constants
pub const CONG_C: u32 = 400; // C=0.4 (inverse) in scaled 1000
pub const CONG_C_SCALE: u64 = 1_000_000_000_000; // ms/s ** 3 * c-scale
pub const CONG_BETA: u32 = 731; // b=0.3, BETA = 1-b, scaled 1024
pub const CONG_BETA_UNIT: u32 = 1024;
pub const CONG_INIT_CWND: u32 = 10;
pub const CONG_MAX_CWND: u32 = 65536;

// Stream states
pub const STREAM_CONNECTED: u32 = 0b000000001;
pub const STREAM_RECEIVING: u32 = 0b000000010;
pub const STREAM_READING: u32 = 0b000000100;
pub const STREAM_ENDING: u32 = 0b000001000;
pub const STREAM_ENDING_REMOTE: u32 = 0b000010000;
pub const STREAM_ENDED: u32 = 0b000100000;
pub const STREAM_ENDED_REMOTE: u32 = 0b001000000;
pub const STREAM_DESTROYING: u32 = 0b010000000;
pub const STREAM_CLOSED: u32 = 0b100000000;
