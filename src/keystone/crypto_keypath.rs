//! BIP32 derivation path encoding (crypto-keypath)
//!
//! Reference: archive/keystone-sdk-rust/libs/ur-registry/src/crypto_key_path.rs

use crate::error::{Error, Result};
use crate::keystone::cbor;
use minicbor::{Decoder, Encoder};
use serde::{Deserialize, Serialize};

/// A single component of a derivation path
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathComponent {
    /// The index value
    pub index: u32,
    /// Whether this is a hardened derivation
    pub hardened: bool,
}

impl PathComponent {
    /// Create a new path component
    pub fn new(index: u32, hardened: bool) -> Self {
        Self { index, hardened }
    }

    /// Create a hardened component
    pub fn hardened(index: u32) -> Self {
        Self {
            index,
            hardened: true,
        }
    }

    /// Create a normal (non-hardened) component
    pub fn normal(index: u32) -> Self {
        Self {
            index,
            hardened: false,
        }
    }

    /// Get the BIP32 index value (with hardened bit if applicable)
    pub fn to_bip32_index(self) -> u32 {
        if self.hardened {
            self.index | 0x80000000
        } else {
            self.index
        }
    }
}

/// BIP32 derivation path (crypto-keypath)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CryptoKeyPath {
    /// Path components (e.g., [44', 60', 0', 0, 0] for m/44'/60'/0'/0/0)
    pub components: Vec<PathComponent>,
    /// Optional source fingerprint (4 bytes)
    pub source_fingerprint: Option<[u8; 4]>,
    /// Optional depth
    pub depth: Option<u8>,
}

impl CryptoKeyPath {
    /// Create a new derivation path
    pub fn new(components: Vec<PathComponent>) -> Self {
        Self {
            components,
            source_fingerprint: None,
            depth: None,
        }
    }

    /// Create a path with source fingerprint
    pub fn with_source_fingerprint(mut self, fingerprint: [u8; 4]) -> Self {
        self.source_fingerprint = Some(fingerprint);
        self
    }

    /// Create a path with depth
    pub fn with_depth(mut self, depth: u8) -> Self {
        self.depth = Some(depth);
        self
    }

    /// Parse from string like "m/44'/60'/0'/0/0"
    pub fn from_str(path: &str) -> Result<Self> {
        let path = path.trim();

        // Remove "m/" prefix if present
        let path = path.strip_prefix("m/").unwrap_or(path);

        if path.is_empty() {
            return Ok(Self::new(vec![]));
        }

        let components: Result<Vec<PathComponent>> = path
            .split('/')
            .map(|component| {
                let hardened = component.ends_with('\'') || component.ends_with('h');
                let index_str = if hardened {
                    &component[..component.len() - 1]
                } else {
                    component
                };

                index_str
                    .parse::<u32>()
                    .map(|index| PathComponent::new(index, hardened))
                    .map_err(|e| {
                        Error::Config(format!("Invalid path component '{}': {}", component, e))
                    })
            })
            .collect();

        Ok(Self::new(components?))
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        if self.components.is_empty() {
            return "m".to_string();
        }

        let components: Vec<String> = self
            .components
            .iter()
            .map(|c| {
                if c.hardened {
                    format!("{}'", c.index)
                } else {
                    c.index.to_string()
                }
            })
            .collect();

        format!("m/{}", components.join("/"))
    }
}

// CBOR encoding: crypto-keypath is tag 304 with map structure
impl minicbor::Encode<()> for CryptoKeyPath {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut (),
    ) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
        // Tag 304 for crypto-keypath
        e.tag(minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH))?;

        // Determine map size
        let mut map_size = 1; // components is required
        if self.source_fingerprint.is_some() {
            map_size += 1;
        }
        if self.depth.is_some() {
            map_size += 1;
        }

        e.map(map_size)?;

        // Key 1: components (array of u32 with hardened bit)
        e.u64(1)?;
        e.array(self.components.len() as u64)?;
        for component in &self.components {
            e.u32(component.to_bip32_index())?;
        }

        // Key 2: source fingerprint (optional)
        if let Some(ref fingerprint) = self.source_fingerprint {
            e.u64(2)?.bytes(fingerprint)?;
        }

        // Key 3: depth (optional)
        if let Some(depth) = self.depth {
            e.u64(3)?.u8(depth)?;
        }

        Ok(())
    }
}

impl<'b> minicbor::Decode<'b, ()> for CryptoKeyPath {
    fn decode(
        d: &mut Decoder<'b>,
        _ctx: &mut (),
    ) -> std::result::Result<Self, minicbor::decode::Error> {
        // Expect tag 304
        let tag = d.tag()?;
        if tag != minicbor::data::Tag::Unassigned(cbor::tags::CRYPTO_KEYPATH) {
            return Err(minicbor::decode::Error::message(
                "Expected crypto-keypath tag 304",
            ));
        }

        // Decode map
        let map_len = d
            .map()?
            .ok_or_else(|| minicbor::decode::Error::message("Expected definite-length map"))?;

        let mut components = None;
        let mut source_fingerprint = None;
        let mut depth = None;

        for _ in 0..map_len {
            let key = d.u64()?;
            match key {
                1 => {
                    // Components array
                    let arr_len = d.array()?.ok_or_else(|| {
                        minicbor::decode::Error::message("Expected definite-length array")
                    })?;
                    let mut comps = Vec::new();
                    for _ in 0..arr_len {
                        let bip32_index = d.u32()?;
                        let hardened = (bip32_index & 0x80000000) != 0;
                        let index = bip32_index & 0x7FFFFFFF;
                        comps.push(PathComponent::new(index, hardened));
                    }
                    components = Some(comps);
                }
                2 => {
                    // Source fingerprint
                    let bytes = d.bytes()?;
                    if bytes.len() == 4 {
                        let mut fp = [0u8; 4];
                        fp.copy_from_slice(bytes);
                        source_fingerprint = Some(fp);
                    }
                }
                3 => {
                    // Depth
                    depth = Some(d.u8()?);
                }
                _ => {
                    // Skip unknown keys
                    d.skip()?;
                }
            }
        }

        Ok(CryptoKeyPath {
            components: components
                .ok_or_else(|| minicbor::decode::Error::message("Missing components"))?,
            source_fingerprint,
            depth,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_from_string() {
        let path = CryptoKeyPath::from_str("m/44'/60'/0'/0/0").unwrap();
        assert_eq!(path.components.len(), 5);
        assert_eq!(path.components[0], PathComponent::hardened(44));
        assert_eq!(path.components[1], PathComponent::hardened(60));
        assert_eq!(path.components[2], PathComponent::hardened(0));
        assert_eq!(path.components[3], PathComponent::normal(0));
        assert_eq!(path.components[4], PathComponent::normal(0));
    }

    #[test]
    fn test_path_to_string() {
        let path = CryptoKeyPath::new(vec![
            PathComponent::hardened(44),
            PathComponent::hardened(60),
            PathComponent::hardened(0),
            PathComponent::normal(0),
            PathComponent::normal(0),
        ]);

        assert_eq!(path.to_string(), "m/44'/60'/0'/0/0");
    }

    #[test]
    fn test_cbor_roundtrip() {
        let path = CryptoKeyPath::new(vec![
            PathComponent::hardened(44),
            PathComponent::hardened(60),
        ])
        .with_source_fingerprint([0x12, 0x34, 0x56, 0x78]);

        let bytes = cbor::to_bytes(&path).unwrap();
        let decoded: CryptoKeyPath = cbor::from_bytes(&bytes).unwrap();

        assert_eq!(path, decoded);
    }
}
