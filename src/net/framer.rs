//! Frame encoding and decoding for the load generator binary protocol.
//!
//! This module provides a Tokio codec implementation that handles the framing
//! of messages according to the binary protocol. It manages the process of
//! converting between complete [`Message`] instances and their byte
//! representations on the wire.
//!
//! The framer handles:
//! - Message serialization with proper length framing
//! - Message deserialization by reading complete frames
//! - Buffer management for partial reads
//! - Error propagation for both header and payload operations
use std::marker::PhantomData;

use super::protocol::{HeaderDeError, Message, MessageHeader, MessageSerError};
use super::{BytesDeserializer, SerdeError};
use bytes::Buf;
use tokio_util::codec;

/// A codec that encodes and decodes `Message` instances according to the binary protocol.
///
/// This struct implements Tokio's [`Encoder`] and [`Decoder`] traits to provide
/// stream-based serialization and deserialization of protocol messages.
///
/// # Type Parameters
/// * `S` - The type of payload to encode (send), must implement
///   [`Serialize`][`serde::ser::Serialize`]
/// * `R` - The type of payload to decode (receive), must implement
///   [`Deserialize`][`serde::de::Deserialize`]
///
///
/// [`Encoder`]: tokio_util::codec::Encoder
/// [`Decoder`]: tokio_util::codec::Decoder
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MessageFramer<S, R> {
    _marker_s: PhantomData<S>,
    _marker_r: PhantomData<R>,
}

impl<S, R> MessageFramer<S, R> {
    /// Creates a new message framer.
    ///
    /// # Returns
    /// A new `MessageFramer` instance
    pub fn new() -> Self {
        Self {
            _marker_s: PhantomData,
            _marker_r: PhantomData,
        }
    }
}

/// Errors that can occur during message deserialization.
///
/// This enum wraps errors from different stages of the deserialization process:
/// header parsing, payload parsing, and I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum DecoderError {
    /// An error occurred while deserializing the message header.
    #[error("failed to deserialize message header: {0}")]
    Header(#[from] HeaderDeError),

    /// An error occurred while deserializing the message payload.
    #[error("failed to deserialize message payload: {0}")]
    Payload(#[from] SerdeError),

    /// An I/O error occurred during receiving the message.
    #[error("failed to receive message: {0}")]
    Io(#[from] std::io::Error),

    /// The message size exceeds the maximum representable value.
    #[error("message length overflow: length exceeds the maximum of `{max}` bytes supported by this machine", max=usize::MAX)]
    SizeOverflow,
}

/// Errors that can occur during message serialization.
///
/// This enum wraps errors from different stages of the serialization process:
/// header serialization, payload serialization, I/O operations, and size validation.
///
/// # Type Parameters
/// * `SE` - The error type for payload serialization
#[derive(Debug, thiserror::Error)]
pub enum EncoderError {
    /// An error occurred during message serialization
    #[error("failed to serialize message: {0}")]
    Message(#[from] MessageSerError),

    /// An I/O error occurred during sending the message.
    #[error("failed to send message: {0}")]
    Io(#[from] std::io::Error),
}

impl<S, R> codec::Encoder<S> for MessageFramer<S, R>
where
    S: serde::Serialize,
{
    type Error = EncoderError;

    /// Encodes a payload into the output buffer.
    ///
    /// This method wraps the payload in a `Message` internally, then serializes
    /// the message by first writing the header, followed by the payload. The
    /// buffer is automatically resized if necessary to accommodate the complete
    /// message.
    ///
    /// # Arguments
    /// * `item` - The payload to encode
    /// * `dst` - The destination buffer where the encoded message will be written
    ///
    /// # Returns
    /// * [`Ok`] if encoding was successful
    /// * [`Err`] if encoding failed
    ///
    /// # Buffer Behavior
    /// This method will reserve sufficient capacity in the destination buffer
    /// to hold the complete encoded message before writing.
    fn encode(
        &mut self,
        item: S,
        dst: &mut bytes::BytesMut,
    ) -> std::result::Result<(), Self::Error> {
        let message = Message::from(item);
        message.serialize(dst)?;
        Ok(())
    }
}

impl<S, R> codec::Decoder for MessageFramer<S, R>
where
    R: serde::de::DeserializeOwned,
{
    type Item = R;
    type Error = DecoderError;

    /// Decodes a message from the input buffer.
    ///
    /// This method attempts to read a complete message from the input buffer.
    /// If insufficient data is available, it returns `Ok(None)` to indicate
    /// that more data is needed. When a complete message is successfully
    /// decoded, it returns `Ok(Some(payload))` with the bare payload.
    ///
    /// # Arguments
    /// * `src` - The source buffer containing potentially partial message data
    ///
    /// # Returns
    /// * [`Ok`] when a complete message is decoded or more data is needed
    /// * [`Err`] when decoding fails
    ///
    /// # Buffer Management
    /// This method automatically advances the buffer past successfully decoded
    /// messages and reserves additional capacity when partial messages are detected.
    fn decode(
        &mut self,
        src: &mut bytes::BytesMut,
    ) -> std::result::Result<Option<Self::Item>, Self::Error> {
        if src.len() < MessageHeader::SIZE {
            src.reserve(MessageHeader::SIZE);
            return Ok(None);
        }
        let header = MessageHeader::deserialize(src, MessageHeader::SIZE)?;

        let payload_size = header.payload_size();

        let message_size = MessageHeader::SIZE
            .checked_add(payload_size)
            .ok_or(DecoderError::SizeOverflow)?;

        if src.len() < message_size {
            src.reserve(message_size.saturating_sub(src.len()));
            return Ok(None);
        }

        src.advance(MessageHeader::SIZE);
        let mut payload_bytes = src.split_to(payload_size);
        let payload = R::deserialize(&mut BytesDeserializer::new(&mut payload_bytes))?;

        Ok(Some(payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::protocol::{MAGIC_NUMBER, MessageHeader, VERSION};
    use bytes::{BufMut, BytesMut};
    use std::assert_matches;
    use tokio_util::codec::{Decoder, Encoder};

    // A simple test payload type for testing the framer
    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    struct TestPayload {
        data: Vec<u8>,
    }

    impl TestPayload {
        fn new(data: Vec<u8>) -> Self {
            Self { data }
        }
    }

    // A simple test payload type with a fixed prefix for testing the framer
    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    struct FixedPrefixTestPayload {
        payload: TestPayload,
    }

    impl FixedPrefixTestPayload {
        const PREFIX: u8 = 1;

        fn new(data: Vec<u8>) -> Self {
            Self {
                payload: TestPayload::new(data),
            }
        }
    }

    #[test]
    fn test_encode_works() {
        let payload = TestPayload::new(vec![1, 2, 3, 4, 5]);

        let mut framer: MessageFramer<TestPayload, TestPayload> = MessageFramer::new();
        let mut buffer = BytesMut::new();

        let result = framer.encode(payload, &mut buffer);
        assert!(result.is_ok());
    }

    #[test]
    fn test_decode_works() {
        let original_payload = TestPayload::new(vec![1, 2, 3, 4, 5]);

        let mut framer: MessageFramer<TestPayload, TestPayload> = MessageFramer::new();
        let mut buffer = BytesMut::new();
        framer
            .encode(original_payload.clone(), &mut buffer)
            .expect("encode not to fail");

        let decoded_payload = framer.decode(&mut buffer).expect("decode not to fail");

        assert!(decoded_payload.is_some());

        let payload = decoded_payload.unwrap();
        assert_eq!(payload, original_payload);
    }

    #[test]
    fn test_decode_returns_none_for_partial_header() {
        let magic_number_bytes = MAGIC_NUMBER.to_be_bytes();
        let mut buffer = BytesMut::from(&magic_number_bytes[..]);
        let mut framer: MessageFramer<TestPayload, TestPayload> = MessageFramer::new();

        let result = framer.decode(&mut buffer);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_wrong_magic_number_returns_error() {
        let mut buffer = BytesMut::new();
        let magic_number = MAGIC_NUMBER.wrapping_sub(1);
        let payload = [1, 2, 3, 4, 5];
        buffer.put_u16(magic_number);
        buffer.put_u8(VERSION);
        buffer.put_u8(0);
        buffer.put_u64(payload.len() as u64);
        buffer.extend_from_slice(&payload[..]);

        let mut framer: MessageFramer<TestPayload, TestPayload> = MessageFramer::new();
        let result = framer.decode(&mut buffer);

        assert_matches!(
            result,
            Err(DecoderError::Header(HeaderDeError::MagicNumberMismatch(_)))
        );
    }

    #[test]
    fn test_wrong_version_returns_error() {
        let mut buffer = BytesMut::new();
        let version = VERSION.wrapping_add(1);
        let payload = [1, 2, 3, 4, 5];

        buffer.put_u16(MAGIC_NUMBER);
        buffer.put_u8(version);
        buffer.put_u8(0);
        buffer.put_u64(payload.len() as u64);
        buffer.extend_from_slice(&payload[..]);

        let mut framer: MessageFramer<TestPayload, TestPayload> = MessageFramer::new();
        let result = framer.decode(&mut buffer);

        assert_matches!(
            result,
            Err(DecoderError::Header(HeaderDeError::VersionMismatch(_)))
        );
    }

    #[test]
    fn test_fixed_prefix_payload_works() {
        let prefix = FixedPrefixTestPayload::PREFIX;
        let data = vec![prefix, 2, 3, 4, 5, 6];
        let payload = FixedPrefixTestPayload::new(data);
        let mut buffer = BytesMut::new();

        let mut framer: MessageFramer<FixedPrefixTestPayload, FixedPrefixTestPayload> =
            MessageFramer::new();

        framer
            .encode(payload.clone(), &mut buffer)
            .expect("encode not to fail");

        let decoded_payload = framer.decode(&mut buffer).expect("decode not to fail");

        assert!(decoded_payload.is_some());

        let result_payload = decoded_payload.unwrap();
        assert_eq!(result_payload, payload);
    }

    #[test]
    fn test_wrong_payload_fails_to_deserialize() {
        let mut buffer = BytesMut::new();
        let prefix = FixedPrefixTestPayload::PREFIX;
        let data = vec![prefix, 2, 3, 4, 5, 6];
        let payload = FixedPrefixTestPayload::new(data);

        let mut framer: MessageFramer<FixedPrefixTestPayload, FixedPrefixTestPayload> =
            MessageFramer::new();

        framer.encode(payload, &mut buffer).unwrap();

        let prefix_offset = MessageHeader::SIZE;
        let prefix_byte = buffer.get_mut(prefix_offset).unwrap();
        *prefix_byte = prefix.wrapping_add(1);

        let result = framer.decode(&mut buffer);

        assert_matches!(result, Err(DecoderError::Payload(_)));
    }

    #[test]
    fn test_roundtrip_empty_payload() {
        let payload = TestPayload::new(vec![]);

        let mut framer: MessageFramer<TestPayload, TestPayload> = MessageFramer::new();
        let mut buffer = BytesMut::new();
        framer.encode(payload.clone(), &mut buffer).unwrap();

        let decoded_payload = framer.decode(&mut buffer).unwrap().unwrap();
        assert_eq!(decoded_payload, payload);
    }
}
