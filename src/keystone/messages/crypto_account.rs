//! Crypto-account (wallet pairing) message types
//!
//! Simplified implementation for wallet connection/pairing
//! Full implementation with CryptoOutput descriptors can be added later

use crate::error::Result;
use crate::keystone::cbor;
use crate::keystone::crypto_keypath::CryptoKeyPath;
use minicbor::{Decoder, Encoder};

/// Simplified crypto-account for wallet pairing
#[derive(Debug, Clone)]
pub struct CryptoAccount {
    /// Master fingerprint (4 bytes)
    pub master_fingerprint: [u8; 4],
    /// Public key
    pub public_key: Vec<u8>,
    /// HD derivation path
    pub key_path: CryptoKeyPath,
    /// Optional chain code
    pub chain_code: Option<Vec<u8>>,
}

impl CryptoAccount {
    /// Create a new crypto-account
    pub fn new(master_fingerprint: [u8; 4], public_key: Vec<u8>, key_path: CryptoKeyPath) -> Self {
        Self {
            master_fingerprint,
            public_key,
            key_path,
            chain_code: None,
        }
    }

    /// Set the chain code
    pub fn with_chain_code(mut self, chain_code: Vec<u8>) -> Self {
        self.chain_code = Some(chain_code);
        self
    }

    /// Get the master fingerprint as u32
    pub fn fingerprint_u32(&self) -> u32 {
        u32::from_be_bytes(self.master_fingerprint)
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

impl minicbor::Encode<()> for CryptoAccount {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        let map_size = if self.chain_code.is_some() { 4 } else { 3 };
        e.map(map_size)?;

        // Key 1: master_fingerprint (as u32)
        e.u8(1)?;
        e.u32(self.fingerprint_u32())?;

        // Key 2: public_key
        e.u8(2)?;
        e.bytes(&self.public_key)?;

        // Key 3: key_path (with tag 304)
        e.u8(3)?;
        e.tag(minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH))?;
        self.key_path.encode(e, ctx)?;

        // Key 4: chain_code (optional)
        if let Some(ref chain_code) = self.chain_code {
            e.u8(4)?;
            e.bytes(chain_code)?;
        }

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for CryptoAccount {
    fn decode(
        d: &mut Decoder<'b>,
        ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        let mut master_fingerprint = None;
        let mut public_key = None;
        let mut key_path = None;
        let mut chain_code = None;

        let map_len = d.map()?.ok_or_else(|| {
            minicbor::decode::Error::message("expected definite-length map for CryptoAccount")
        })?;

        for _ in 0..map_len {
            let key = d.u8()?;
            match key {
                1 => {
                    // master_fingerprint (u32)
                    let fp_u32 = d.u32()?;
                    master_fingerprint = Some(fp_u32.to_be_bytes());
                }
                2 => {
                    // public_key
                    public_key = Some(d.bytes()?.to_vec());
                }
                3 => {
                    // key_path (with tag 304)
                    let tag = d.tag()?;
                    if tag != minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH) {
                        return Err(minicbor::decode::Error::message(
                            "expected CRYPTO_KEYPATH tag for key_path",
                        ));
                    }
                    key_path = Some(CryptoKeyPath::decode(d, ctx)?);
                }
                4 => {
                    // chain_code
                    chain_code = Some(d.bytes()?.to_vec());
                }
                _ => {
                    // Skip unknown keys
                    d.skip()?;
                }
            }
        }

        Ok(Self {
            master_fingerprint: master_fingerprint
                .ok_or_else(|| minicbor::decode::Error::message("missing master_fingerprint"))?,
            public_key: public_key
                .ok_or_else(|| minicbor::decode::Error::message("missing public_key"))?,
            key_path: key_path
                .ok_or_else(|| minicbor::decode::Error::message("missing key_path"))?,
            chain_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_account_cbor() {
        let master_fp = [0x78, 0x23, 0x08, 0x04];
        let public_key = vec![
            0x02, 0xc6, 0x04, 0x7f, 0x94, 0x41, 0xed, 0x7d, 0x6d, 0x30, 0x45, 0x40, 0x6e, 0x95,
            0xc0, 0x7c, 0xd8, 0x5c, 0x77, 0x8e, 0x4b, 0x8c, 0xef, 0x3c, 0xa7, 0xab, 0xac, 0x09,
            0xb9, 0x5c, 0x70, 0x9e, 0xe5,
        ];
        let path = CryptoKeyPath::from_str("m/44'/60'/0'/0/0").unwrap();

        let account = CryptoAccount::new(master_fp, public_key.clone(), path.clone());

        // Encode
        let cbor_bytes = account.to_cbor().unwrap();
        assert!(!cbor_bytes.is_empty());

        // Decode
        let decoded = CryptoAccount::from_cbor(&cbor_bytes).unwrap();
        assert_eq!(decoded.master_fingerprint, master_fp);
        assert_eq!(decoded.public_key, public_key);
        assert_eq!(decoded.key_path.to_string(), path.to_string());
    }

    #[test]
    fn test_crypto_account_with_chain_code() {
        let master_fp = [0x12, 0x34, 0x56, 0x78];
        let public_key = vec![0xaa; 33];
        let path = CryptoKeyPath::from_str("m/44'/0'/0'/0/0").unwrap();
        let chain_code = vec![0xbb; 32];

        let account = CryptoAccount::new(master_fp, public_key.clone(), path)
            .with_chain_code(chain_code.clone());

        let cbor_bytes = account.to_cbor().unwrap();
        let decoded = CryptoAccount::from_cbor(&cbor_bytes).unwrap();

        assert_eq!(decoded.chain_code, Some(chain_code));
    }

    #[test]
    fn test_fingerprint_conversion() {
        let fp = [0x12, 0x34, 0x56, 0x78];
        let public_key = vec![0x02; 33];
        let path = CryptoKeyPath::from_str("m/44'/0'/0'/0/0").unwrap();

        let account = CryptoAccount::new(fp, public_key, path);
        assert_eq!(account.fingerprint_u32(), 0x12345678);
    }
}
