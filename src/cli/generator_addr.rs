use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use super::Error;

pub const DEFAULT_GENERATOR_PORT: u16 = 24266;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratorAddr {
    addr: Arc<str>,
    port: u16,
}

impl GeneratorAddr {
    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl FromStr for GeneratorAddr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle [IPv6]:port or [IPv6]
        if s.starts_with("[") {
            let (addr, port) = s
                .split_once(']')
                .ok_or_else(|| Error::InvalidIPv6Format(s.to_string()))?;
            let addr = format!("{addr}]");
            let port = match port.strip_prefix(':') {
                Some(port) => port
                    .parse::<u16>()
                    .map_err(|_| Error::InvalidPort(port.to_string()))?,
                None if port.is_empty() => DEFAULT_GENERATOR_PORT,
                None => return Err(Error::InvalidPort(port.to_string())),
            };
            return Ok(GeneratorAddr {
                addr: addr.into(),
                port,
            });
        }

        // Try addr:port
        if let Some((addr, port)) = s.rsplit_once(':') {
            let port = port
                .parse::<u16>()
                .map_err(|_| Error::InvalidPort(port.to_string()))?;
            return Ok(GeneratorAddr {
                addr: addr.to_string().into(),
                port,
            });
        }

        // No port — use default
        Ok(GeneratorAddr {
            addr: s.to_string().into(),
            port: DEFAULT_GENERATOR_PORT,
        })
    }
}

impl fmt::Display for GeneratorAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[test]
    fn parses_addr_with_port() {
        let addr: GeneratorAddr = "10.0.0.1:8080".parse().unwrap();
        assert_eq!(addr.addr(), "10.0.0.1");
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn parses_addr_without_port_uses_default() {
        let addr: GeneratorAddr = "10.0.0.1".parse().unwrap();
        assert_eq!(addr.addr(), "10.0.0.1");
        assert_eq!(addr.port(), DEFAULT_GENERATOR_PORT);
    }

    #[test]
    fn parses_hostname_with_port() {
        let addr: GeneratorAddr = "my-generator.local:9090".parse().unwrap();
        assert_eq!(addr.addr(), "my-generator.local");
        assert_eq!(addr.port(), 9090);
    }

    #[test]
    fn parses_hostname_without_port() {
        let addr: GeneratorAddr = "my-generator.local".parse().unwrap();
        assert_eq!(addr.addr(), "my-generator.local");
        assert_eq!(addr.port(), DEFAULT_GENERATOR_PORT);
    }

    #[test]
    fn parses_ipv6_with_port() {
        let addr: GeneratorAddr = "[::1]:8888".parse().unwrap();
        assert_eq!(addr.addr(), "[::1]");
        assert_eq!(addr.port(), 8888);
    }

    #[test]
    fn parses_ipv6_without_port() {
        let addr: GeneratorAddr = "[::1]".parse().unwrap();
        assert_eq!(addr.addr(), "[::1]");
        assert_eq!(addr.port(), DEFAULT_GENERATOR_PORT);
    }

    #[test]
    fn errors_on_ipv6_missing_bracket() {
        let invalid = "[::1:8080";
        let err = invalid.parse::<GeneratorAddr>().unwrap_err();
        assert_matches!(err, Error::InvalidIPv6Format(s) if s == invalid);
    }

    #[test]
    fn errors_on_ipv6_unexpected_suffix() {
        let err = "[::1]extra".parse::<GeneratorAddr>().unwrap_err();
        assert_matches!(err, Error::InvalidPort(s) if s == "extra");
    }

    #[test]
    fn errors_on_invalid_port_in_ipv6() {
        let err = "[::1]:notaport".parse::<GeneratorAddr>().unwrap_err();
        assert_matches!(err, Error::InvalidPort(s) if s == "notaport");
    }

    #[test]
    fn errors_on_invalid_port_in_ipv4() {
        let err = "10.0.0.1:notaport".parse::<GeneratorAddr>().unwrap_err();
        assert_matches!(err, Error::InvalidPort(s) if s == "notaport");
    }

    #[test]
    fn display_formats_addr_port() {
        let addr: GeneratorAddr = "10.0.0.1:8080".parse().unwrap();
        assert_eq!(addr.to_string(), "10.0.0.1:8080");
    }

    #[test]
    fn display_formats_with_default_port() {
        let addr: GeneratorAddr = "10.0.0.1".parse().unwrap();
        assert_eq!(
            addr.to_string(),
            format!("10.0.0.1:{DEFAULT_GENERATOR_PORT}")
        );
    }
}
