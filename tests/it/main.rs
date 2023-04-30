#![allow(dead_code)]
#![allow(unused)]
#![allow(clippy::blacklisted_name)]
#![cfg_attr(
    any(feature = "nightly-tests", feature = "use-associated-futures"),
    feature(type_alias_impl_trait)
)]
#![cfg_attr(
    any(feature = "nightly-tests", feature = "use-associated-futures"),
    feature(async_fn_in_trait)
)]
#![cfg_attr(
    any(feature = "nightly-tests", feature = "use-associated-futures"),
    feature(closure_track_caller)
)]
#![cfg_attr(
    any(feature = "nightly-tests", feature = "use-associated-futures"),
    feature(impl_trait_in_assoc_type)
)]

mod delegation_modes;
mod dependency_inversion;
mod mockall;
mod simple;

#[cfg(feature = "unimock")]
mod unimock;

fn main() {}
