//! Typed async client for the [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)
//! gateway HTTP API.
//!
//! A [`Client`] holds the gateway URL and the bearer token; each resource
//! group is reached through an accessor that borrows it:
//!
//! ```no_run
//! # async fn demo() -> zeroclaw_api_client::Result<()> {
//! use zeroclaw_api_client::Client;
//!
//! let client = Client::new("http://127.0.0.1:42617", "zc_…")?;
//! let channels = client.channels().list().await?;
//! let port = client.config().get("gateway.port").await?;
//! # Ok(()) }
//! ```
//!
//! Every HTTP request goes through a [tower](https://docs.rs/tower) stack
//! ([`transport`]): an answer timeout, retries with backoff for idempotent
//! calls, an optional concurrency limit and any layer of your own (see
//! [`ClientBuilder::layer`]).
//!
//! Errors are typed ([`Error`]): a gateway refusal keeps its status and the
//! gateway's own message. Shapes the gateway builds dynamically (config
//! sections, SOP definitions, log events) are `serde_json::Value`.
//!
//! The SDK targets the ZeroClaw 0.8.5 gateway; see the book under
//! `docs/book` for the build it is verified against and the endpoint map.

pub mod agents;
pub mod canvas;
pub mod channels;
pub mod chat;
pub mod client;
pub mod config;
pub mod cost;
pub mod cron;
pub mod error;
pub mod events;
pub mod files;
pub mod memory;
pub mod pairing;
pub mod quickstart;
pub mod sessions;
pub mod skills;
pub mod sops;
pub mod system;
pub mod transport;
pub mod webauthn;
pub mod webhook;

pub use client::{Client, ClientBuilder};
pub use error::{Error, Result};
pub use transport::RetryPolicy;
