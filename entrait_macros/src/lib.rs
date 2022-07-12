//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

extern crate proc_macro;

use proc_macro::TokenStream;

mod deps;
mod entrait;
mod input;

///
/// Generate a trait definition from a regular function.
///
#[proc_macro_attribute]
pub fn entrait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        if cfg!(feature = "unimock") {
            attr.unimock = Some((input::FeatureCfg::Always, proc_macro2::Span::call_site()));
        }
        if cfg!(feature = "mockall") {
            attr.mockall = Some((input::FeatureCfg::Always, proc_macro2::Span::call_site()));
        }
    })
}

///
/// Generate a trait definition from a regular function, with
/// [Impl](https://docs.rs/implementation/latest/implementation/struct.Impl.html)
/// and
/// [Unimock](https://docs.rs/unimock/latest/unimock/struct.Unimock.html)
/// implementations.
///
#[proc_macro_attribute]
pub fn entrait_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.unimock = Some((input::FeatureCfg::Always, proc_macro2::Span::call_site()));
    })
}

///
/// Generate a trait definition from a regular function, with
/// [Impl](https://docs.rs/implementation/latest/implementation/struct.Impl.html)
/// and
/// `cfg(test)`-gated [Unimock](https://docs.rs/unimock/latest/unimock/struct.Unimock.html)
/// implementations.
///
#[proc_macro_attribute]
pub fn entrait_unimock_test(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.unimock = Some((input::FeatureCfg::TestOnly, proc_macro2::Span::call_site()));
    })
}

///
/// Generate a trait definition from a regular function, with an implementation for
/// [Impl](https://docs.rs/implementation/latest/implementation/struct.Impl.html),
/// with [mockall](https://docs.rs/mockall/latest/mockall/) support.
///
#[proc_macro_attribute]
pub fn entrait_mockall(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.mockall = Some((input::FeatureCfg::Always, proc_macro2::Span::call_site()));
    })
}

///
/// Generate a trait definition from a regular function, with an implementation for
/// [Impl](https://docs.rs/implementation/latest/implementation/struct.Impl.html),
/// with `cfg(test)`-gated [mockall](https://docs.rs/mockall/latest/mockall/) support.
///
#[proc_macro_attribute]
pub fn entrait_mockall_test(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.mockall = Some((input::FeatureCfg::TestOnly, proc_macro2::Span::call_site()));
    })
}
