//! Compatibility facade. All implementation comes from patched quick-xml 0.42.0.
//! See backend/compat/README.md for provenance and enforced audit boundaries.
//! The legacy XML reader API still parses real S3-shaped result documents:
//! ```
//! use quick_xml::{Reader, events::Event};
//! let mut reader = Reader::from_str("<ListBucketResult><Key>part.parquet</Key></ListBucketResult>");
//! let mut text_events = 0;
//! loop {
//!     match reader.read_event().unwrap() {
//!         Event::Text(_) => text_events += 1,
//!         Event::Eof => break,
//!         _ => {}
//!     }
//! }
//! assert_eq!(text_events, 1);
//! ```
#![forbid(unsafe_code)]
pub use quick_xml_patched::*;
