//! QR code encoder

use crate::error::{Error, Result};
use crate::qr::QrPayload;
use image::{DynamicImage, Luma};
use qrcode::QrCode;

/// QR code encoder
pub struct QrEncoder {
    /// Error correction level
    ecc_level: qrcode::EcLevel,
}

impl QrEncoder {
    /// Create a new QR encoder with default settings (Medium ECC)
    pub fn new() -> Self {
        Self {
            ecc_level: qrcode::EcLevel::M,
        }
    }

    /// Create a new QR encoder with a specific error correction level
    pub fn with_ecc_level(ecc_level: qrcode::EcLevel) -> Self {
        Self { ecc_level }
    }

    /// Encode data into a QR code image
    pub fn encode(&self, payload: &QrPayload) -> Result<DynamicImage> {
        let code = QrCode::with_error_correction_level(&payload.data, self.ecc_level)
            .map_err(|e| Error::QrEncode(format!("Failed to create QR code: {}", e)))?;

        // Render to image with a reasonable module size
        let image = code
            .render::<Luma<u8>>()
            .min_dimensions(400, 400) // Minimum size for reliable scanning
            .build();

        Ok(DynamicImage::ImageLuma8(image))
    }

    /// Encode a string into a QR code image
    pub fn encode_string(&self, data: &str) -> Result<DynamicImage> {
        let payload = QrPayload::from_string(data.to_string());
        self.encode(&payload)
    }

    /// Encode bytes into a QR code image
    pub fn encode_bytes(&self, data: &[u8]) -> Result<DynamicImage> {
        let payload = QrPayload::from_bytes(data.to_vec());
        self.encode(&payload)
    }
}

impl Default for QrEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        let _encoder = QrEncoder::new();
    }

    #[test]
    fn test_encode_string() {
        let encoder = QrEncoder::new();
        let result = encoder.encode_string("Hello, Keystone!");
        assert!(result.is_ok());
    }

    #[test]
    fn test_encode_bytes() {
        let encoder = QrEncoder::new();
        let result = encoder.encode_bytes(b"Binary data");
        assert!(result.is_ok());
    }

    #[test]
    fn test_round_trip() {
        use crate::qr::QrDecoder;

        let encoder = QrEncoder::new();
        let decoder = QrDecoder::new();

        let original = "Test payload for round trip";
        let qr_image = encoder.encode_string(original).unwrap();
        let decoded = decoder.decode(&qr_image).unwrap();

        assert_eq!(decoded.as_str(), Some(original));
    }
}
