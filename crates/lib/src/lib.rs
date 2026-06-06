#![cfg_attr(test, allow(deprecated))]

pub mod api;
pub mod core;
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
pub mod ignore;
pub mod rules;
pub mod templaters;
#[cfg(test)]
mod tests;
pub mod utils;
