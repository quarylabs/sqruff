#![feature(trait_upcasting)]
#![feature(let_chains)]
#![allow(non_snake_case, clippy::module_inception, clippy::type_complexity)]
#![deny(unused_qualifications)]

pub mod api;
pub mod cli;
mod core;
pub mod dialects;
pub mod helpers;
mod rules;
mod utils;

fn main() {
    println!("Hello, world!");
}
