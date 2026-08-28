//! Protocol definitions for the load generator binary communication.
//!
//! This module defines the message format used in the binary TCP protocol
//! between orchestrator and load agents. Messages consist of a fixed-size
//! header followed by a variable-length payload.

use super::{BytesSerializer, SerdeError};

/// Magic number used to identify valid protocol messages.
///
/// This constant serves as a identifier for the beginning of protocol message.
pub const MAGIC_NUMBER: u16 = 0xABCD;

/// Protocol version number.
///
/// Used to ensure compatibility between orchestrator and load agent versions.
pub const VERSION: u8 = 1;

/// A complete protocol message consisting of a header and payload.
///
/// Messages follow the format: `[Header][Payload]` where the header contains
/// metadata about the message and the payload contains the actual payload.
///
/// # Type Parameters
/// * `P` - The type of the message payload, which must implement [`Serialize`][S]
///   and [`Deserialize`][D]
///
/// [S]: serde::ser::Serialize
/// [D]: serde::de::Deserialize
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Message<P> {
    /// The message payload
    pub(crate) payload: P,
}

#[derive(Debug, thiserror::Error)]
pub enum MessageSerError {
    /// An error occurred while serializing the message header.
    #[error("failed to serialize header: {0}")]
    Header(#[from] HeaderSeError),

    /// An error occurred while serializing the message payload.
    #[error("failed to serialize payload: {0}")]
    Payload(#[from] SerdeError),
}

impl<P> From<P> for Message<P> {
    fn from(payload: P) -> Self {
        Self { payload }
    }
}
impl<P> Message<P>
where
    P: serde::Serialize,
{
    pub(super) fn serialize(&self, buf: &mut bytes::BytesMut) -> Result<(), MessageSerError> {
        let header_pos = buf.len();

        MessageHeader::init_header(buf)?;

        let payload_pos = buf.len();

        self.payload.serialize(&mut BytesSerializer::new(buf))?;

        let payload_size = buf.len() - payload_pos;
        MessageHeader::patch_payload_size(
            &mut buf[header_pos..header_pos + MessageHeader::SIZE],
            payload_size,
        )?;

        Ok(())
    }
}

/// The fixed-size header that precedes each message payload.
///
/// The header contains protocol identification information and metadata
/// about the following payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MessageHeader {
    /// Protocol flags (reserved for future use)
    flags: u8,
    /// Size of the payload in bytes
    payload_size: usize,
}

impl MessageHeader {
    const MAGIC_NUMBER_SIZE: usize = 2;
    const VERSION_SIZE: usize = 1;
    const FLAGS_SIZE: usize = 1;
    const PAYLOAD_SIZE_SIZE: usize = 8;
    pub(crate) const SIZE: usize =
        Self::MAGIC_NUMBER_SIZE + Self::VERSION_SIZE + Self::FLAGS_SIZE + Self::PAYLOAD_SIZE_SIZE;

    pub(crate) fn payload_size(&self) -> usize {
        self.payload_size
    }

    fn patch_payload_size(buf: &mut [u8], payload_size: usize) -> Result<(), HeaderSeError> {
        if buf.len() < MessageHeader::SIZE {
            return Err(HeaderSeError::InvalidBufferSize(buf.len()));
        }

        let payload_size = u64::try_from(payload_size)
            .map_err(|_| HeaderSeError::MaximumPayloadSize(payload_size))?;
        const PAYLOAD_SIZE_OFFSET: usize = MessageHeader::MAGIC_NUMBER_SIZE
            + MessageHeader::VERSION_SIZE
            + MessageHeader::FLAGS_SIZE;
        buf[PAYLOAD_SIZE_OFFSET..PAYLOAD_SIZE_OFFSET + MessageHeader::PAYLOAD_SIZE_SIZE]
            .copy_from_slice(&payload_size.to_be_bytes());

        Ok(())
    }

    fn init_header<B>(buf: &mut B) -> Result<(), HeaderSeError>
    where
        B: bytes::BufMut,
    {
        buf.put_u16(MAGIC_NUMBER);
        buf.put_u8(VERSION);
        // Flags
        buf.put_u8(0);
        // Payload Size
        buf.put_u64(0);

        Ok(())
    }

    pub(crate) fn deserialize(buf: &[u8], size: usize) -> Result<Self, HeaderDeError> {
        if size < Self::SIZE {
            return Err(HeaderDeError::InsufficientData(size));
        }

        let magic_number = u16::from_be_bytes([buf[0], buf[1]]);
        if magic_number != MAGIC_NUMBER {
            return Err(HeaderDeError::MagicNumberMismatch(magic_number));
        }

        let version = buf[2];
        if version != VERSION {
            return Err(HeaderDeError::VersionMismatch(version));
        }

        let flags = buf[3];

        let payload_size_bytes = [
            buf[4], buf[5], buf[6], buf[7], buf[8], buf[9], buf[10], buf[11],
        ];
        let payload_size = u64::from_be_bytes(payload_size_bytes);
        let payload_size =
            usize::try_from(payload_size).map_err(|_| HeaderDeError::SizeOverflow(payload_size))?;

        Ok(Self {
            flags,
            payload_size,
        })
    }
}

/// Errors that can occur during header deserialization.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HeaderDeError {
    /// The magic number in the received message doesn't match the expected value.
    #[error("magic number mismatch: expected magic number `{expected:#06X}`, but got magic number `{0:#06X}`)", expected=MAGIC_NUMBER)]
    MagicNumberMismatch(u16),

    /// The protocol version in the received message doesn't match the expected version.
    #[error("version mismatch: expected version `{expected}`, but got version `{0}`)", expected=VERSION)]
    VersionMismatch(u8),

    /// The payload size exceeds the maximum value that can be represented on this platform.
    #[error("payload size overflow: expected at most `{max}` bytes, but got `{0}` bytes", max=usize::MAX)]
    SizeOverflow(u64),

    /// Insufficient data provided to deserialize a complete header.
    #[error("insufficient data to deserialize header: expected `{expected}` bytes, got `{0}` bytes", expected = MessageHeader::SIZE)]
    InsufficientData(usize),
}

/// Errors that can occur during header serialization.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HeaderSeError {
    /// The payload size exceeds the maximum value that can be serialized.
    #[error("payload size exceeds maximum size: expected at most `{max}` bytes, but got `{0}` bytes", max=u64::MAX)]
    MaximumPayloadSize(usize),

    #[error("invalid buffer size for patching payload size: expected a buffer with at least `{len}` bytes, but got `{0}` bytes", len=MessageHeader::SIZE)]
    InvalidBufferSize(usize),
}

// TODO: tests
