//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

extern crate proc_macro;

use proc_macro::TokenStream;

mod delegate_impl;
mod entrait;
mod util;

use entrait::input::AsyncStrategy;

#[proc_macro_attribute]
pub fn entrait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |_| {})
}

#[proc_macro_attribute]
pub fn entrait_export(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        set_fallbacks([&mut attr.export]);
    })
}

#[proc_macro_attribute]
pub fn entrait_use_async_trait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        attr.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        set_fallbacks([&mut attr.export]);
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
        set_fallbacks([&mut attr.export]);
        attr.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        set_fallbacks([&mut attr.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        set_fallbacks([&mut attr.export, &mut attr.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        set_fallbacks([&mut attr.unimock]);
        attr.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        set_fallbacks([&mut attr.export, &mut attr.unimock]);
        attr.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        set_fallbacks([&mut attr.unimock]);
        attr.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |attr| {
        set_fallbacks([&mut attr.export, &mut attr.unimock]);
        attr.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn delegate_impl(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    delegate_impl::gen_delegate_impl(attr, input, |_| {})
}

#[proc_macro_attribute]
pub fn delegate_impl_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    delegate_impl::gen_delegate_impl(attr, input, |input| {
        set_fallbacks([&mut input.unimock]);
    })
}

fn set_fallbacks<const N: usize>(opts: [&mut Option<util::opt::SpanOpt<bool>>; N]) {
    for opt in opts.into_iter() {
        opt.get_or_insert(util::opt::SpanOpt::of(true));
    }
}
