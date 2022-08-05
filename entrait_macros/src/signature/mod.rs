pub mod converter;
pub mod future;

mod fn_params;
mod lifetimes;

use std::ops::Deref;

use crate::opt::AsyncStrategy;
use crate::opt::Opts;
use crate::opt::SpanOpt;

#[derive(Clone, Copy)]
pub struct InputSig<'s> {
    sig: &'s syn::Signature,
}

impl<'s> InputSig<'s> {
    pub fn new(sig: &'s syn::Signature) -> Self {
        Self { sig }
    }

    pub fn use_associated_future(&self, opts: &Opts) -> bool {
        matches!(
            (opts.async_strategy(), self.sig.asyncness),
            (SpanOpt(AsyncStrategy::AssociatedFuture, _), Some(_async))
        )
    }
}

impl<'s> Deref for InputSig<'s> {
    type Target = &'s syn::Signature;

    fn deref(&self) -> &Self::Target {
        &self.sig
    }
}

#[derive(Clone, Copy)]
pub struct FnIndex(pub usize);

pub struct InjectDynImplParam(pub bool);

/// The fn signature inside the trait
pub struct EntraitSignature {
    pub sig: syn::Signature,
    pub associated_fut_decl: Option<proc_macro2::TokenStream>,
    pub associated_fut_impl: Option<proc_macro2::TokenStream>,
    pub lifetimes: Vec<EntraitLifetime>,
}

/// Only used for associated future:
pub struct EntraitLifetime {
    pub lifetime: syn::Lifetime,
    pub source: SigComponent,
    pub user_provided: UserProvidedLifetime,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SigComponent {
    Receiver,
    Param(usize),
    Output,
}

pub struct UserProvidedLifetime(bool);

#[derive(Clone, Copy)]
pub enum ReceiverGeneration {
    Insert,
    Rewrite,
    None,
}
