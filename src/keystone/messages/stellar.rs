//! Stellar (XLM) signature request and response types
//!
//! Reference: https://github.com/KeystoneHQ/keystone-sdk-rust

use crate::error::{Error, Result};
use crate::keystone::cbor;
use crate::keystone::crypto_keypath::CryptoKeyPath;
use minicbor::{Decoder, Encoder};
use uuid::Uuid;

/// Stellar signature type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StellarSignType {
    /// Full transaction
    Transaction = 1,
    /// Transaction hash only
    TransactionHash = 2,
    /// Message signing
    Message = 3,
}

impl Default for StellarSignType {
    fn default() -> Self {
        Self::Transaction
    }
}

impl StellarSignType {
    /// Create from u32
    pub fn from_u32(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::Transaction),
            2 => Ok(Self::TransactionHash),
            3 => Ok(Self::Message),
            _ => Err(Error::Cbor(format!(
                "Invalid Stellar sign type: {}, expected 1, 2, or 3",
                value
            ))),
        }
    }

    /// Convert to u32
    pub fn to_u32(&self) -> u32 {
        match self {
            Self::Transaction => 1,
            Self::TransactionHash => 2,
            Self::Message => 3,
        }
    }
}

/// Stellar signature request
#[derive(Debug, Clone)]
pub struct StellarSignRequest {
    /// Request ID (UUID)
    pub request_id: Option<Uuid>,
    /// Transaction data or hash to sign
    pub sign_data: Vec<u8>,
    /// BIP32 derivation path
    pub derivation_path: CryptoKeyPath,
    /// Type of signing operation
    pub sign_type: StellarSignType,
    /// Optional Stellar address
    pub address: Option<Vec<u8>>,
    /// Optional origin (e.g., "lobstr", "freighter")
    pub origin: Option<String>,
}

impl StellarSignRequest {
    /// Create a new transaction signing request
    pub fn new_transaction(
        sign_data: Vec<u8>,
        derivation_path: CryptoKeyPath,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            request_id,
            sign_data,
            derivation_path,
            sign_type: StellarSignType::Transaction,
            address: None,
            origin: None,
        }
    }

    /// Create a new transaction hash signing request
    pub fn new_transaction_hash(
        tx_hash: Vec<u8>,
        derivation_path: CryptoKeyPath,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            request_id,
            sign_data: tx_hash,
            derivation_path,
            sign_type: StellarSignType::TransactionHash,
            address: None,
            origin: None,
        }
    }

    /// Create a new message signing request
    pub fn new_message(
        message: Vec<u8>,
        derivation_path: CryptoKeyPath,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            request_id,
            sign_data: message,
            derivation_path,
            sign_type: StellarSignType::Message,
            address: None,
            origin: None,
        }
    }

    /// Set the address
    pub fn with_address(mut self, address: Vec<u8>) -> Self {
        self.address = Some(address);
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
        let mut size = 3; // sign_data, derivation_path, sign_type
        if self.request_id.is_some() {
            size += 1;
        }
        if self.address.is_some() {
            size += 1;
        }
        if self.origin.is_some() {
            size += 1;
        }
        size
    }
}

impl minicbor::Encode<()> for StellarSignRequest {
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

        // Key 2: sign_data
        e.u8(2)?;
        e.bytes(&self.sign_data)?;

        // Key 3: derivation_path (with tag 304)
        e.u8(3)?;
        e.tag(minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH))?;
        self.derivation_path.encode(e, ctx)?;

        // Key 4: address (optional)
        if let Some(ref address) = self.address {
            e.u8(4)?;
            e.bytes(address)?;
        }

        // Key 5: origin (optional)
        if let Some(ref origin) = self.origin {
            e.u8(5)?;
            e.str(origin)?;
        }

        // Key 6: sign_type
        e.u8(6)?;
        e.u32(self.sign_type.to_u32())?;

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for StellarSignRequest {
    fn decode(
        d: &mut Decoder<'b>,
        ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        let mut request_id = None;
        let mut sign_data = None;
        let mut derivation_path = None;
        let mut sign_type = None;
        let mut address = None;
        let mut origin = None;

        let map_len = d.map()?.ok_or_else(|| {
            minicbor::decode::Error::message("expected definite-length map for StellarSignRequest")
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
                    // sign_data
                    sign_data = Some(d.bytes()?.to_vec());
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
                    // address
                    address = Some(d.bytes()?.to_vec());
                }
                5 => {
                    // origin
                    origin = Some(d.str()?.to_string());
                }
                6 => {
                    // sign_type
                    let type_val = d.u32()?;
                    sign_type = Some(StellarSignType::from_u32(type_val).map_err(|e| {
                        minicbor::decode::Error::message(format!("invalid sign_type: {}", e))
                    })?);
                }
                _ => {
                    // Skip unknown keys
                    d.skip()?;
                }
            }
        }

        Ok(Self {
            request_id,
            sign_data: sign_data
                .ok_or_else(|| minicbor::decode::Error::message("missing sign_data"))?,
            derivation_path: derivation_path
                .ok_or_else(|| minicbor::decode::Error::message("missing derivation_path"))?,
            sign_type: sign_type
                .ok_or_else(|| minicbor::decode::Error::message("missing sign_type"))?,
            address,
            origin,
        })
    }
}

/// Stellar signature response
#[derive(Debug, Clone)]
pub struct StellarSignature {
    /// Request ID that this signature corresponds to
    pub request_id: Option<Uuid>,
    /// Ed25519 signature (64 bytes)
    pub signature: Vec<u8>,
}

impl StellarSignature {
    /// Create a new signature
    pub fn new(request_id: Option<Uuid>, signature: Vec<u8>) -> Self {
        Self {
            request_id,
            signature,
        }
    }

    /// Get the signature bytes
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    /// Encode to CBOR bytes
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        cbor::to_bytes(self)
    }

    /// Decode from CBOR bytes
    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        cbor::from_bytes(bytes)
    }
}

impl minicbor::Encode<()> for StellarSignature {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        let map_size = if self.request_id.is_some() { 2 } else { 1 };
        e.map(map_size)?;

        // Key 1: request_id (optional UUID with tag 37)
        if let Some(ref uuid) = self.request_id {
            e.u8(1)?;
            e.tag(minicbor::data::Tag::Unassigned(cbor::tags::UUID))?;
            e.bytes(uuid.as_bytes())?;
        }

        // Key 2: signature
        e.u8(2)?;
        e.bytes(&self.signature)?;

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for StellarSignature {
    fn decode(
        d: &mut Decoder<'b>,
        _ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        let mut request_id = None;
        let mut signature = None;

        let map_len = d.map()?.ok_or_else(|| {
            minicbor::decode::Error::message("expected definite-length map for StellarSignature")
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stellar_sign_request_cbor() {
        let path = CryptoKeyPath::from_str("m/44'/148'/0'").unwrap();
        let request_id = Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap();
        let sign_data = vec![1, 2, 3, 4, 5];

        let request =
            StellarSignRequest::new_transaction(sign_data.clone(), path.clone(), Some(request_id));

        // Encode
        let cbor_bytes = request.to_cbor().unwrap();
        assert!(!cbor_bytes.is_empty());

        // Decode
        let decoded = StellarSignRequest::from_cbor(&cbor_bytes).unwrap();
        assert_eq!(decoded.request_id, Some(request_id));
        assert_eq!(decoded.sign_data, sign_data);
        assert_eq!(decoded.derivation_path.to_string(), path.to_string());
        assert_eq!(decoded.sign_type, StellarSignType::Transaction);
    }

    #[test]
    fn test_stellar_sign_request_with_origin() {
        let path = CryptoKeyPath::from_str("m/44'/148'/0'").unwrap();
        let request_id = Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap();
        let sign_data = vec![1, 2, 3, 4, 5];

        let request = StellarSignRequest::new_transaction(sign_data, path, Some(request_id))
            .with_origin("lobstr".to_string());

        let cbor_bytes = request.to_cbor().unwrap();
        let decoded = StellarSignRequest::from_cbor(&cbor_bytes).unwrap();
        assert_eq!(decoded.origin, Some("lobstr".to_string()));
    }

    #[test]
    fn test_stellar_signature_cbor() {
        let request_id = Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap();
        let signature = vec![0xaa; 64]; // Mock 64-byte signature

        let sig = StellarSignature::new(Some(request_id), signature.clone());

        // Encode
        let cbor_bytes = sig.to_cbor().unwrap();
        assert!(!cbor_bytes.is_empty());

        // Decode
        let decoded = StellarSignature::from_cbor(&cbor_bytes).unwrap();
        assert_eq!(decoded.request_id, Some(request_id));
        assert_eq!(decoded.signature, signature);
    }

    #[test]
    fn test_stellar_sign_types() {
        assert_eq!(
            StellarSignType::from_u32(1).unwrap(),
            StellarSignType::Transaction
        );
        assert_eq!(
            StellarSignType::from_u32(2).unwrap(),
            StellarSignType::TransactionHash
        );
        assert_eq!(
            StellarSignType::from_u32(3).unwrap(),
            StellarSignType::Message
        );
        assert!(StellarSignType::from_u32(99).is_err());

        assert_eq!(StellarSignType::Transaction.to_u32(), 1);
        assert_eq!(StellarSignType::TransactionHash.to_u32(), 2);
        assert_eq!(StellarSignType::Message.to_u32(), 3);
    }
}
