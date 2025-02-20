pub mod converter;

mod fn_params;

use std::ops::Deref;

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
    #[expect(unused)]
    pub et_lifetimes: Vec<EntraitLifetime>,
}

impl EntraitSignature {
    pub fn new(sig: syn::Signature) -> Self {
        Self {
            sig,
            et_lifetimes: vec![],
        }
    }
}

/// Only used for associated future:
#[derive(Clone)]
pub struct EntraitLifetime {
    #[expect(unused)]
    pub lifetime: syn::Lifetime,
}

#[derive(Clone, Copy)]
pub enum ReceiverGeneration {
    Insert,
    Rewrite,
    None,
}
