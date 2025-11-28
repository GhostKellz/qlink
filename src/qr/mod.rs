//! QR code encoding and decoding
//!
//! This module provides fast QR code processing with support for both
//! encoding (generating QR codes) and decoding (scanning QR codes from images).

mod decoder;
mod encoder;

pub use decoder::QrDecoder;
pub use encoder::QrEncoder;

use serde::{Deserialize, Serialize};

/// A decoded QR code payload
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QrPayload {
    /// The raw decoded data
    pub data: Vec<u8>,
    /// String representation if valid UTF-8
    pub text: Option<String>,
}

impl QrPayload {
    /// Create a new QR payload from raw bytes
    pub fn from_bytes(data: Vec<u8>) -> Self {
        let text = String::from_utf8(data.clone()).ok();
        Self { data, text }
    }

    /// Create a new QR payload from a string
    pub fn from_string(s: String) -> Self {
        Self {
            data: s.as_bytes().to_vec(),
            text: Some(s),
        }
    }

    /// Get the payload as a string, if valid UTF-8
    pub fn as_str(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qr_payload_from_string() {
        let payload = QrPayload::from_string("hello world".to_string());
        assert_eq!(payload.as_str(), Some("hello world"));
        assert_eq!(payload.as_bytes(), b"hello world");
    }

    #[test]
    fn test_qr_payload_from_bytes() {
        let payload = QrPayload::from_bytes(vec![0xFF, 0xFE]);
        assert!(payload.as_str().is_none()); // Invalid UTF-8
        assert_eq!(payload.as_bytes(), &[0xFF, 0xFE]);
    }
}
