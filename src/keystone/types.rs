//! Keystone payload container and dynamic message decoding

use crate::error::Result;
use crate::keystone::messages::{
    CryptoAccount, EthSignRequest, EthSignature, HederaSignRequest, HederaSignature,
    SolanaSignRequest, SolanaSignature, StellarSignRequest, StellarSignature, XrpSignRequest,
    XrpSignature,
};
use serde::{Deserialize, Serialize};

/// Encoded data format carried by the UR payload
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayloadEncoding {
    /// Binary CBOR blob (most Keystone messages)
    Cbor,
    /// UTF-8 JSON payload (e.g. XRP requests)
    Json,
    /// Raw binary payload with unknown encoding
    Binary,
}

impl Default for PayloadEncoding {
    fn default() -> Self {
        PayloadEncoding::Binary
    }
}

/// A Keystone-specific payload decoded from a UR string
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystonePayload {
    /// UR type (e.g., "crypto-account", "eth-sign-request")
    pub ur_type: String,
    /// Raw payload bytes
    pub data: Vec<u8>,
    /// Metadata captured during decoding (multi-part info, etc.)
    pub metadata: KeystoneMetadata,
    /// Detected data encoding
    pub encoding: PayloadEncoding,
}

impl KeystonePayload {
    /// Parse a UR string into a Keystone payload
    pub fn from_ur(ur_string: &str) -> Result<Self> {
        crate::keystone::ur::decode_ur(ur_string)
    }

    /// Encode this payload back into a UR string
    pub fn to_ur(&self) -> String {
        crate::keystone::ur::encode_ur(&self.ur_type, &self.data)
    }

    /// Attempt to parse the payload into a strongly typed Keystone message
    pub fn message_type(&self) -> Result<KeystoneMessage> {
        KeystoneMessage::from_ur_payload(self)
    }
}

/// Metadata describing how a UR payload was obtained
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeystoneMetadata {
    /// Sequence number (1-indexed) for multi-part payloads, if known
    pub sequence: Option<u32>,
    /// Total number of parts for multi-part payloads, if known
    pub total_parts: Option<u32>,
    /// True if the payload was reconstructed from multiple QR frames
    pub multipart: bool,
}

/// Dynamic Keystone message variants
#[derive(Debug, Clone)]
pub enum KeystoneMessage {
    /// Wallet pairing account information
    CryptoAccount(CryptoAccount),
    /// Ethereum sign request (transaction, typed data, personal message)
    EthSignRequest(EthSignRequest),
    /// Ethereum signature response
    EthSignature(EthSignature),
    /// Hedera sign request
    HederaSignRequest(HederaSignRequest),
    /// Hedera signature response
    HederaSignature(HederaSignature),
    /// Solana sign request
    SolanaSignRequest(SolanaSignRequest),
    /// Solana signature response
    SolanaSignature(SolanaSignature),
    /// XRP sign request (JSON)
    XrpSignRequest(XrpSignRequest),
    /// XRP signature response (JSON)
    XrpSignature(XrpSignature),
    /// Stellar sign request
    StellarSignRequest(StellarSignRequest),
    /// Stellar signature response
    StellarSignature(StellarSignature),
    /// Unrecognised UR payload
    Unknown {
        /// UR type string exactly as provided by the UR payload
        ur_type: String,
        /// Raw payload bytes for callers to handle manually
        data: Vec<u8>,
    },
}

impl KeystoneMessage {
    /// Decode a `KeystonePayload` into a concrete message
    pub fn from_ur_payload(payload: &KeystonePayload) -> Result<Self> {
        Self::from_ur_type(&payload.ur_type, &payload.data)
    }

    /// Decode based on UR type and raw payload bytes
    pub fn from_ur_type(ur_type: &str, data: &[u8]) -> Result<Self> {
        match ur_type {
            "crypto-account" => Ok(Self::CryptoAccount(CryptoAccount::from_cbor(data)?)),
            "eth-sign-request" => Ok(Self::EthSignRequest(EthSignRequest::from_cbor(data)?)),
            "eth-signature" => Ok(Self::EthSignature(EthSignature::from_cbor(data)?)),
            "hbar-sign-request" => Ok(Self::HederaSignRequest(HederaSignRequest::from_cbor(data)?)),
            "hbar-signature" => Ok(Self::HederaSignature(HederaSignature::from_cbor(data)?)),
            "sol-sign-request" => Ok(Self::SolanaSignRequest(SolanaSignRequest::from_cbor(data)?)),
            "sol-signature" => Ok(Self::SolanaSignature(SolanaSignature::from_cbor(data)?)),
            "xrp-sign-request" => Ok(Self::XrpSignRequest(XrpSignRequest::from_json_bytes(data)?)),
            "xrp-signature" => Ok(Self::XrpSignature(XrpSignature::from_json_bytes(data)?)),
            "stellar-sign-request" => Ok(Self::StellarSignRequest(StellarSignRequest::from_cbor(
                data,
            )?)),
            "stellar-signature" => Ok(Self::StellarSignature(StellarSignature::from_cbor(data)?)),
            _ => Ok(Self::Unknown {
                ur_type: ur_type.to_string(),
                data: data.to_vec(),
            }),
        }
    }
}

impl From<&KeystoneMessage> for PayloadEncoding {
    fn from(message: &KeystoneMessage) -> Self {
        match message {
            KeystoneMessage::XrpSignRequest(_) | KeystoneMessage::XrpSignature(_) => {
                PayloadEncoding::Json
            }
            KeystoneMessage::Unknown { .. } => PayloadEncoding::Binary,
            _ => PayloadEncoding::Cbor,
        }
    }
}

impl From<KeystoneMessage> for KeystonePayload {
    fn from(message: KeystoneMessage) -> Self {
        match message {
            KeystoneMessage::CryptoAccount(value) => serialize_cbor("crypto-account", &value),
            KeystoneMessage::EthSignRequest(value) => serialize_cbor("eth-sign-request", &value),
            KeystoneMessage::EthSignature(value) => serialize_cbor("eth-signature", &value),
            KeystoneMessage::HederaSignRequest(value) => {
                serialize_cbor("hbar-sign-request", &value)
            }
            KeystoneMessage::HederaSignature(value) => serialize_cbor("hbar-signature", &value),
            KeystoneMessage::SolanaSignRequest(value) => serialize_cbor("sol-sign-request", &value),
            KeystoneMessage::SolanaSignature(value) => serialize_cbor("sol-signature", &value),
            KeystoneMessage::StellarSignRequest(value) => {
                serialize_cbor("stellar-sign-request", &value)
            }
            KeystoneMessage::StellarSignature(value) => serialize_cbor("stellar-signature", &value),
            KeystoneMessage::XrpSignRequest(value) => serialize_json("xrp-sign-request", &value),
            KeystoneMessage::XrpSignature(value) => serialize_json("xrp-signature", &value),
            KeystoneMessage::Unknown { ur_type, data } => KeystonePayload {
                ur_type,
                data,
                metadata: KeystoneMetadata::default(),
                encoding: PayloadEncoding::Binary,
            },
        }
    }
}

fn serialize_cbor<T>(ur_type: &str, value: &T) -> KeystonePayload
where
    T: minicbor::Encode<()>,
{
    let data = crate::keystone::cbor::to_bytes(value).expect("CBOR serialization failed");
    KeystonePayload {
        ur_type: ur_type.to_string(),
        data,
        metadata: KeystoneMetadata::default(),
        encoding: PayloadEncoding::Cbor,
    }
}

fn serialize_json<T>(ur_type: &str, value: &T) -> KeystonePayload
where
    T: Serialize,
{
    let data = serde_json::to_vec(value).expect("JSON serialization failed");
    KeystonePayload {
        ur_type: ur_type.to_string(),
        data,
        metadata: KeystoneMetadata::default(),
        encoding: PayloadEncoding::Json,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystone::crypto_keypath::CryptoKeyPath;
    use uuid::Uuid;

    #[test]
    fn detects_json_encoding() {
        let msg = KeystoneMessage::XrpSignature(XrpSignature::new(None, "deadbeef".to_string()));
        let payload: KeystonePayload = msg.into();
        assert_eq!(payload.encoding, PayloadEncoding::Json);
        assert_eq!(payload.ur_type, "xrp-signature");
    }

    #[test]
    fn round_trip_eth_request() {
        let path = CryptoKeyPath::from_str("m/44'/60'/0'/0/0").unwrap();
        let request = EthSignRequest::new_transaction(vec![1, 2, 3], path, Some(1));
        let payload: KeystonePayload = KeystoneMessage::EthSignRequest(request.clone()).into();
        let parsed = payload.message_type().unwrap();
        match parsed {
            KeystoneMessage::EthSignRequest(decoded) => {
                assert_eq!(decoded.sign_data, request.sign_data);
                assert_eq!(decoded.chain_id, request.chain_id);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn unknown_payload_preserved() {
        let payload = KeystonePayload {
            ur_type: "custom-type".to_string(),
            data: vec![0x01, 0x02],
            metadata: KeystoneMetadata::default(),
            encoding: PayloadEncoding::Binary,
        };

        match payload.message_type().unwrap() {
            KeystoneMessage::Unknown { ur_type, data } => {
                assert_eq!(ur_type, "custom-type");
                assert_eq!(data, vec![0x01, 0x02]);
            }
            _ => panic!("expected unknown variant"),
        }
    }

    #[test]
    fn json_round_trip_xrp() {
        let request_id = Some(Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap());
        let request = XrpSignRequest::new(
            "{}".to_string(),
            "m/44'/144'/0'/0/0".to_string(),
            request_id,
        );
        let payload: KeystonePayload = KeystoneMessage::XrpSignRequest(request.clone()).into();
        let decoded = payload.message_type().unwrap();
        match decoded {
            KeystoneMessage::XrpSignRequest(parsed) => {
                assert_eq!(parsed.derivation_path, request.derivation_path);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn cbor_round_trip_crypto_account() {
        let account = CryptoAccount::new(
            [0xde, 0xad, 0xbe, 0xef],
            vec![0u8; 33],
            CryptoKeyPath::from_str("m/84'/0'/0'").unwrap(),
        );

        let payload: KeystonePayload = KeystoneMessage::CryptoAccount(account.clone()).into();
        let decoded = payload.message_type().unwrap();

        match decoded {
            KeystoneMessage::CryptoAccount(parsed) => {
                assert_eq!(parsed.master_fingerprint, account.master_fingerprint);
                assert_eq!(parsed.public_key, account.public_key);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn cbor_round_trip_hedera_request() {
        let path = CryptoKeyPath::from_str("m/44'/3030'/0'/0/0").unwrap();
        let request = HederaSignRequest::new(vec![1, 2, 3], path.clone(), None)
            .with_account_id("0.0.42".into());

        let payload: KeystonePayload = KeystoneMessage::HederaSignRequest(request.clone()).into();
        let decoded = payload.message_type().unwrap();

        match decoded {
            KeystoneMessage::HederaSignRequest(parsed) => {
                assert_eq!(parsed.transaction_bytes, request.transaction_bytes);
                assert_eq!(parsed.derivation_path.to_string(), path.to_string());
                assert_eq!(parsed.account_id, Some("0.0.42".into()));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn cbor_round_trip_solana_signature() {
        let signature = vec![0x11; 64];
        let public_key = vec![0x22; 32];
        let sig = SolanaSignature::new(signature.clone(), None).with_public_key(public_key.clone());

        let payload: KeystonePayload = KeystoneMessage::SolanaSignature(sig.clone()).into();
        let decoded = payload.message_type().unwrap();

        match decoded {
            KeystoneMessage::SolanaSignature(parsed) => {
                assert_eq!(parsed.signature, signature);
                assert_eq!(parsed.public_key, Some(public_key));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn cbor_round_trip_stellar_signature() {
        let request_id = Some(Uuid::parse_str("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d").unwrap());
        let signature = vec![0xaa; 64];
        let sig = StellarSignature::new(request_id, signature.clone());

        let payload: KeystonePayload = KeystoneMessage::StellarSignature(sig.clone()).into();
        let decoded = payload.message_type().unwrap();

        match decoded {
            KeystoneMessage::StellarSignature(parsed) => {
                assert_eq!(parsed.signature, signature);
                assert_eq!(parsed.request_id, sig.request_id);
            }
            _ => panic!("unexpected variant"),
        }
    }
}
