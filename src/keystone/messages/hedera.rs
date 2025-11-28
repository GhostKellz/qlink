//! Hedera (HBAR) signature request and response types (for Helix wallet!)
//!
//! Note: Hedera support is custom implementation for Helix wallet.
//! Using tag numbers 9001/9002 for HBAR messages.

use crate::error::Result;
use crate::keystone::cbor;
use crate::keystone::crypto_keypath::CryptoKeyPath;
use minicbor::{Decoder, Encoder};
use uuid::Uuid;

/// CBOR tags for Hedera messages (custom range)
pub mod tags {
    /// Hedera sign request tag
    pub const HEDERA_SIGN_REQUEST: u64 = 9001;
    /// Hedera signature tag
    pub const HEDERA_SIGNATURE: u64 = 9002;
}

/// Hedera signature request
#[derive(Debug, Clone)]
pub struct HederaSignRequest {
    /// Request ID (UUID)
    pub request_id: Option<Uuid>,
    /// Hedera protobuf transaction bytes
    pub transaction_bytes: Vec<u8>,
    /// BIP32 derivation path (typically m/44'/3030'/0'/0/0 for Hedera)
    pub derivation_path: CryptoKeyPath,
    /// Hedera account ID (e.g., "0.0.12345")
    pub account_id: Option<String>,
    /// Optional origin (e.g., "helix")
    pub origin: Option<String>,
}

impl HederaSignRequest {
    /// Create a new Hedera transaction signing request
    pub fn new(
        transaction_bytes: Vec<u8>,
        derivation_path: CryptoKeyPath,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            request_id,
            transaction_bytes,
            derivation_path,
            account_id: None,
            origin: None,
        }
    }

    /// Set the Hedera account ID
    pub fn with_account_id(mut self, account_id: String) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Set the origin
    pub fn with_origin(mut self, origin: String) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Encode to CBOR bytes
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        cbor::to_bytes(self)
    }

    /// Decode from CBOR bytes
    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        cbor::from_bytes(bytes)
    }

    /// Get map size for CBOR encoding
    fn map_size(&self) -> u64 {
        let mut size = 2; // transaction_bytes, derivation_path
        if self.request_id.is_some() {
            size += 1;
        }
        if self.account_id.is_some() {
            size += 1;
        }
        if self.origin.is_some() {
            size += 1;
        }
        size
    }
}

impl minicbor::Encode<()> for HederaSignRequest {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        e.map(self.map_size())?;

        // Key 1: request_id (optional UUID with tag 37)
        if let Some(ref uuid) = self.request_id {
            e.u8(1)?;
            e.tag(minicbor::data::Tag::Unassigned(cbor::tags::UUID))?;
            e.bytes(uuid.as_bytes())?;
        }

        // Key 2: transaction_bytes
        e.u8(2)?;
        e.bytes(&self.transaction_bytes)?;

        // Key 3: derivation_path (with tag 304)
        e.u8(3)?;
        e.tag(minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH))?;
        self.derivation_path.encode(e, ctx)?;

        // Key 4: account_id (optional)
        if let Some(ref account_id) = self.account_id {
            e.u8(4)?;
            e.str(account_id)?;
        }

        // Key 5: origin (optional)
        if let Some(ref origin) = self.origin {
            e.u8(5)?;
            e.str(origin)?;
        }

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for HederaSignRequest {
    fn decode(
        d: &mut Decoder<'b>,
        ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        let mut request_id = None;
        let mut transaction_bytes = None;
        let mut derivation_path = None;
        let mut account_id = None;
        let mut origin = None;

        let map_len = d.map()?.ok_or_else(|| {
            minicbor::decode::Error::message("expected definite-length map for HederaSignRequest")
        })?;

        for _ in 0..map_len {
            let key = d.u8()?;
            match key {
                1 => {
                    // request_id (UUID with tag 37)
                    let tag = d.tag()?;
                    if tag != minicbor::data::Tag::Unassigned(cbor::tags::UUID) {
                        return Err(minicbor::decode::Error::message(
                            "expected UUID tag for request_id",
                        ));
                    }
                    let uuid_bytes = d.bytes()?;
                    request_id = Some(Uuid::from_slice(uuid_bytes).map_err(|e| {
                        minicbor::decode::Error::message(format!("invalid UUID: {}", e))
                    })?);
                }
                2 => {
                    // transaction_bytes
                    transaction_bytes = Some(d.bytes()?.to_vec());
                }
                3 => {
                    // derivation_path (with tag 304)
                    let tag = d.tag()?;
                    if tag != minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH) {
                        return Err(minicbor::decode::Error::message(
                            "expected CRYPTO_KEYPATH tag for derivation_path",
                        ));
                    }
                    derivation_path = Some(CryptoKeyPath::decode(d, ctx)?);
                }
                4 => {
                    // account_id
                    account_id = Some(d.str()?.to_string());
                }
                5 => {
                    // origin
                    origin = Some(d.str()?.to_string());
                }
                _ => {
                    // Skip unknown keys
                    d.skip()?;
                }
            }
        }

        Ok(Self {
            request_id,
            transaction_bytes: transaction_bytes
                .ok_or_else(|| minicbor::decode::Error::message("missing transaction_bytes"))?,
            derivation_path: derivation_path
                .ok_or_else(|| minicbor::decode::Error::message("missing derivation_path"))?,
            account_id,
            origin,
        })
    }
}

/// Hedera signature response
#[derive(Debug, Clone)]
pub struct HederaSignature {
    /// Request ID that this signature corresponds to
    pub request_id: Option<Uuid>,
    /// Ed25519 signature (64 bytes for Hedera)
    pub signature: Vec<u8>,
    /// Optional public key (32 bytes Ed25519 public key)
    pub public_key: Option<Vec<u8>>,
}

impl HederaSignature {
    /// Create a new signature
    pub fn new(request_id: Option<Uuid>, signature: Vec<u8>) -> Self {
        Self {
            request_id,
            signature,
            public_key: None,
        }
    }

    /// Set the public key
    pub fn with_public_key(mut self, public_key: Vec<u8>) -> Self {
        self.public_key = Some(public_key);
        self
    }

    /// Get the signature bytes
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    /// Get the public key if present
    pub fn public_key(&self) -> Option<&[u8]> {
        self.public_key.as_deref()
    }

    /// Encode to CBOR bytes
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        cbor::to_bytes(self)
    }

    /// Decode from CBOR bytes
    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        cbor::from_bytes(bytes)
    }

    /// Get map size for CBOR encoding
    fn map_size(&self) -> u64 {
        let mut size = 1; // signature
        if self.request_id.is_some() {
            size += 1;
        }
        if self.public_key.is_some() {
            size += 1;
        }
        size
    }
}

impl minicbor::Encode<()> for HederaSignature {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        e.map(self.map_size())?;

        // Key 1: request_id (optional UUID with tag 37)
        if let Some(ref uuid) = self.request_id {
            e.u8(1)?;
            e.tag(minicbor::data::Tag::Unassigned(cbor::tags::UUID))?;
            e.bytes(uuid.as_bytes())?;
        }

        // Key 2: signature
        e.u8(2)?;
        e.bytes(&self.signature)?;

        // Key 3: public_key (optional)
        if let Some(ref public_key) = self.public_key {
            e.u8(3)?;
            e.bytes(public_key)?;
        }

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for HederaSignature {
    fn decode(
        d: &mut Decoder<'b>,
        _ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        let mut request_id = None;
        let mut signature = None;
        let mut public_key = None;

        let map_len = d.map()?.ok_or_else(|| {
            minicbor::decode::Error::message("expected definite-length map for HederaSignature")
        })?;

        for _ in 0..map_len {
            let key = d.u8()?;
            match key {
                1 => {
                    // request_id (UUID with tag 37)
                    let tag = d.tag()?;
                    if tag != minicbor::data::Tag::Unassigned(cbor::tags::UUID) {
                        return Err(minicbor::decode::Error::message(
                            "expected UUID tag for request_id",
                        ));
                    }
                    let uuid_bytes = d.bytes()?;
                    request_id = Some(Uuid::from_slice(uuid_bytes).map_err(|e| {
                        minicbor::decode::Error::message(format!("invalid UUID: {}", e))
                    })?);
                }
                2 => {
                    // signature
                    signature = Some(d.bytes()?.to_vec());
                }
                3 => {
                    // public_key
                    public_key = Some(d.bytes()?.to_vec());
                }
                _ => {
                    // Skip unknown keys
                    d.skip()?;
                }
            }
        }

        Ok(Self {
            request_id,
            signature: signature
                .ok_or_else(|| minicbor::decode::Error::message("missing signature"))?,
            public_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hedera_sign_request_cbor() {
        // Hedera uses m/44'/3030'/0'/0/0
        let path = CryptoKeyPath::from_str("m/44'/3030'/0'/0/0").unwrap();
        let request_id = Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap();
        let tx_bytes = vec![0x0a, 0x10, 0x1a, 0x20]; // Mock Hedera protobuf

        let request = HederaSignRequest::new(tx_bytes.clone(), path.clone(), Some(request_id))
            .with_account_id("0.0.12345".to_string())
            .with_origin("helix".to_string());

        // Encode
        let cbor_bytes = request.to_cbor().unwrap();
        assert!(!cbor_bytes.is_empty());

        // Decode
        let decoded = HederaSignRequest::from_cbor(&cbor_bytes).unwrap();
        assert_eq!(decoded.request_id, Some(request_id));
        assert_eq!(decoded.transaction_bytes, tx_bytes);
        assert_eq!(decoded.derivation_path.to_string(), path.to_string());
        assert_eq!(decoded.account_id, Some("0.0.12345".to_string()));
        assert_eq!(decoded.origin, Some("helix".to_string()));
    }

    #[test]
    fn test_hedera_signature_cbor() {
        let request_id = Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap();
        let signature = vec![0xbb; 64]; // Mock Ed25519 signature
        let public_key = vec![0xcc; 32]; // Mock Ed25519 public key

        let sig = HederaSignature::new(Some(request_id), signature.clone())
            .with_public_key(public_key.clone());

        // Encode
        let cbor_bytes = sig.to_cbor().unwrap();
        assert!(!cbor_bytes.is_empty());

        // Decode
        let decoded = HederaSignature::from_cbor(&cbor_bytes).unwrap();
        assert_eq!(decoded.request_id, Some(request_id));
        assert_eq!(decoded.signature, signature);
        assert_eq!(decoded.public_key, Some(public_key));
    }

    #[test]
    fn test_hedera_path() {
        // Hedera uses BIP44 with coin type 3030
        let path = CryptoKeyPath::from_str("m/44'/3030'/0'/0/0").unwrap();
        assert_eq!(path.to_string(), "m/44'/3030'/0'/0/0");
    }
}
