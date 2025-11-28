//! Multi-part QR decoder using ur crate

use crate::error::{Error, Result};
use crate::keystone::{KeystoneMetadata, KeystonePayload, ur};
use std::collections::HashSet;

/// Multi-part QR decoder
pub struct MultiPartDecoder {
    decoder: Option<ur::Decoder>,
    received_parts: HashSet<String>,
    single_part_result: Option<KeystonePayload>,
}

/// Decoding progress information
#[derive(Debug, Clone)]
pub struct DecodeProgress {
    /// Number of parts received so far
    pub parts_received: usize,
    /// Total number of parts (if known)
    pub total_parts: Option<usize>,
    /// Progress percentage (0-100)
    pub percentage: u8,
    /// Whether decoding is complete
    pub complete: bool,
}

impl DecodeProgress {
    /// Check if decoding is complete
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Get a progress message
    pub fn message(&self) -> String {
        if self.complete {
            "Complete!".to_string()
        } else if let Some(total) = self.total_parts {
            format!(
                "Received {}/{} parts ({}%)",
                self.parts_received, total, self.percentage
            )
        } else {
            format!("Received {} parts...", self.parts_received)
        }
    }
}

impl MultiPartDecoder {
    /// Create a new multi-part decoder
    pub fn new() -> Self {
        Self {
            decoder: Some(ur::Decoder::default()),
            received_parts: HashSet::new(),
            single_part_result: None,
        }
    }

    /// Receive a UR string part
    pub fn receive(&mut self, ur_string: &str) -> Result<DecodeProgress> {
        // Track unique parts
        let is_new = self.received_parts.insert(ur_string.to_string());

        if !is_new {
            // Already received this part
            return self.current_progress();
        }

        // Try single-part decode first (if this is the first part)
        if self.received_parts.len() == 1 {
            match ur::decode_ur(ur_string) {
                Ok(payload) => {
                    // This is a single-part UR
                    self.single_part_result = Some(payload);
                    return Ok(DecodeProgress {
                        parts_received: 1,
                        total_parts: Some(1),
                        percentage: 100,
                        complete: true,
                    });
                }
                Err(_) => {
                    // Not a single-part, continue with multi-part decoder
                }
            }
        }

        let decoder = self
            .decoder
            .as_mut()
            .ok_or_else(|| Error::UrParse("Decoder not initialized".to_string()))?;

        // Try to receive this part as multi-part
        decoder
            .receive(ur_string)
            .map_err(|e| Error::UrParse(format!("Failed to receive part: {:?}", e)))?;

        self.current_progress()
    }

    fn current_progress(&self) -> Result<DecodeProgress> {
        let decoder = self
            .decoder
            .as_ref()
            .ok_or_else(|| Error::UrParse("Decoder not initialized".to_string()))?;

        let complete = decoder.complete();
        let parts_received = self.received_parts.len();

        // Get progress from the ur::Decoder (0-99, or 100 when complete)
        let percentage = if complete { 100 } else { decoder.progress() };

        // Total parts is not available from ur::Decoder API
        // It uses fountain codes which don't have a fixed total
        let total_parts = None;

        Ok(DecodeProgress {
            parts_received,
            total_parts,
            percentage,
            complete,
        })
    }

    /// Get the decoded result (only call when `is_complete()` returns true)
    pub fn result(&self) -> Result<KeystonePayload> {
        // Check if we have a single-part result
        if let Some(ref payload) = self.single_part_result {
            return Ok(payload.clone());
        }

        // Otherwise, use multi-part decoder
        let decoder = self
            .decoder
            .as_ref()
            .ok_or_else(|| Error::UrParse("No decoder initialized".to_string()))?;

        if !decoder.complete() {
            return Err(Error::UrParse("Decoding not complete".to_string()));
        }

        let message = decoder
            .message()
            .map_err(|e| Error::UrParse(format!("Failed to get message: {:?}", e)))?
            .ok_or_else(|| Error::UrParse("No message available".to_string()))?;

        // Extract type from any received part
        let first_part = self
            .received_parts
            .iter()
            .next()
            .ok_or_else(|| Error::UrParse("No UR fragments stored".to_string()))?;

        let ur_type = ur::extract_ur_type(first_part).unwrap_or_else(|_| "unknown".to_string());

        let (sequence, total_parts) = parse_fragment_metadata(first_part);

        Ok(KeystonePayload {
            ur_type: ur_type.clone(),
            data: message,
            metadata: KeystoneMetadata {
                sequence,
                total_parts,
                multipart: true,
            },
            encoding: ur::payload_encoding(&ur_type),
        })
    }

    /// Check if decoding is complete
    pub fn is_complete(&self) -> bool {
        // Check single-part result first
        if self.single_part_result.is_some() {
            return true;
        }

        // Otherwise check multi-part decoder
        self.decoder.as_ref().map(|d| d.complete()).unwrap_or(false)
    }

    /// Reset the decoder
    pub fn reset(&mut self) {
        self.decoder = Some(ur::Decoder::default());
        self.received_parts.clear();
        self.single_part_result = None;
    }
}

fn parse_fragment_metadata(fragment: &str) -> (Option<u32>, Option<u32>) {
    let stripped = fragment.strip_prefix("ur:").unwrap_or(fragment);
    let mut parts = stripped.split('/');
    let _ = parts.next(); // skip type
    if let Some(sequence_part) = parts.next() {
        if let Some((seq, total)) = sequence_part.split_once("of") {
            let seq = seq.parse::<u32>().ok();
            let total = total.parse::<u32>().ok();
            return (seq, total);
        }
        if let Some((seq, total)) = sequence_part.split_once('-') {
            let seq = seq.parse::<u32>().ok();
            let total = total.parse::<u32>().ok();
            return (seq, total);
        }
    }
    (None, None)
}

impl Default for MultiPartDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let decoder = MultiPartDecoder::new();
        assert!(!decoder.is_complete());
    }

    #[test]
    fn test_progress_message() {
        let progress = DecodeProgress {
            parts_received: 3,
            total_parts: Some(5),
            percentage: 60,
            complete: false,
        };

        let msg = progress.message();
        assert!(msg.contains("3/5"));
        assert!(msg.contains("60%"));
    }
}
