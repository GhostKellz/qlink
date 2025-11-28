//! QR code decoder using rqrr

use crate::error::{Error, Result};
use crate::qr::QrPayload;
use image::{DynamicImage, GrayImage};

/// QR code decoder
pub struct QrDecoder {
    // Configuration could go here (e.g., detection parameters)
}

impl QrDecoder {
    /// Create a new QR decoder with default settings
    pub fn new() -> Self {
        Self {}
    }

    /// Decode a QR code from an image
    pub fn decode(&self, img: &DynamicImage) -> Result<QrPayload> {
        // Convert to grayscale if needed
        let gray = img.to_luma8();

        self.decode_gray(&gray)
    }

    /// Decode a QR code from a grayscale image
    pub fn decode_gray(&self, img: &GrayImage) -> Result<QrPayload> {
        let mut prepared = rqrr::PreparedImage::prepare(img.clone());

        let grids = prepared.detect_grids();

        if grids.is_empty() {
            return Err(Error::NoQrCodeFound);
        }

        // Take the first detected QR code
        let grid = &grids[0];

        match grid.decode() {
            Ok((meta, content)) => {
                tracing::debug!(
                    "Decoded QR: version={:?}, ecc_level={:?}, length={}",
                    meta.version,
                    meta.ecc_level,
                    content.len()
                );

                Ok(QrPayload::from_bytes(content.into_bytes()))
            }
            Err(e) => Err(Error::QrDecode(format!("Decode failed: {:?}", e))),
        }
    }

    /// Decode multiple QR codes from an image
    pub fn decode_all(&self, img: &DynamicImage) -> Result<Vec<QrPayload>> {
        let gray = img.to_luma8();
        let mut prepared = rqrr::PreparedImage::prepare(gray);

        let grids = prepared.detect_grids();

        if grids.is_empty() {
            return Err(Error::NoQrCodeFound);
        }

        let mut payloads = Vec::new();

        for grid in grids {
            match grid.decode() {
                Ok((_meta, content)) => {
                    payloads.push(QrPayload::from_bytes(content.into_bytes()));
                }
                Err(e) => {
                    tracing::warn!("Failed to decode one QR code: {:?}", e);
                }
            }
        }

        if payloads.is_empty() {
            return Err(Error::QrDecode("No QR codes could be decoded".to_string()));
        }

        Ok(payloads)
    }
}

impl Default for QrDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let _decoder = QrDecoder::new();
    }
}
