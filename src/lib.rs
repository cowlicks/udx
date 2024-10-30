//! UDX: Reliable, multiplexed, and congestion controlled streams over UDP
//!
//! This is a Rust implementation of the UDX protocol, translated from the original C implementation.
//! The protocol provides reliable streams over UDP with:
//! - Stream multiplexing
//! - Congestion control
//! - Flow control
//! - Reliable delivery
#![warn(
    missing_debug_implementations,
    missing_docs,
    redundant_lifetimes,
    non_local_definitions,
    unsafe_code,
    non_local_definitions
)]

pub mod cirbuf;
pub mod congestion;
pub mod constants;
pub mod error;
pub mod interface;
pub mod packet;
pub mod queue;
pub mod socket;
pub mod stream;

pub use error::UdxError;
pub use socket::UdxSocket;
pub use stream::UdxStream;

// Constants translated from the C implementation
pub const UDX_MAGIC_BYTE: u8 = 0x75; // 'u'
pub const UDX_VERSION: u8 = 0x01;

const UDX_STREAM_ENDED: u32 = 0b000100000;
const UDX_STREAM_ENDED_REMOTE: u32 = 0b001000000;
const UDX_STREAM_DESTROYING: u32 = 0b010000000;
const UDX_STREAM_CLOSED: u32 = 0b100000000;
const UDX_STREAM_ALL_ENDED: u32 = UDX_STREAM_ENDED | UDX_STREAM_ENDED_REMOTE;
const UDX_STREAM_DEAD: u32 = UDX_STREAM_DESTROYING | UDX_STREAM_CLOSED;

const UDX_HEADER_DATA: u8 = 0x01;
const UDX_HEADER_END: u8 = 0x02;
const UDX_HEADER_SACK: u8 = 0x04;
const UDX_HEADER_DESTROY: u8 = 0x08;
const UDX_HEADER_MESSAGE: u8 = 0x10;

const UDX_DEFAULT_TTL: u8 = 64;
const UDX_DEFAULT_BUFFER_SIZE: usize = 212992;
const UDX_MAX_RTO_TIMEOUTS: u32 = 6;

// Congestion control constants
const UDX_CONG_C: u32 = 400; // C=0.4 (inverse) in scaled 1000
const UDX_CONG_C_SCALE: u64 = 1_000_000_000_000; // ms/s ** 3 * c-scale
const UDX_CONG_BETA: u32 = 731; // b=0.3, BETA = 1-b, scaled 1024
const UDX_CONG_BETA_UNIT: u32 = 1024;
const UDX_CONG_INIT_CWND: u32 = 10;
const UDX_CONG_MAX_CWND: u32 = 65536;
