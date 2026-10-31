//! Compatibility facade. All implementation comes from patched zstd 0.14.3.
//! See backend/compat/README.md for provenance and enforced audit boundaries.
//! A legacy consumer's real compression operations resolve to the patched codec:
//! ```
//! let input = b"independent crawler and user data must roundtrip exactly";
//! let compressed = zstd::stream::encode_all(&input[..], 3).unwrap();
//! assert_eq!(zstd::stream::decode_all(&compressed[..]).unwrap(), input);
//! let block = zstd::bulk::compress(input, 3).unwrap();
//! assert_eq!(zstd::bulk::decompress(&block, input.len()).unwrap(), input);
//! ```
#![forbid(unsafe_code)]
pub use zstd_patched::*;
