//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

extern crate proc_macro;

use proc_macro::TokenStream;

mod entrait;
mod util;

use util::opt::AsyncStrategy;

#[proc_macro_attribute]
pub fn entrait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |_| {})
}

#[proc_macro_attribute]
pub fn entrait_export(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export]);
    })
}

#[proc_macro_attribute]
pub fn entrait_use_async_trait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        opts.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export]);
        opts.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        opts.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export]);
        opts.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export, &mut opts.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.unimock]);
        opts.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export, &mut opts.unimock]);
        opts.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.unimock]);
        opts.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    entrait::invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export, &mut opts.unimock]);
        opts.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

fn set_fallbacks<const N: usize>(opts: [&mut Option<util::opt::SpanOpt<bool>>; N]) {
    for opt in opts.into_iter() {
        opt.get_or_insert(util::opt::SpanOpt::of(true));
    }
}
