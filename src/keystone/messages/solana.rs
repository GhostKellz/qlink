//! Solana (SOL) signature request and response types
//!
//! Reference: https://github.com/KeystoneHQ/keystone-sdk-rust

use crate::error::Result;
use crate::keystone::cbor;
use crate::keystone::crypto_keypath::CryptoKeyPath;
use minicbor::{Decoder, Encoder};
use uuid::Uuid;

/// Solana signature request
#[derive(Debug, Clone)]
pub struct SolanaSignRequest {
    /// Optional request identifier
    pub request_id: Option<Uuid>,
    /// Serialized Solana transaction message bytes
    pub transaction: Vec<u8>,
    /// BIP44 derivation path (typically m/44'/501'/0'/0')
    pub derivation_path: CryptoKeyPath,
    /// Optional application origin string
    pub origin: Option<String>,
}

impl SolanaSignRequest {
    /// Construct a new Solana sign request
    pub fn new(
        transaction: Vec<u8>,
        derivation_path: CryptoKeyPath,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            request_id,
            transaction,
            derivation_path,
            origin: None,
        }
    }

    /// Attach an origin descriptor (e.g. dApp name)
    pub fn with_origin(mut self, origin: String) -> Self {
        self.origin = Some(origin);
        self
    }

    fn map_len(&self) -> u64 {
        let mut len = 2; // transaction + derivation_path
        if self.request_id.is_some() {
            len += 1;
        }
        if self.origin.is_some() {
            len += 1;
        }
        len
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

impl minicbor::Encode<()> for SolanaSignRequest {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(minicbor::data::Tag::Unassigned(
            cbor::tags::SOL_SIGN_REQUEST,
        ))?;
        e.map(self.map_len())?;

        if let Some(ref uuid) = self.request_id {
            e.u8(1)?;
            e.tag(minicbor::data::Tag::Unassigned(cbor::tags::UUID))?;
            e.bytes(uuid.as_bytes())?;
        }

        e.u8(2)?;
        e.bytes(&self.transaction)?;

        e.u8(3)?;
        e.tag(minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH))?;
        self.derivation_path.encode(e, ctx)?;

        if let Some(ref origin) = self.origin {
            e.u8(4)?;
            e.str(origin)?;
        }

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for SolanaSignRequest {
    fn decode(
        d: &mut Decoder<'b>,
        ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        let tag = d.tag()?;
        if tag != minicbor::data::Tag::Unassigned(cbor::tags::SOL_SIGN_REQUEST) {
            return Err(minicbor::decode::Error::message(
                "expected sol-sign-request tag",
            ));
        }

        let map_len = d
            .map()?
            .ok_or_else(|| minicbor::decode::Error::message("expected definite-length map"))?;

        let mut request_id = None;
        let mut transaction = None;
        let mut derivation_path = None;
        let mut origin = None;

        for _ in 0..map_len {
            let key = d.u8()?;
            match key {
                1 => {
                    let tag = d.tag()?;
                    if tag != minicbor::data::Tag::Unassigned(cbor::tags::UUID) {
                        return Err(minicbor::decode::Error::message("expected UUID tag"));
                    }
                    let bytes = d.bytes()?;
                    request_id = Some(Uuid::from_slice(bytes).map_err(|e| {
                        minicbor::decode::Error::message(format!("invalid UUID: {}", e))
                    })?);
                }
                2 => transaction = Some(d.bytes()?.to_vec()),
                3 => {
                    let tag = d.tag()?;
                    if tag != minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH) {
                        return Err(minicbor::decode::Error::message(
                            "expected CRYPTO_KEYPATH tag",
                        ));
                    }
                    derivation_path = Some(CryptoKeyPath::decode(d, ctx)?);
                }
                4 => origin = Some(d.str()?.to_string()),
                _ => d.skip()?,
            }
        }

        Ok(Self {
            request_id,
            transaction: transaction
                .ok_or_else(|| minicbor::decode::Error::message("missing transaction"))?,
            derivation_path: derivation_path
                .ok_or_else(|| minicbor::decode::Error::message("missing derivation_path"))?,
            origin,
        })
    }
}

/// Solana signature response
#[derive(Debug, Clone)]
pub struct SolanaSignature {
    /// Request identifier echo
    pub request_id: Option<Uuid>,
    /// Signature bytes (64 bytes Ed25519 signature)
    pub signature: Vec<u8>,
    /// Optional attested public key
    pub public_key: Option<Vec<u8>>,
}

impl SolanaSignature {
    /// Create a new signature container
    pub fn new(signature: Vec<u8>, request_id: Option<Uuid>) -> Self {
        Self {
            request_id,
            signature,
            public_key: None,
        }
    }

    /// Attach a public key to the signature payload
    pub fn with_public_key(mut self, public_key: Vec<u8>) -> Self {
        self.public_key = Some(public_key);
        self
    }

    fn map_len(&self) -> u64 {
        let mut len = 1; // signature
        if self.request_id.is_some() {
            len += 1;
        }
        if self.public_key.is_some() {
            len += 1;
        }
        len
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

impl minicbor::Encode<()> for SolanaSignature {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(minicbor::data::Tag::Unassigned(cbor::tags::SOL_SIGNATURE))?;
        e.map(self.map_len())?;

        if let Some(ref uuid) = self.request_id {
            e.u8(1)?;
            e.tag(minicbor::data::Tag::Unassigned(cbor::tags::UUID))?;
            e.bytes(uuid.as_bytes())?;
        }

        e.u8(2)?;
        e.bytes(&self.signature)?;

        if let Some(ref public_key) = self.public_key {
            e.u8(3)?;
            e.bytes(public_key)?;
        }

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for SolanaSignature {
    fn decode(
        d: &mut Decoder<'b>,
        _ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        let tag = d.tag()?;
        if tag != minicbor::data::Tag::Unassigned(cbor::tags::SOL_SIGNATURE) {
            return Err(minicbor::decode::Error::message(
                "expected sol-signature tag",
            ));
        }

        let map_len = d
            .map()?
            .ok_or_else(|| minicbor::decode::Error::message("expected definite-length map"))?;

        let mut request_id = None;
        let mut signature = None;
        let mut public_key = None;

        for _ in 0..map_len {
            let key = d.u8()?;
            match key {
                1 => {
                    let tag = d.tag()?;
                    if tag != minicbor::data::Tag::Unassigned(cbor::tags::UUID) {
                        return Err(minicbor::decode::Error::message("expected UUID tag"));
                    }
                    let bytes = d.bytes()?;
                    request_id = Some(Uuid::from_slice(bytes).map_err(|e| {
                        minicbor::decode::Error::message(format!("invalid UUID: {}", e))
                    })?);
                }
                2 => signature = Some(d.bytes()?.to_vec()),
                3 => public_key = Some(d.bytes()?.to_vec()),
                _ => d.skip()?,
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
    use crate::keystone::crypto_keypath::CryptoKeyPath;
    use uuid::Uuid;

    #[test]
    fn test_solana_sign_request_roundtrip() {
        let path = CryptoKeyPath::from_str("m/44'/501'/0'/0'").unwrap();
        let request_id = Some(Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap());
        let tx = vec![0xde, 0xad, 0xbe, 0xef];
        let request = SolanaSignRequest::new(tx.clone(), path.clone(), request_id)
            .with_origin("phantom".to_string());

        let bytes = request.to_cbor().unwrap();
        let decoded = SolanaSignRequest::from_cbor(&bytes).unwrap();

        assert_eq!(decoded.transaction, tx);
        assert_eq!(decoded.derivation_path.to_string(), path.to_string());
        assert_eq!(decoded.origin.as_deref(), Some("phantom"));
    }

    #[test]
    fn test_solana_signature_roundtrip() {
        let signature = vec![0u8; 64];
        let public_key = vec![1u8; 32];
        let sig = SolanaSignature::new(signature.clone(), None).with_public_key(public_key.clone());

        let bytes = sig.to_cbor().unwrap();
        let decoded = SolanaSignature::from_cbor(&bytes).unwrap();

        assert_eq!(decoded.signature, signature);
        assert_eq!(decoded.public_key, Some(public_key));
    }
}
