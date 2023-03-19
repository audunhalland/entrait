use crate::{
    analyze_generics::TraitFn,
    generics::{FnDeps, TraitGenerics},
    signature::EntraitSignature,
    trait_codegen::{self, Supertraits},
};

use syn::spanned::Spanned;

#[derive(Clone)]
pub struct OutTrait {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub trait_token: syn::token::Trait,
    pub generics: TraitGenerics,
    pub ident: syn::Ident,
    pub supertraits: trait_codegen::Supertraits,
    pub fns: Vec<TraitFn>,
}

pub fn analyze_trait(item_trait: syn::ItemTrait) -> syn::Result<OutTrait> {
    let mut associated_types = vec![];
    let mut fns = vec![];

    for item in item_trait.items.into_iter() {
        match item {
            syn::TraitItem::Fn(method) => {
                let originally_async = method.sig.asyncness.is_some();

                let entrait_sig = EntraitSignature::new(method.sig);

                fns.push(TraitFn {
                    deps: FnDeps::NoDeps,
                    attrs: method.attrs,
                    entrait_sig,
                    originally_async,
                });
            }
            syn::TraitItem::Type(ty) => {
                associated_types.push(ty);
            }
            item => {
                return Err(syn::Error::new(
                    item.span(),
                    "Entrait does not support this kind of trait item.",
                ));
            }
        }
    }

    let supertraits = if let Some(colon_token) = item_trait.colon_token {
        Supertraits::Some {
            colon_token,
            bounds: item_trait.supertraits,
        }
    } else {
        Supertraits::None
    };

    Ok(OutTrait {
        attrs: item_trait.attrs,
        vis: item_trait.vis,
        trait_token: item_trait.trait_token,
        ident: item_trait.ident,
        generics: TraitGenerics {
            params: item_trait.generics.params,
            where_predicates: item_trait
                .generics
                .where_clause
                .map(|where_clause| where_clause.predicates)
                .unwrap_or_default(),
        },
        supertraits,
        fns,
    })
}
