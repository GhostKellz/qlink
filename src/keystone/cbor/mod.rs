//! CBOR encoding/decoding helpers for Keystone messages

pub mod decode;
pub mod encode;

// Re-export traits if needed
// pub use encode::*;
// pub use decode::*;

use crate::error::{Error, Result};

/// Helper to encode a value to CBOR bytes
pub fn to_bytes<T: minicbor::Encode<()>>(value: &T) -> Result<Vec<u8>> {
    minicbor::to_vec(value).map_err(|e| Error::Cbor(format!("CBOR encode failed: {}", e)))
}

/// Helper to decode CBOR bytes to a value
pub fn from_bytes<'a, T: minicbor::Decode<'a, ()>>(bytes: &'a [u8]) -> Result<T> {
    minicbor::decode(bytes).map_err(|e| Error::Cbor(format!("CBOR decode failed: {}", e)))
}

/// CBOR tag constants for Keystone protocol
pub mod tags {
    /// UUID tag (RFC 4122)
    pub const UUID: u64 = 37;

    /// Crypto-keypath (BIP32 derivation path)
    pub const CRYPTO_KEYPATH: u64 = 304;

    /// Crypto-coin-info
    pub const CRYPTO_COIN_INFO: u64 = 305;

    /// Crypto-eckey (elliptic curve key)
    pub const CRYPTO_ECKEY: u64 = 306;

    /// Crypto-output
    pub const CRYPTO_OUTPUT: u64 = 308;

    /// Crypto-account
    pub const CRYPTO_ACCOUNT: u64 = 311;

    /// ETH sign request
    pub const ETH_SIGN_REQUEST: u64 = 401;

    /// ETH signature
    pub const ETH_SIGNATURE: u64 = 402;

    /// SOL sign request
    pub const SOL_SIGN_REQUEST: u64 = 1101;

    /// SOL signature
    pub const SOL_SIGNATURE: u64 = 1102;

    /// Stellar sign request
    pub const STELLAR_SIGN_REQUEST: u64 = 8201;

    /// Stellar signature
    pub const STELLAR_SIGNATURE: u64 = 8202;
}
