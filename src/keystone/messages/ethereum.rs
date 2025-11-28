//! Ethereum message types
//!
//! Reference: archive/keystone-sdk-rust/libs/ur-registry/src/ethereum/

use crate::error::{Error, Result};
use crate::keystone::cbor;
use crate::keystone::crypto_keypath::CryptoKeyPath;
use minicbor::{Decoder, Encoder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of Ethereum data being signed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum EthDataType {
    /// Legacy transaction (RLP-encoded)
    Transaction = 1,
    /// EIP-712 typed data
    TypedData = 2,
    /// Personal message (eth_sign)
    PersonalMessage = 3,
    /// EIP-1559/2930 typed transaction
    TypedTransaction = 4,
}

impl From<EthDataType> for u8 {
    fn from(t: EthDataType) -> u8 {
        t as u8
    }
}

impl TryFrom<u8> for EthDataType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(EthDataType::Transaction),
            2 => Ok(EthDataType::TypedData),
            3 => Ok(EthDataType::PersonalMessage),
            4 => Ok(EthDataType::TypedTransaction),
            _ => Err(Error::InvalidKeystonePayload(format!(
                "Invalid ETH data type: {}",
                value
            ))),
        }
    }
}

/// Ethereum signature request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthSignRequest {
    /// Optional request ID for matching request/response
    pub request_id: Option<Uuid>,
    /// The data to sign (transaction bytes, message, etc.)
    pub sign_data: Vec<u8>,
    /// Type of data being signed
    pub data_type: EthDataType,
    /// Optional chain ID (1 = mainnet, 137 = Polygon, etc.)
    pub chain_id: Option<i128>,
    /// BIP32 derivation path
    pub derivation_path: CryptoKeyPath,
    /// Optional address (for verification)
    pub address: Option<Vec<u8>>,
    /// Optional origin (dApp identifier)
    pub origin: Option<String>,
}

impl EthSignRequest {
    /// Create a new transaction sign request
    pub fn new_transaction(
        sign_data: Vec<u8>,
        derivation_path: CryptoKeyPath,
        chain_id: Option<i128>,
    ) -> Self {
        Self {
            request_id: Some(Uuid::new_v4()),
            sign_data,
            data_type: EthDataType::Transaction,
            chain_id,
            derivation_path,
            address: None,
            origin: None,
        }
    }

    /// Create a new typed transaction sign request (EIP-1559)
    pub fn new_typed_transaction(
        sign_data: Vec<u8>,
        derivation_path: CryptoKeyPath,
        chain_id: Option<i128>,
    ) -> Self {
        Self {
            request_id: Some(Uuid::new_v4()),
            sign_data,
            data_type: EthDataType::TypedTransaction,
            chain_id,
            derivation_path,
            address: None,
            origin: None,
        }
    }

    /// Create a new personal message sign request
    pub fn new_personal_message(sign_data: Vec<u8>, derivation_path: CryptoKeyPath) -> Self {
        Self {
            request_id: Some(Uuid::new_v4()),
            sign_data,
            data_type: EthDataType::PersonalMessage,
            chain_id: None,
            derivation_path,
            address: None,
            origin: None,
        }
    }

    /// Set the origin (dApp URL)
    pub fn with_origin(mut self, origin: String) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Set the address
    pub fn with_address(mut self, address: Vec<u8>) -> Self {
        self.address = Some(address);
        self
    }

    /// Serialize the request as CBOR bytes
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        cbor::to_bytes(self)
    }

    /// Parse a request from CBOR bytes
    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        cbor::from_bytes(bytes)
    }
}

// CBOR encoding for EthSignRequest (tag 401)
impl minicbor::Encode<()> for EthSignRequest {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(minicbor::data::Tag::Unassigned(
            cbor::tags::ETH_SIGN_REQUEST,
        ))?;

        // Count map entries
        let mut size = 3; // sign_data, data_type, derivation_path
        if self.request_id.is_some() {
            size += 1;
        }
        if self.chain_id.is_some() {
            size += 1;
        }
        if self.address.is_some() {
            size += 1;
        }
        if self.origin.is_some() {
            size += 1;
        }

        e.map(size)?;

        // Key 1: request_id (optional UUID with tag 37)
        if let Some(ref uuid) = self.request_id {
            e.u64(1)?;
            e.tag(minicbor::data::Tag::Unassigned(cbor::tags::UUID))?;
            e.bytes(uuid.as_bytes())?;
        }

        // Key 2: sign_data
        e.u64(2)?.bytes(&self.sign_data)?;

        // Key 3: data_type
        e.u64(3)?.u8(self.data_type.into())?;

        // Key 4: chain_id (optional)
        if let Some(chain_id) = self.chain_id {
            e.u64(4)?;
            // Encode as signed integer
            if chain_id >= 0 {
                e.u64(chain_id as u64)?;
            } else {
                e.i64(chain_id as i64)?;
            }
        }

        // Key 5: derivation_path
        e.u64(5)?.encode(&self.derivation_path)?;

        // Key 6: address (optional)
        if let Some(ref address) = self.address {
            e.u64(6)?.bytes(address)?;
        }

        // Key 7: origin (optional)
        if let Some(ref origin) = self.origin {
            e.u64(7)?.str(origin)?;
        }

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for EthSignRequest {
    fn decode(
        d: &mut Decoder<'b>,
        _ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        // Expect tag 401
        let tag = d.tag()?;
        if tag != minicbor::data::Tag::Unassigned(cbor::tags::ETH_SIGN_REQUEST) {
            return Err(minicbor::decode::Error::message(
                "Expected eth-sign-request tag 401",
            ));
        }

        let map_len = d
            .map()?
            .ok_or_else(|| minicbor::decode::Error::message("Expected definite-length map"))?;

        let mut request_id = None;
        let mut sign_data = None;
        let mut data_type = None;
        let mut chain_id = None;
        let mut derivation_path = None;
        let mut address = None;
        let mut origin = None;

        for _ in 0..map_len {
            let key = d.u64()?;
            match key {
                1 => {
                    // request_id (UUID tag 37)
                    let tag = d.tag()?;
                    if tag == minicbor::data::Tag::Unassigned(cbor::tags::UUID) {
                        let bytes = d.bytes()?;
                        request_id = Uuid::from_slice(bytes).ok();
                    }
                }
                2 => sign_data = Some(d.bytes()?.to_vec()),
                3 => {
                    data_type = Some(
                        EthDataType::try_from(d.u8()?)
                            .map_err(|_| minicbor::decode::Error::message("Invalid data type"))?,
                    )
                }
                4 => {
                    // chain_id can be positive or negative
                    // Try as i64 first, fall back to u64
                    chain_id = d
                        .i64()
                        .map(|v| Some(v as i128))
                        .or_else(|_| d.u64().map(|v| Some(v as i128)))
                        .unwrap_or(None);
                }
                5 => derivation_path = Some(d.decode()?),
                6 => address = Some(d.bytes()?.to_vec()),
                7 => origin = Some(d.str()?.to_string()),
                _ => {
                    d.skip()?;
                }
            }
        }

        Ok(EthSignRequest {
            request_id,
            sign_data: sign_data
                .ok_or_else(|| minicbor::decode::Error::message("Missing sign_data"))?,
            data_type: data_type
                .ok_or_else(|| minicbor::decode::Error::message("Missing data_type"))?,
            chain_id,
            derivation_path: derivation_path
                .ok_or_else(|| minicbor::decode::Error::message("Missing derivation_path"))?,
            address,
            origin,
        })
    }
}

/// Ethereum signature (response from Keystone)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthSignature {
    /// Request ID (matches the request)
    pub request_id: Option<Uuid>,
    /// ECDSA signature (65 bytes: r + s + v)
    pub signature: Vec<u8>,
    /// Optional origin
    pub origin: Option<String>,
}

impl EthSignature {
    /// Create a new signature
    pub fn new(signature: Vec<u8>) -> Self {
        Self {
            request_id: None,
            signature,
            origin: None,
        }
    }

    /// Get the signature components (r, s, v)
    pub fn rsv(&self) -> Result<([u8; 32], [u8; 32], u8)> {
        if self.signature.len() != 65 {
            return Err(Error::InvalidKeystonePayload(format!(
                "Invalid signature length: {} (expected 65)",
                self.signature.len()
            )));
        }

        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&self.signature[0..32]);
        s.copy_from_slice(&self.signature[32..64]);
        let v = self.signature[64];

        Ok((r, s, v))
    }

    /// Serialize the signature as CBOR bytes
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        cbor::to_bytes(self)
    }

    /// Parse a signature from CBOR bytes
    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        cbor::from_bytes(bytes)
    }
}

// CBOR encoding for EthSignature (tag 402)
impl minicbor::Encode<()> for EthSignature {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(minicbor::data::Tag::Unassigned(cbor::tags::ETH_SIGNATURE))?;

        let mut size = 1; // signature is required
        if self.request_id.is_some() {
            size += 1;
        }
        if self.origin.is_some() {
            size += 1;
        }

        e.map(size)?;

        // Key 1: request_id (optional)
        if let Some(ref uuid) = self.request_id {
            e.u64(1)?;
            e.tag(minicbor::data::Tag::Unassigned(cbor::tags::UUID))?;
            e.bytes(uuid.as_bytes())?;
        }

        // Key 2: signature
        e.u64(2)?.bytes(&self.signature)?;

        // Key 3: origin (optional)
        if let Some(ref origin) = self.origin {
            e.u64(3)?.str(origin)?;
        }

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for EthSignature {
    fn decode(
        d: &mut Decoder<'b>,
        _ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        let tag = d.tag()?;
        if tag != minicbor::data::Tag::Unassigned(cbor::tags::ETH_SIGNATURE) {
            return Err(minicbor::decode::Error::message(
                "Expected eth-signature tag 402",
            ));
        }

        let map_len = d
            .map()?
            .ok_or_else(|| minicbor::decode::Error::message("Expected definite-length map"))?;

        let mut request_id = None;
        let mut signature = None;
        let mut origin = None;

        for _ in 0..map_len {
            let key = d.u64()?;
            match key {
                1 => {
                    let tag = d.tag()?;
                    if tag == minicbor::data::Tag::Unassigned(cbor::tags::UUID) {
                        let bytes = d.bytes()?;
                        request_id = Uuid::from_slice(bytes).ok();
                    }
                }
                2 => signature = Some(d.bytes()?.to_vec()),
                3 => origin = Some(d.str()?.to_string()),
                _ => {
                    d.skip()?;
                }
            }
        }

        Ok(EthSignature {
            request_id,
            signature: signature
                .ok_or_else(|| minicbor::decode::Error::message("Missing signature"))?,
            origin,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eth_sign_request_cbor() {
        let path = CryptoKeyPath::from_str("m/44'/60'/0'/0/0").unwrap();
        let request = EthSignRequest::new_transaction(
            vec![1, 2, 3, 4, 5],
            path,
            Some(1), // mainnet
        );

        let bytes = request.to_cbor().unwrap();
        let decoded = EthSignRequest::from_cbor(&bytes).unwrap();

        assert_eq!(decoded.sign_data, request.sign_data);
        assert_eq!(decoded.data_type, EthDataType::Transaction);
        assert_eq!(decoded.chain_id, Some(1));
    }

    #[test]
    fn test_eth_signature_cbor() {
        let sig = EthSignature::new(vec![0u8; 65]);

        let bytes = sig.to_cbor().unwrap();
        let decoded = EthSignature::from_cbor(&bytes).unwrap();

        assert_eq!(decoded.signature.len(), 65);
    }

    #[test]
    fn test_eth_personal_message_roundtrip() {
        let path = CryptoKeyPath::from_str("m/44'/60'/0'/0/1").unwrap();
        let request = EthSignRequest::new_personal_message(b"hello".to_vec(), path.clone())
            .with_origin("metamask".to_string());

        let bytes = request.to_cbor().unwrap();
        let decoded = EthSignRequest::from_cbor(&bytes).unwrap();

        assert_eq!(decoded.data_type, EthDataType::PersonalMessage);
        assert_eq!(decoded.derivation_path.to_string(), path.to_string());
        assert_eq!(decoded.origin, Some("metamask".to_string()));
    }
}
