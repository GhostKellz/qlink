//! Helpers for rendering and distributing structured scan output

#[cfg(target_family = "unix")]
pub mod unix;

use crate::keystone::messages::{EthDataType, StellarSignType};
use crate::{KeystoneMessage, KeystonePayload, PayloadEncoding};
use hex::encode as hex_encode;
use serde_json::{Map, Value, json};

/// Combined structured and human-readable representation of a Keystone payload
#[derive(Debug, Clone)]
pub struct RenderedKeystone {
    /// Structured JSON representation suitable for downstream consumers
    pub json: Value,
    /// Human-readable lines for terminal presentation
    pub human: Vec<String>,
}

/// Render a Keystone payload into both JSON and human-readable forms.
pub fn render_keystone_payload(payload: &KeystonePayload) -> RenderedKeystone {
    let json = keystone_payload_value(payload);
    let mut human = Vec::new();

    human.push("Keystone payload detected".to_string());
    human.push(format!("  UR type: {}", payload.ur_type));
    human.push(format!("  Encoding: {}", encoding_label(payload.encoding)));

    if payload.metadata.multipart {
        let sequence = payload.metadata.sequence.unwrap_or(0);
        let total = payload.metadata.total_parts.unwrap_or(0);
        human.push(format!(
            "  Multipart: true (part {} of {})",
            sequence, total
        ));
    }

    human.push(format!("  Raw bytes: {}", payload.data.len()));

    match payload.message_type() {
        Ok(message) => human.extend(human_lines_for_message(&message)),
        Err(err) => human.push(format!("  Failed to decode message: {err}")),
    }

    RenderedKeystone { json, human }
}

/// Produce a structured JSON representation of the Keystone payload.
pub fn keystone_payload_value(payload: &KeystonePayload) -> Value {
    let mut root = Map::new();
    root.insert(
        "ur_type".to_string(),
        Value::String(payload.ur_type.clone()),
    );
    root.insert(
        "encoding".to_string(),
        Value::String(encoding_label(payload.encoding).to_string()),
    );
    root.insert("byte_length".to_string(), Value::from(payload.data.len()));
    root.insert(
        "bytes_hex".to_string(),
        Value::String(hex_encode(&payload.data)),
    );
    root.insert(
        "metadata".to_string(),
        json!({
            "multipart": payload.metadata.multipart,
            "sequence": payload.metadata.sequence,
            "total_parts": payload.metadata.total_parts,
        }),
    );

    match payload.message_type() {
        Ok(message) => {
            root.insert(
                "message_variant".to_string(),
                Value::String(message_variant_label(&message).to_string()),
            );
            root.insert("message".to_string(), keystone_message_value(&message));
        }
        Err(err) => {
            root.insert("message_error".to_string(), Value::String(err.to_string()));
        }
    }

    Value::Object(root)
}

fn keystone_message_value(message: &KeystoneMessage) -> Value {
    match message {
        KeystoneMessage::CryptoAccount(account) => json!({
            "master_fingerprint": format!("{:08x}", account.fingerprint_u32()),
            "derivation_path": account.key_path.to_string(),
            "public_key_hex": hex_encode(&account.public_key),
            "public_key_bytes": account.public_key.len(),
            "chain_code_hex": account.chain_code.as_ref().map(hex_encode),
        }),
        KeystoneMessage::EthSignRequest(request) => json!({
            "request_id": request.request_id.map(|id| id.to_string()),
            "derivation_path": request.derivation_path.to_string(),
            "chain_id": request.chain_id,
            "data_type": eth_data_type_label(request.data_type),
            "origin": request.origin.clone(),
            "address_hex": request.address.as_ref().map(hex_encode),
            "sign_data_hex": hex_encode(&request.sign_data),
            "sign_data_bytes": request.sign_data.len(),
        }),
        KeystoneMessage::EthSignature(signature) => {
            let components = signature.rsv().ok();
            json!({
                "request_id": signature.request_id.map(|id| id.to_string()),
                "origin": signature.origin.clone(),
                "signature_hex": hex_encode(&signature.signature),
                "signature_bytes": signature.signature.len(),
                "r": components.as_ref().map(|(r, _, _)| hex_encode(r)),
                "s": components.as_ref().map(|(_, s, _)| hex_encode(s)),
                "v": components.as_ref().map(|(_, _, v)| format!("{:02x}", v)),
            })
        }
        KeystoneMessage::HederaSignRequest(request) => json!({
            "request_id": request.request_id.map(|id| id.to_string()),
            "derivation_path": request.derivation_path.to_string(),
            "account_id": request.account_id.clone(),
            "origin": request.origin.clone(),
            "transaction_hex": hex_encode(&request.transaction_bytes),
            "transaction_bytes": request.transaction_bytes.len(),
        }),
        KeystoneMessage::HederaSignature(signature) => json!({
            "request_id": signature.request_id.map(|id| id.to_string()),
            "signature_hex": hex_encode(&signature.signature),
            "signature_bytes": signature.signature.len(),
            "public_key_hex": signature.public_key.as_ref().map(hex_encode),
        }),
        KeystoneMessage::SolanaSignRequest(request) => json!({
            "request_id": request.request_id.map(|id| id.to_string()),
            "derivation_path": request.derivation_path.to_string(),
            "origin": request.origin.clone(),
            "transaction_hex": hex_encode(&request.transaction),
            "transaction_bytes": request.transaction.len(),
        }),
        KeystoneMessage::SolanaSignature(signature) => json!({
            "request_id": signature.request_id.map(|id| id.to_string()),
            "signature_hex": hex_encode(&signature.signature),
            "signature_bytes": signature.signature.len(),
            "public_key_hex": signature.public_key.as_ref().map(hex_encode),
        }),
        KeystoneMessage::StellarSignRequest(request) => json!({
            "request_id": request.request_id.map(|id| id.to_string()),
            "derivation_path": request.derivation_path.to_string(),
            "sign_type": stellar_sign_type_label(&request.sign_type),
            "origin": request.origin.clone(),
            "address_hex": request.address.as_ref().map(hex_encode),
            "sign_data_hex": hex_encode(&request.sign_data),
            "sign_data_bytes": request.sign_data.len(),
        }),
        KeystoneMessage::StellarSignature(signature) => json!({
            "request_id": signature.request_id.map(|id| id.to_string()),
            "signature_hex": hex_encode(&signature.signature),
            "signature_bytes": signature.signature.len(),
        }),
        KeystoneMessage::XrpSignRequest(request) => {
            let tx_value = serde_json::from_str::<Value>(&request.transaction_json)
                .unwrap_or_else(|_| Value::String(request.transaction_json.clone()));
            json!({
                "request_id": request.request_id.map(|id| id.to_string()),
                "derivation_path": request.derivation_path.clone(),
                "origin": request.origin.clone(),
                "transaction": tx_value,
            })
        }
        KeystoneMessage::XrpSignature(signature) => json!({
            "request_id": signature.request_id.map(|id| id.to_string()),
            "signature": signature.signature.clone(),
        }),
        KeystoneMessage::Unknown { ur_type, data } => json!({
            "ur_type": ur_type,
            "data_hex": hex_encode(data),
            "data_bytes": data.len(),
        }),
    }
}

fn human_lines_for_message(message: &KeystoneMessage) -> Vec<String> {
    match message {
        KeystoneMessage::CryptoAccount(account) => {
            let mut lines = vec!["  Variant: crypto_account".to_string()];
            lines.push(format!(
                "    Fingerprint: {:08x}",
                account.fingerprint_u32()
            ));
            lines.push(format!(
                "    Derivation path: {}",
                account.key_path.to_string()
            ));
            lines.push(format!(
                "    Public key: {}",
                format_hex_snippet(&account.public_key)
            ));
            if let Some(chain_code) = &account.chain_code {
                lines.push(format!(
                    "    Chain code: {}",
                    format_hex_snippet(chain_code)
                ));
            }
            lines
        }
        KeystoneMessage::EthSignRequest(request) => {
            let mut lines = vec!["  Variant: eth_sign_request".to_string()];
            if let Some(id) = request.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!(
                "    Data type: {}",
                eth_data_type_label(request.data_type)
            ));
            if let Some(chain_id) = request.chain_id {
                lines.push(format!("    Chain ID: {}", chain_id));
            }
            lines.push(format!(
                "    Derivation path: {}",
                request.derivation_path.to_string()
            ));
            if let Some(address) = &request.address {
                lines.push(format!("    Address: {}", format_hex_snippet(address)));
            }
            if let Some(origin) = &request.origin {
                lines.push(format!("    Origin: {}", origin));
            }
            lines.push(format!(
                "    Sign data: {}",
                format_hex_snippet(&request.sign_data)
            ));
            lines
        }
        KeystoneMessage::EthSignature(signature) => {
            let mut lines = vec!["  Variant: eth_signature".to_string()];
            if let Some(id) = signature.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            if let Some(origin) = &signature.origin {
                lines.push(format!("    Origin: {}", origin));
            }
            lines.push(format!(
                "    Signature: {}",
                format_hex_snippet(&signature.signature)
            ));
            if let Ok((r, s, v)) = signature.rsv() {
                lines.push(format!("    r: {}", hex_encode(r)));
                lines.push(format!("    s: {}", hex_encode(s)));
                lines.push(format!("    v: {:02x}", v));
            }
            lines
        }
        KeystoneMessage::HederaSignRequest(request) => {
            let mut lines = vec!["  Variant: hedera_sign_request".to_string()];
            if let Some(id) = request.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!(
                "    Derivation path: {}",
                request.derivation_path.to_string()
            ));
            if let Some(account_id) = &request.account_id {
                lines.push(format!("    Account ID: {}", account_id));
            }
            if let Some(origin) = &request.origin {
                lines.push(format!("    Origin: {}", origin));
            }
            lines.push(format!(
                "    Transaction bytes: {}",
                format_hex_snippet(&request.transaction_bytes)
            ));
            lines
        }
        KeystoneMessage::HederaSignature(signature) => {
            let mut lines = vec!["  Variant: hedera_signature".to_string()];
            if let Some(id) = signature.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!(
                "    Signature: {}",
                format_hex_snippet(&signature.signature)
            ));
            if let Some(public_key) = &signature.public_key {
                lines.push(format!(
                    "    Public key: {}",
                    format_hex_snippet(public_key)
                ));
            }
            lines
        }
        KeystoneMessage::SolanaSignRequest(request) => {
            let mut lines = vec!["  Variant: solana_sign_request".to_string()];
            if let Some(id) = request.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!(
                "    Derivation path: {}",
                request.derivation_path.to_string()
            ));
            if let Some(origin) = &request.origin {
                lines.push(format!("    Origin: {}", origin));
            }
            lines.push(format!(
                "    Transaction bytes: {}",
                format_hex_snippet(&request.transaction)
            ));
            lines
        }
        KeystoneMessage::SolanaSignature(signature) => {
            let mut lines = vec!["  Variant: solana_signature".to_string()];
            if let Some(id) = signature.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!(
                "    Signature: {}",
                format_hex_snippet(&signature.signature)
            ));
            if let Some(public_key) = &signature.public_key {
                lines.push(format!(
                    "    Public key: {}",
                    format_hex_snippet(public_key)
                ));
            }
            lines
        }
        KeystoneMessage::StellarSignRequest(request) => {
            let mut lines = vec!["  Variant: stellar_sign_request".to_string()];
            if let Some(id) = request.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!(
                "    Derivation path: {}",
                request.derivation_path.to_string()
            ));
            lines.push(format!(
                "    Sign type: {}",
                stellar_sign_type_label(&request.sign_type)
            ));
            if let Some(origin) = &request.origin {
                lines.push(format!("    Origin: {}", origin));
            }
            if let Some(address) = &request.address {
                lines.push(format!("    Address: {}", format_hex_snippet(address)));
            }
            lines.push(format!(
                "    Sign data: {}",
                format_hex_snippet(&request.sign_data)
            ));
            lines
        }
        KeystoneMessage::StellarSignature(signature) => {
            let mut lines = vec!["  Variant: stellar_signature".to_string()];
            if let Some(id) = signature.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!(
                "    Signature: {}",
                format_hex_snippet(&signature.signature)
            ));
            lines
        }
        KeystoneMessage::XrpSignRequest(request) => {
            let mut lines = vec!["  Variant: xrp_sign_request".to_string()];
            if let Some(id) = request.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!("    Derivation path: {}", request.derivation_path));
            if let Some(origin) = &request.origin {
                lines.push(format!("    Origin: {}", origin));
            }
            lines.push(format!(
                "    Transaction JSON: {}",
                format_text_snippet(&request.transaction_json)
            ));
            lines
        }
        KeystoneMessage::XrpSignature(signature) => {
            let mut lines = vec!["  Variant: xrp_signature".to_string()];
            if let Some(id) = signature.request_id {
                lines.push(format!("    Request ID: {}", id));
            }
            lines.push(format!("    Signature: {}", signature.signature));
            lines
        }
        KeystoneMessage::Unknown { ur_type, data } => {
            vec![
                format!("  Variant: unknown ({ur_type})"),
                format!("    Raw bytes: {}", format_hex_snippet(data)),
            ]
        }
    }
}

fn message_variant_label(message: &KeystoneMessage) -> &'static str {
    match message {
        KeystoneMessage::CryptoAccount(_) => "crypto_account",
        KeystoneMessage::EthSignRequest(_) => "eth_sign_request",
        KeystoneMessage::EthSignature(_) => "eth_signature",
        KeystoneMessage::HederaSignRequest(_) => "hedera_sign_request",
        KeystoneMessage::HederaSignature(_) => "hedera_signature",
        KeystoneMessage::SolanaSignRequest(_) => "solana_sign_request",
        KeystoneMessage::SolanaSignature(_) => "solana_signature",
        KeystoneMessage::StellarSignRequest(_) => "stellar_sign_request",
        KeystoneMessage::StellarSignature(_) => "stellar_signature",
        KeystoneMessage::XrpSignRequest(_) => "xrp_sign_request",
        KeystoneMessage::XrpSignature(_) => "xrp_signature",
        KeystoneMessage::Unknown { .. } => "unknown",
    }
}

fn encoding_label(encoding: PayloadEncoding) -> &'static str {
    match encoding {
        PayloadEncoding::Cbor => "cbor",
        PayloadEncoding::Json => "json",
        PayloadEncoding::Binary => "binary",
    }
}

fn eth_data_type_label(data_type: EthDataType) -> &'static str {
    match data_type {
        EthDataType::Transaction => "transaction",
        EthDataType::TypedData => "typed_data",
        EthDataType::PersonalMessage => "personal_message",
        EthDataType::TypedTransaction => "typed_transaction",
    }
}

fn stellar_sign_type_label(sign_type: &StellarSignType) -> &'static str {
    match sign_type {
        StellarSignType::Transaction => "transaction",
        StellarSignType::TransactionHash => "transaction_hash",
        StellarSignType::Message => "message",
    }
}

fn format_hex_snippet(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "empty".to_string();
    }

    let hex = hex_encode(bytes);
    const MAX: usize = 64;
    if hex.len() > MAX {
        format!("{}... ({} bytes)", &hex[..MAX], bytes.len())
    } else {
        format!("{} ({} bytes)", hex, bytes.len())
    }
}

fn format_text_snippet(text: &str) -> String {
    const MAX: usize = 120;
    if text.chars().count() <= MAX {
        text.to_string()
    } else {
        let snippet: String = text.chars().take(MAX).collect();
        let total = text.chars().count();
        format!("{}... ({} chars)", snippet, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystone::crypto_keypath::CryptoKeyPath;
    use crate::keystone::messages::ethereum::EthSignRequest;

    #[test]
    fn renders_eth_request_consistently() {
        let path = CryptoKeyPath::from_str("m/44'/60'/0'/0/0").unwrap();
        let request = EthSignRequest::new_transaction(vec![1, 2, 3], path, Some(1));
        let payload: KeystonePayload = KeystoneMessage::EthSignRequest(request.clone()).into();
        let rendered = render_keystone_payload(&payload);

        assert_eq!(rendered.json["message_variant"], "eth_sign_request");
        assert!(
            rendered
                .human
                .iter()
                .any(|line| line.contains("Variant: eth_sign_request"))
        );
        assert!(
            rendered
                .human
                .iter()
                .any(|line| line.contains("Chain ID: 1"))
        );
        assert!(
            rendered
                .human
                .iter()
                .any(|line| line.contains("Sign data:"))
        );
    }
}
