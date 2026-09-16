//! Cursor-specific local integration boundary.
//!
//! Cursor traffic remains separate from the existing JSON/Provider proxy. Protocol and MITM
//! runtime pieces are added in later phases after the reversible settings contract is stable.

pub mod adapter;
pub mod ca;
pub mod e2e;
pub mod error;
pub mod fake_ip;
pub mod harness;
pub mod hosts;
pub mod mitm;
pub mod profile;
pub mod protocol;
pub mod protocol_backend;
pub mod provider;
pub mod provider_factory;
pub mod routes;
pub mod settings;
pub mod transparent;
pub mod transport;
