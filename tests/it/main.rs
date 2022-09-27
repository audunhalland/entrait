#![allow(dead_code)]
#![allow(unused)]
#![allow(clippy::blacklisted_name)]
#![cfg_attr(feature = "use-associated-futures", feature(type_alias_impl_trait))]

mod delegation_modes;
mod dependency_inversion;
mod mockall;
mod simple;

#[cfg(feature = "unimock")]
mod unimock;

fn main() {}
