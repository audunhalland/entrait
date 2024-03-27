#![allow(dead_code)]
#![allow(unused)]
#![allow(clippy::disallowed_names)]

mod delegation_modes;
mod dependency_inversion;
mod mockall;
mod simple;

#[cfg(feature = "unimock")]
mod unimock;

fn main() {}
