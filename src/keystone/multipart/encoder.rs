//! Multi-part QR encoder using ur crate

use crate::error::Result;
use crate::keystone::ur;

/// Multi-part QR encoder with cyclic iteration
pub struct MultiPartEncoder {
    parts: Vec<String>,
    current_index: usize,
}

/// Result of encoding operation
#[derive(Debug, Clone)]
pub struct EncodeResult {
    /// The UR string for this part
    pub ur_string: String,
    /// Current part number (1-indexed)
    pub part_num: usize,
    /// Total number of parts
    pub total_parts: usize,
    /// Whether this is a multi-part encoding
    pub is_multipart: bool,
}

impl MultiPartEncoder {
    /// Create a new multi-part encoder
    pub fn new(ur_type: &str, data: &[u8], max_fragment_len: usize) -> Result<Self> {
        let (parts, _is_multipart) = ur::encode_ur_with_fragments(ur_type, data, max_fragment_len)?;

        Ok(Self {
            parts,
            current_index: 0,
        })
    }

    /// Check if this is a multi-part encoding
    pub fn is_multipart(&self) -> bool {
        self.parts.len() > 1
    }

    /// Get the total number of parts
    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    /// Get the next part (cyclic - wraps around)
    pub fn next_part(&mut self) -> EncodeResult {
        let part = &self.parts[self.current_index];
        let result = EncodeResult {
            ur_string: part.clone(),
            part_num: self.current_index + 1,
            total_parts: self.parts.len(),
            is_multipart: self.is_multipart(),
        };

        // Advance to next part (cyclic)
        self.current_index = (self.current_index + 1) % self.parts.len();

        result
    }

    /// Get a specific part by index (0-based)
    pub fn part_at(&self, index: usize) -> Option<EncodeResult> {
        if index >= self.parts.len() {
            return None;
        }

        Some(EncodeResult {
            ur_string: self.parts[index].clone(),
            part_num: index + 1,
            total_parts: self.parts.len(),
            is_multipart: self.is_multipart(),
        })
    }

    /// Get all parts as a vector
    pub fn all_parts(&self) -> Vec<String> {
        self.parts.clone()
    }

    /// Reset the cyclic counter to the beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_part() {
        let data = b"Small data";
        let encoder = MultiPartEncoder::new("test", data, 1000).unwrap();

        assert!(!encoder.is_multipart());
        assert_eq!(encoder.part_count(), 1);
    }

    #[test]
    fn test_reset() {
        let data = b"test data";
        let mut encoder = MultiPartEncoder::new("test", data, 1000).unwrap();

        // Advance
        encoder.next_part();

        // Reset
        encoder.reset();

        // Should be back at part 1
        let part = encoder.next_part();
        assert_eq!(part.part_num, 1);
    }
}
