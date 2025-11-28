//! Keystone Pro 3 protocol implementation
//!
//! This module handles the UR (Uniform Resources) protocol used by Keystone
//! for air-gapped communication via QR codes.
//!
//! Reference: https://github.com/BlockchainCommons/Research/blob/master/papers/bcr-2020-005-ur.md

pub mod cbor;
pub mod crypto_keypath;
pub mod messages;
pub mod multipart;
mod types;
mod ur;

pub use crypto_keypath::{CryptoKeyPath, PathComponent};
pub use messages::*;
pub use types::{KeystoneMessage, KeystoneMetadata, KeystonePayload, PayloadEncoding};

use crate::error::{Error, Result};
use crate::qr::QrPayload;

impl TryFrom<QrPayload> for KeystonePayload {
    type Error = Error;

    fn try_from(qr: QrPayload) -> Result<Self> {
        // Keystone QR codes use the UR format: ur:TYPE/DATA
        let text = qr.as_str().ok_or_else(|| {
            Error::InvalidKeystonePayload("QR payload is not valid UTF-8".to_string())
        })?;

        if !text.starts_with("ur:") {
            return Err(Error::InvalidKeystonePayload(
                "Not a UR-encoded payload".to_string(),
            ));
        }

        KeystonePayload::from_ur(text)
    }
}

impl From<KeystonePayload> for QrPayload {
    fn from(keystone: KeystonePayload) -> Self {
        let ur_string = keystone.to_ur();
        QrPayload::from_string(ur_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ur_detection() {
        let ur_qr = QrPayload::from_string("ur:crypto-account/test".to_string());
        let result = KeystonePayload::try_from(ur_qr);
        // Will fail with parse error but should recognize it as UR
        assert!(result.is_err());
        if let Err(Error::UrParse(_)) = result {
            // Expected
        } else {
            panic!("Expected UrParse error");
        }
    }

    #[test]
    fn test_non_ur_rejection() {
        let regular_qr = QrPayload::from_string("https://example.com".to_string());
        let result = KeystonePayload::try_from(regular_qr);
        assert!(matches!(result, Err(Error::InvalidKeystonePayload(_))));
    }
}
