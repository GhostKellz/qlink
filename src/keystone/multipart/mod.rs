//! Multi-part QR code support with fountain codes
//!
//! Implements cyclic encoding for animated QR codes and progressive decoding.

mod decoder;
mod encoder;

pub use decoder::{DecodeProgress, MultiPartDecoder};
pub use encoder::{EncodeResult, MultiPartEncoder};
/// Default maximum fragment length (works well for most QR scanners)
pub const DEFAULT_MAX_FRAGMENT_LEN: usize = 400;

/// Recommended display time per frame (milliseconds)
pub const RECOMMENDED_FRAME_DELAY_MS: u64 = 150;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_part_flow() {
        let data = b"Hello, Keystone!";
        let encoder = MultiPartEncoder::new("test-type", data, 1000).unwrap();

        assert!(!encoder.is_multipart());
        assert_eq!(encoder.part_count(), 1);
    }

    #[test]
    fn test_multi_part_flow() {
        let data = vec![0u8; 500]; // Large enough for multi-part
        let encoder = MultiPartEncoder::new("test-type", &data, 100).unwrap();

        assert!(encoder.is_multipart());
        assert!(encoder.part_count() > 1);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let original_data = b"Test data for roundtrip";
        let encoder = MultiPartEncoder::new("test-type", original_data, 1000).unwrap();

        let mut decoder = MultiPartDecoder::new();

        // Feed all parts to decoder
        for part in encoder.all_parts() {
            let progress = decoder.receive(&part).unwrap();
            if progress.is_complete() {
                let decoded = decoder.result().unwrap();
                assert_eq!(decoded.ur_type, "test-type");
                assert_eq!(decoded.data, original_data);
                return;
            }
        }

        panic!("Decoder should have completed");
    }
}
