pub mod converter;
pub mod future;
pub mod lifetimes;

mod fn_params;

use std::ops::Deref;

use crate::generics::TraitIndirection;
use crate::idents::CrateIdents;

#[derive(Clone, Copy)]
pub struct InputSig<'s> {
    sig: &'s syn::Signature,
}

impl<'s> InputSig<'s> {
    pub fn new(sig: &'s syn::Signature) -> Self {
        Self { sig }
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
    pub et_lifetimes: Vec<EntraitLifetime>,
}

impl EntraitSignature {
    pub fn new(sig: syn::Signature) -> Self {
        Self {
            sig,
            associated_fut: None,
            et_lifetimes: vec![],
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

    fn et_lifetimes_in_assoc_future(&self) -> impl Iterator<Item = &'_ EntraitLifetime> {
        self.et_lifetimes
            .iter()
            .filter(|et| et.used_in_output.0 || matches!(et.source, SigComponent::Base))
    }

    fn et_lifetimes_in_assoc_future_except_base(
        &self,
    ) -> impl Iterator<Item = &'_ EntraitLifetime> {
        self.et_lifetimes.iter().filter(|et| et.used_in_output.0)
    }
}

#[derive(Clone)]
pub struct AssociatedFut {
    pub ident: syn::Ident,
    pub output: syn::Type,
    pub base_lifetime: syn::Lifetime,
}

/// Only used for associated future:
#[derive(Clone)]
pub struct EntraitLifetime {
    pub lifetime: syn::Lifetime,
    pub source: SigComponent,
    pub user_provided: UserProvidedLifetime,
    pub used_in_output: UsedInOutput,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SigComponent {
    Receiver,
    Param(usize),
    Output,
    Base,
}

#[derive(Clone, Copy)]
pub struct UserProvidedLifetime(bool);

#[derive(Clone, Copy)]
pub struct UsedInOutput(bool);

#[derive(Clone, Copy)]
pub enum ReceiverGeneration {
    Insert,
    Rewrite,
    None,
}
