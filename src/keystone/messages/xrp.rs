//! XRP (Ripple) message types
//!
//! XRP uses JSON-wrapped format (not standard CBOR)
//! Reference: Keystone SDK uses JSON serialization

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// XRP signature request (JSON format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrpSignRequest {
    /// Request ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Uuid>,
    /// XRP transaction JSON
    pub transaction_json: String,
    /// BIP44 derivation path (e.g., "m/44'/144'/0'/0/0")
    pub derivation_path: String,
    /// Optional origin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

impl XrpSignRequest {
    /// Create a new XRP signing request
    pub fn new(
        transaction_json: String,
        derivation_path: String,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            request_id,
            transaction_json,
            derivation_path,
            origin: None,
        }
    }

    /// Set the origin
    pub fn with_origin(mut self, origin: String) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Encode to JSON bytes (for UR wrapping)
    pub fn to_json_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| Error::Cbor(format!("JSON encoding failed: {}", e)))
    }

    /// Decode from JSON bytes
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes)
            .map_err(|e| Error::Cbor(format!("JSON decoding failed: {}", e)))
    }
}

/// XRP signature response (hex-encoded signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrpSignature {
    /// Request ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Uuid>,
    /// Hex-encoded signature
    pub signature: String,
}

impl XrpSignature {
    /// Create a new signature
    pub fn new(request_id: Option<Uuid>, signature: String) -> Self {
        Self {
            request_id,
            signature,
        }
    }

    /// Get the signature hex string
    pub fn signature(&self) -> &str {
        &self.signature
    }

    /// Encode to JSON bytes
    pub fn to_json_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| Error::Cbor(format!("JSON encoding failed: {}", e)))
    }

    /// Decode from JSON bytes
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes)
            .map_err(|e| Error::Cbor(format!("JSON decoding failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xrp_sign_request_json() {
        let request_id = Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap();
        let tx_json = r#"{"TransactionType":"Payment","Account":"rN7n7otQDd6FczFgLdlqtyMVrn3HMbjGVBMgr":"rLHzPsX6oXkzU9TzUMNm7cPxqhDuQNp2QN","Amount":"1000000"}"#.to_string();
        let path = "m/44'/144'/0'/0/0".to_string();

        let request = XrpSignRequest::new(tx_json.clone(), path.clone(), Some(request_id));

        // Encode to JSON
        let json_bytes = request.to_json_bytes().unwrap();
        assert!(!json_bytes.is_empty());

        // Decode
        let decoded = XrpSignRequest::from_json_bytes(&json_bytes).unwrap();
        assert_eq!(decoded.request_id, Some(request_id));
        assert_eq!(decoded.transaction_json, tx_json);
        assert_eq!(decoded.derivation_path, path);
    }

    #[test]
    fn test_xrp_signature_json() {
        let request_id = Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap();
        let signature_hex = "304402201234567890abcdef".to_string();

        let sig = XrpSignature::new(Some(request_id), signature_hex.clone());

        // Encode
        let json_bytes = sig.to_json_bytes().unwrap();
        assert!(!json_bytes.is_empty());

        // Decode
        let decoded = XrpSignature::from_json_bytes(&json_bytes).unwrap();
        assert_eq!(decoded.request_id, Some(request_id));
        assert_eq!(decoded.signature, signature_hex);
    }

    #[test]
    fn test_xrp_with_origin() {
        let tx_json = "{}".to_string();
        let request = XrpSignRequest::new(tx_json, "m/44'/144'/0'/0/0".to_string(), None)
            .with_origin("xumm".to_string());

        let json_bytes = request.to_json_bytes().unwrap();
        let decoded = XrpSignRequest::from_json_bytes(&json_bytes).unwrap();
        assert_eq!(decoded.origin, Some("xumm".to_string()));
    }
}
