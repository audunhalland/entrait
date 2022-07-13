//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

extern crate proc_macro;

use proc_macro::TokenStream;

mod deps;
mod entrait;
mod input;

use input::AsyncStrategy;

#[proc_macro_attribute]
pub fn entrait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |_| {})
}

#[proc_macro_attribute]
pub fn entrait_export(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.export]);
    })
}

#[proc_macro_attribute]
pub fn entrait_async_trait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.export]);
        attr.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.export]);
        attr.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.export, &mut attr.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.unimock]);
        attr.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.export, &mut attr.unimock]);
        attr.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.unimock]);
        attr.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        enable_bool([&mut attr.export, &mut attr.unimock]);
        attr.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

fn enable_bool<const N: usize>(opts: [&mut Option<input::SpanOpt<bool>>; N]) {
    for opt in opts.into_iter() {
        opt.get_or_insert(input::SpanOpt::of(true));
    }
}
