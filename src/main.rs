#![allow(non_snake_case, clippy::module_inception)]
#![deny(unused_qualifications)]

pub mod api;
pub mod cli;
mod core;
pub mod dialects;
mod rules;
mod utils;

fn main() {
    println!("Hello, world!");
}
