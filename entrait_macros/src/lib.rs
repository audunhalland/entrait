//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

extern crate proc_macro;

use proc_macro::TokenStream;

mod deps;
mod entrait;
mod input;

#[proc_macro_attribute]
pub fn entrait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |_| {})
}

#[proc_macro_attribute]
pub fn entrait_export(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable([&mut attr.export]);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable([&mut attr.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable([&mut attr.export, &mut attr.unimock]);
    })
}

fn enable<const N: usize>(opts: [&mut Option<input::SpanOpt<bool>>; N]) {
    for opt in opts.into_iter() {
        opt.get_or_insert(input::SpanOpt::of(true));
    }
}
