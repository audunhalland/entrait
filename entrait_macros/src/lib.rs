//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

extern crate proc_macro;

use proc_macro::TokenStream;

mod entrait_fn;
mod entrait_trait;
mod generics;
mod input;
mod opt;

use input::Input;
use opt::AsyncStrategy;
use opt::Opts;

#[proc_macro_attribute]
pub fn entrait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    invoke(attr, input, |_| {})
}

#[proc_macro_attribute]
pub fn entrait_export(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export]);
    })
}

#[proc_macro_attribute]
pub fn entrait_use_async_trait(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        opts.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export]);
        opts.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        opts.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export]);
        opts.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock(attr: TokenStream, input: TokenStream) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export, &mut opts.unimock]);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.unimock]);
        opts.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock_use_async_trait(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export, &mut opts.unimock]);
        opts.set_fallback_async_strategy(AsyncStrategy::AsyncTrait);
    })
}

#[proc_macro_attribute]
pub fn entrait_unimock_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.unimock]);
        opts.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

#[proc_macro_attribute]
pub fn entrait_export_unimock_use_associated_future(
    attr: TokenStream,
    input: TokenStream,
) -> proc_macro::TokenStream {
    invoke(attr, input, |opts| {
        set_fallbacks([&mut opts.export, &mut opts.unimock]);
        opts.set_fallback_async_strategy(AsyncStrategy::AssociatedFuture);
    })
}

fn set_fallbacks<const N: usize>(opts: [&mut Option<opt::SpanOpt<bool>>; N]) {
    for opt in opts.into_iter() {
        opt.get_or_insert(opt::SpanOpt::of(true));
    }
}

fn invoke(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
    opts_modifier: impl FnOnce(&mut Opts),
) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as Input);

    let (result, debug) = match input {
        Input::Fn(input_fn) => {
            let mut attr = syn::parse_macro_input!(attr as entrait_fn::attr::EntraitFnAttr);
            opts_modifier(&mut attr.opts);

            (
                entrait_fn::output_tokens(&attr, input_fn),
                attr.debug_value(),
            )
        }
        Input::Trait(item_trait) => {
            let mut attr = syn::parse_macro_input!(attr as entrait_trait::EntraitTraitAttr);
            opts_modifier(&mut attr.opts);
            let debug = attr.opts.debug.map(|opt| *opt.value()).unwrap_or(false);

            (entrait_trait::output_tokens(attr, item_trait), debug)
        }
    };

    let output = match result {
        Ok(token_stream) => token_stream,
        Err(err) => err.into_compile_error(),
    };

    if debug {
        println!("{}", output);
    }

    proc_macro::TokenStream::from(output)
}
