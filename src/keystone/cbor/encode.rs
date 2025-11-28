//! CBOR encoding helpers

// Optional encoding helper (commented out - not needed for current implementation)
//
// /// Helper trait for encoding optional values in CBOR maps
// pub trait EncodeOptional {
//     /// Encode an optional value at the given map key
//     fn encode_optional<T: minicbor::Encode<()>>(
//         &mut self,
//         key: u64,
//         value: &Option<T>,
//     ) -> std::result::Result<&mut Self, minicbor::encode::Error<std::io::Error>>;
// }
//
// impl<W: minicbor::encode::Write> EncodeOptional for Encoder<W> {
//     fn encode_optional<T: minicbor::Encode<()>>(
//         &mut self,
//         key: u64,
//         value: &Option<T>,
//     ) -> std::result::Result<&mut Self, minicbor::encode::Error<std::io::Error>> {
//         if let Some(val) = value {
//             self.u64(key)?.encode(val)?;
//         }
//         Ok(self)
//     }
// }
