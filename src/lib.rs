// SPDX-License-Identifier: MIT

pub mod app;
pub mod config;
pub mod scanner;

/// Set from the `version` field in `Cargo.toml` at compile time, which the
/// release workflow rewrites to match the pushed tag before building — see
/// `.github/workflows/release.yml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
