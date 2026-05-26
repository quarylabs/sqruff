#![cfg_attr(test, allow(deprecated))]

pub mod api;
pub mod config;
pub mod rules;
pub mod templaters;
#[cfg(test)]
mod tests;

#[allow(dead_code, unreachable_pub)]
pub(crate) mod core;
#[allow(dead_code, unreachable_pub)]
pub(crate) mod utils;

pub use config::{ConfigLoadOptions, ConfigLoader, ConfigOverrides, ConfigPatch, FluffConfig};
