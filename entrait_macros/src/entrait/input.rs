//!
//! inputs to procedural macros
//!
//!

use crate::util::opt::*;

use syn::parse::{Parse, ParseStream};

/// The `entrait` invocation for functions
pub struct EntraitAttr {
    pub trait_visibility: syn::Visibility,
    pub trait_ident: syn::Ident,
    pub opts: Opts,
}

impl EntraitAttr {
    pub fn no_deps_value(&self) -> bool {
        self.default_option(self.opts.no_deps, false).0
    }

    pub fn debug_value(&self) -> bool {
        self.default_option(self.opts.debug, false).0
    }

    pub fn async_strategy(&self) -> SpanOpt<AsyncStrategy> {
        self.default_option(self.opts.async_strategy, AsyncStrategy::NoHack)
    }

    pub fn export_value(&self) -> bool {
        self.default_option(self.opts.export, false).0
    }

    pub fn default_option<T>(&self, option: Option<SpanOpt<T>>, default: T) -> SpanOpt<T> {
        match option {
            Some(option) => option,
            None => SpanOpt(default, self.trait_ident.span()),
        }
    }
}

impl Parse for EntraitAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let trait_visibility: syn::Visibility = input.parse()?;

        let trait_ident = input.parse()?;

        let mut no_deps = None;
        let mut debug = None;
        let mut async_strategy = None;
        let mut export = None;
        let mut unimock = None;
        let mut mockall = None;

        while input.peek(syn::token::Comma) {
            input.parse::<syn::token::Comma>()?;

            match input.parse::<EntraitOpt>()? {
                EntraitOpt::NoDeps(opt) => no_deps = Some(opt),
                EntraitOpt::Debug(opt) => debug = Some(opt),
                EntraitOpt::AsyncTrait(opt) => {
                    async_strategy = Some(SpanOpt(AsyncStrategy::AsyncTrait, opt.1))
                }
                EntraitOpt::AssociatedFuture(opt) => {
                    async_strategy = Some(SpanOpt(AsyncStrategy::AssociatedFuture, opt.1))
                }
                EntraitOpt::Export(opt) => export = Some(opt),
                EntraitOpt::Unimock(opt) => unimock = Some(opt),
                EntraitOpt::Mockall(opt) => mockall = Some(opt),
            };
        }

        Ok(EntraitAttr {
            trait_visibility,
            trait_ident,
            opts: Opts {
                no_deps,
                debug,
                async_strategy,
                export,
                unimock,
                mockall,
            }
        })
    }
}

pub enum Input {
    Fn(InputFn),
    Trait(syn::ItemTrait),
}

///
/// The "body" that is decorated with entrait.
///
pub struct InputFn {
    pub fn_attrs: Vec<syn::Attribute>,
    pub fn_vis: syn::Visibility,
    pub fn_sig: syn::Signature,
    // don't try to parse fn_body, just pass through the tokens:
    pub fn_body: proc_macro2::TokenStream,
}

impl Parse for InputFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fn_attrs = input.call(syn::Attribute::parse_outer)?;
        let fn_vis = input.parse()?;
        let fn_sig: syn::Signature = input.parse()?;
        let fn_body = input.parse()?;

        Ok(InputFn {
            fn_attrs,
            fn_vis,
            fn_sig,
            fn_body,
        })
    }
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;

        // BUG (In theory): missing "unsafe" and "auto" traits
        if input.peek(syn::token::Trait) {
            let item_trait: syn::ItemTrait = input.parse()?;

            Ok(Input::Trait(
                syn::ItemTrait {
                    attrs,
                    vis,
                    ..item_trait
                }
            ))
        } else {
            let fn_sig: syn::Signature = input.parse()?;
            let fn_body = input.parse()?;

            Ok(Input::Fn(InputFn {
                fn_attrs: attrs,
                fn_vis: vis,
                fn_sig,
                fn_body,
            }))
        }
    }
}
