pub mod converter;
pub mod future;
pub mod lifetimes;

mod fn_params;

use std::ops::Deref;

use crate::generics::TraitIndirection;
use crate::idents::CrateIdents;
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
        /*
        matches!(
            (opts.async_strategy(), self.sig.asyncness),
            (SpanOpt(AsyncStrategy::AssociatedFuture, _), Some(_async))
        )
        */
        false
    }
}

impl<'s> Deref for InputSig<'s> {
    type Target = &'s syn::Signature;

    fn deref(&self) -> &Self::Target {
        &self.sig
    }
}

pub enum ImplReceiverKind {
    // (&self, ..)
    SelfRef,
    // (&__impl, ..)
    StaticImpl,
    // (&self, &__impl, ..)
    DynamicImpl,
}

/// The fn signature inside the trait
#[derive(Clone)]
pub struct EntraitSignature {
    pub sig: syn::Signature,
    pub associated_fut: Option<AssociatedFut>,
    pub lifetimes: Vec<EntraitLifetime>,
}

impl EntraitSignature {
    pub fn new(sig: syn::Signature) -> Self {
        Self {
            sig,
            associated_fut: None,
            lifetimes: vec![],
        }
    }

    pub fn associated_fut_decl<'s>(
        &'s self,
        trait_indirection: TraitIndirection,
        crate_idents: &'s CrateIdents,
    ) -> Option<future::FutDecl<'s>> {
        self.associated_fut
            .as_ref()
            .map(|associated_fut| future::FutDecl {
                signature: self,
                associated_fut,
                trait_indirection,
                crate_idents,
            })
    }

    pub fn associated_fut_impl<'s>(
        &'s self,
        trait_indirection: TraitIndirection,
        crate_idents: &'s CrateIdents,
    ) -> Option<future::FutImpl<'s>> {
        self.associated_fut
            .as_ref()
            .map(|associated_fut| future::FutImpl {
                signature: self,
                associated_fut,
                trait_indirection,
                crate_idents,
            })
    }
}

#[derive(Clone)]
pub struct AssociatedFut {
    pub ident: syn::Ident,
    pub output: syn::Type,
}

/// Only used for associated future:
#[derive(Clone)]
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

#[derive(Clone, Copy)]
pub struct UserProvidedLifetime(bool);

#[derive(Clone, Copy)]
pub enum ReceiverGeneration {
    Insert,
    Rewrite,
    None,
}
