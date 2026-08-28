mod error;
mod profile;
mod registry;
mod warmup;

pub use error::{Error, Result};
pub use profile::{DeadlineConfig, LoadProfileConfig, LoadStepConfig};
pub use registry::ServiceRegistryConfig;
pub use warmup::WarmupConfig;

/// A type that can be parsed from in-memory bytes.
pub trait FromBytes: Sized {
    fn from_bytes(bytes: &[u8]) -> Result<Self>;
}

impl FromBytes for std::sync::Arc<str> {
    /// Parses a Lua script from UTF-8 bytes.
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let s = std::str::from_utf8(bytes).map_err(|e| Error::InvalidUtf8(e.valid_up_to()))?;
        Ok(s.into())
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::sync::Arc;

    use super::FromBytes;
    use crate::config::Error;

    #[test]
    fn script_from_bytes_accepts_valid_utf8() {
        let script = <Arc<str> as FromBytes>::from_bytes(b"function requests() end\n").unwrap();
        assert_eq!(&*script, "function requests() end\n");
    }

    #[test]
    fn script_from_bytes_rejects_invalid_utf8() {
        let err = <Arc<str> as FromBytes>::from_bytes(&[0x61, 0x62, 0x63, 0xff, 0xfe]).unwrap_err();
        assert_matches!(err, Error::InvalidUtf8(3));
    }
}
