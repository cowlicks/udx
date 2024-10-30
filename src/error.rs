#[derive(thiserror::Error, Debug)]
pub enum UdxError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Stream not connected")]
    NotConnected,

    #[error("Stream closed")]
    Closed,

    #[error("Invalid packet")]
    InvalidPacket,

    #[error("Buffer too small")]
    BufferTooSmall,

    #[error("Invalid state")]
    InvalidState,
}

pub type Result<T> = std::result::Result<T, UdxError>;
