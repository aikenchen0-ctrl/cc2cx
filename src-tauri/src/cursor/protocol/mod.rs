//! Cursor wire-protocol primitives.
//!
//! This boundary intentionally contains only the validated Connect and BidiAppend subset used
//! by the first protocol slice. Provider conversion and full Agent semantics belong in a later
//! adapter layer.

pub mod bidi;
pub mod connect;
pub mod proto;
pub mod run_sse;
