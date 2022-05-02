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
    entrait::invoke(attr, input, |_| {})
}

///
/// Generate a trait definition from a regular function with entrait = true.
///
#[proc_macro_attribute]
pub fn entrait_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.unimock = Some((input::EnabledValue::Always, proc_macro2::Span::call_site()));
    })
}

///
/// Generate a trait definition from a regular function with entrait = test.
///
#[proc_macro_attribute]
pub fn entrait_unimock_test(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.unimock = Some((
            input::EnabledValue::TestOnly,
            proc_macro2::Span::call_site(),
        ));
    })
}

///
/// Generate a trait definition from a regular function with mockall = true.
///
#[proc_macro_attribute]
pub fn entrait_mockall(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.mockall = Some((input::EnabledValue::Always, proc_macro2::Span::call_site()));
    })
}

///
/// Generate a trait definition from a regular function with mockall = test.
///
#[proc_macro_attribute]
pub fn entrait_mockall_test(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.mockall = Some((
            input::EnabledValue::TestOnly,
            proc_macro2::Span::call_site(),
        ));
    })
}
