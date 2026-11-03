//! Compatibility facade. All implementation comes from patched Apache Thrift 0.24.0.
//! See backend/compat/README.md for provenance and enforced audit boundaries.
//!
//! Reject attacker-controlled binary lengths before attempting to read their payload.
//! The small configured limit makes the allocation-bound regression safe and deterministic.
//! ```
//! use std::io::Cursor;
//! use thrift::{Error, ProtocolErrorKind, TConfiguration};
//! use thrift::protocol::{TBinaryInputProtocol, TInputProtocol};
//! let config = TConfiguration::builder().max_string_size(Some(4)).build().unwrap();
//! let mut oversized = TBinaryInputProtocol::with_config(Cursor::new(5_i32.to_be_bytes()), true, config.clone());
//! assert!(matches!(oversized.read_bytes(), Err(Error::Protocol(e)) if e.kind == ProtocolErrorKind::SizeLimit));
//! let mut negative = TBinaryInputProtocol::with_config(Cursor::new((-1_i32).to_be_bytes()), true, config.clone());
//! assert!(matches!(negative.read_bytes(), Err(Error::Protocol(e)) if e.kind == ProtocolErrorKind::NegativeSize));
//! // Non-strict message headers had a separate allocation path fixed in 0.24.
//! let mut header = TBinaryInputProtocol::with_config(Cursor::new(5_i32.to_be_bytes()), false, config);
//! assert!(matches!(header.read_message_begin(), Err(Error::Protocol(e)) if e.kind == ProtocolErrorKind::SizeLimit));
//! ```
//!
//! Keep the compact protocol used by Parquet interoperable through the legacy package API.
//! ```
//! use std::io::Cursor;
//! use thrift::protocol::{TCompactInputProtocol, TCompactOutputProtocol, TInputProtocol, TOutputProtocol};
//! let mut bytes = Vec::new();
//! {
//!     let mut writer = TCompactOutputProtocol::new(&mut bytes);
//!     writer.write_string("Parquet metadata").unwrap();
//!     writer.write_i64(-123456789).unwrap();
//! }
//! let mut reader = TCompactInputProtocol::new(Cursor::new(bytes));
//! assert_eq!(reader.read_string().unwrap(), "Parquet metadata");
//! assert_eq!(reader.read_i64().unwrap(), -123456789);
//! ```
#![forbid(unsafe_code)]
pub use thrift_patched::*;
