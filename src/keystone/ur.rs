//! UR (Uniform Resources) encoding/decoding using the ur-rs library
//!
//! Reference: https://github.com/BlockchainCommons/Research/blob/master/papers/bcr-2020-005-ur.md

use crate::error::{Error, Result};
use crate::keystone::types::{KeystoneMetadata, KeystonePayload, PayloadEncoding};

// Re-export Decoder for multi-part decoding
pub use ur::Decoder;

/// Decode a UR string (single or multi-part)
pub fn decode_ur(ur_string: &str) -> Result<KeystonePayload> {
    let (_kind, data) = ur::decode(ur_string)
        .map_err(|e| Error::UrParse(format!("Failed to decode UR: {:?}", e)))?;

    // Extract type from the UR string
    let ur_type = extract_ur_type(ur_string)?;

    Ok(KeystonePayload {
        encoding: payload_encoding(&ur_type),
        metadata: KeystoneMetadata::default(),
        ur_type,
        data,
    })
}

/// Extract the UR type from a UR string
pub(crate) fn extract_ur_type(ur_string: &str) -> Result<String> {
    if !ur_string.starts_with("ur:") {
        return Err(Error::UrParse("Not a UR string".to_string()));
    }

    let without_prefix = &ur_string[3..];
    let parts: Vec<&str> = without_prefix.split('/').collect();

    if parts.is_empty() {
        return Err(Error::UrParse("Invalid UR format".to_string()));
    }

    Ok(parts[0].to_string())
}

pub(crate) fn payload_encoding(ur_type: &str) -> PayloadEncoding {
    match ur_type {
        t if t.starts_with("xrp-") => PayloadEncoding::Json,
        _ => PayloadEncoding::Cbor,
    }
}

/// Encode data as a single-part UR string
pub fn encode_ur(ur_type: &str, data: &[u8]) -> String {
    ur::encode(data, ur_type)
}

/// Encode data as UR with multi-part support
///
/// Returns (parts, is_multipart)
pub fn encode_ur_with_fragments(
    ur_type: &str,
    data: &[u8],
    max_fragment_len: usize,
) -> Result<(Vec<String>, bool)> {
    // Try single-part first
    let single = encode_ur(ur_type, data);

    if single.len() <= max_fragment_len {
        return Ok((vec![single], false));
    }

    // Need multi-part
    let mut encoder = ur::Encoder::new(data, max_fragment_len, ur_type)
        .map_err(|e| Error::UrParse(format!("Failed to create encoder: {:?}", e)))?;

    // Generate all parts
    let mut ur_parts = Vec::new();
    let total_parts = encoder.fragment_count();

    for _ in 0..total_parts {
        let part = encoder
            .next_part()
            .map_err(|e| Error::UrParse(format!("Failed to get next part: {:?}", e)))?;
        ur_parts.push(part);
    }

    Ok((ur_parts, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_ur() {
        let ur_type = "crypto-account";
        let data = vec![0x01, 0x02, 0x03, 0x04];

        let encoded = encode_ur(ur_type, &data);
        assert!(encoded.starts_with("ur:crypto-account/"));

        let decoded = decode_ur(&encoded).unwrap();
        assert_eq!(decoded.ur_type, ur_type);
        assert_eq!(decoded.data, data);
    }

    #[test]
    fn test_invalid_ur() {
        let result = decode_ur("not-a-ur");
        assert!(result.is_err());
    }

    #[test]
    fn test_single_part_encoding() {
        let data = vec![1, 2, 3, 4, 5];
        let (parts, is_multipart) = encode_ur_with_fragments("test-type", &data, 1000).unwrap();

        assert!(!is_multipart);
        assert_eq!(parts.len(), 1);
    }

    #[test]
    fn test_multi_part_encoding() {
        // Create data large enough to require multiple parts
        let data = vec![0u8; 200];
        let (parts, is_multipart) = encode_ur_with_fragments("test-type", &data, 50).unwrap();

        assert!(is_multipart);
        assert!(parts.len() > 1);
    }
}
