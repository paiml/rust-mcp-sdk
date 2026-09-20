//! [`CompletionSource`](crate::seams::CompletionSource) implementations.
//!
//! The trait is the extension point (AGNT-01), so this module ships concrete
//! sources behind one interface:
//!
//! - [`SamplingSource`] — zero-dependency, drives the model over the
//!   server-side peer's spec sampling surface (AGNT-04). NATIVE-ONLY: it is
//!   built over `pmcp::PeerHandle`, which is itself `#[cfg(not(wasm32))]`, so
//!   this source (like the adapter that mints it per request) cannot compile
//!   for wasm32. The wasm32 CI gate (D-13) therefore proves the LOOP + SEAMS +
//!   config path is target-clean, NOT SamplingSource-on-wasm.
//! - `OpenAiCompatSource` — any OpenAI-compatible `/chat/completions` endpoint
//!   (Ollama / vLLM / `OpenRouter` / …), behind the `openai-compat` feature
//!   (AGNT-05).
//! - `AnthropicSource` — the Anthropic Messages API, behind the `anthropic`
//!   feature (AGNT-06).
//!
//! The default (no-feature) build never pulls `reqwest`, keeping the default +
//! wasm32 build clean (D-13). API keys for the HTTP sources live in a redacting
//! [`SecretString`].

mod secret;

pub use secret::SecretString;

// SamplingSource rides `pmcp::PeerHandle` (native-only), so it is gated off
// wasm32. The default NATIVE build still compiles it with zero extra deps.
#[cfg(not(target_arch = "wasm32"))]
mod sampling;
#[cfg(not(target_arch = "wasm32"))]
pub use sampling::SamplingSource;

#[cfg(feature = "openai-compat")]
mod openai_compat;
#[cfg(feature = "openai-compat")]
pub use openai_compat::OpenAiCompatSource;

#[cfg(feature = "anthropic")]
mod anthropic;
#[cfg(feature = "anthropic")]
pub use anthropic::AnthropicSource;

// Shared HTTP-source plumbing (endpoint policy, client build, bounded body,
// status classification) is enabled whenever either HTTP source is compiled.
#[cfg(any(feature = "openai-compat", feature = "anthropic"))]
mod http_common;
#[cfg(any(feature = "openai-compat", feature = "anthropic"))]
pub use http_common::HttpSourceOptions;
