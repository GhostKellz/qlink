//! CBOR decoding helpers

use minicbor::Decoder;

/// Helper trait for decoding optional values from CBOR maps
pub trait DecodeOptional<'b> {
    /// Try to decode an optional value, returning None if key not present
    fn decode_optional<T: minicbor::Decode<'b, ()>>(
        &mut self,
    ) -> Result<Option<T>, minicbor::decode::Error>;
}

impl<'b> DecodeOptional<'b> for Decoder<'b> {
    fn decode_optional<T: minicbor::Decode<'b, ()>>(
        &mut self,
    ) -> Result<Option<T>, minicbor::decode::Error> {
        // Try to decode, if it fails due to unexpected type or missing data, return None
        match self.decode() {
            Ok(val) => Ok(Some(val)),
            Err(_) => Ok(None),
        }
    }
}
