use crate::{
    analyze_generics::TraitFn,
    generics::{FnDeps, TraitGenerics},
    idents::CrateIdents,
    opt::Opts,
    signature::{EntraitSignature, FnIndex, InputSig, ReceiverGeneration},
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

pub fn analyze_trait<'i>(
    item_trait: syn::ItemTrait,
    crate_idents: &CrateIdents,
    opts: &Opts,
) -> syn::Result<OutTrait> {
    let trait_span = item_trait.ident.span();
    let methods = item_trait
        .items
        .into_iter()
        .map(|trait_item| match trait_item {
            syn::TraitItem::Method(method) => {
                // FIXME: Report errors on default methods, etc
                Ok(method)
            }
            _ => Err(syn::Error::new(
                trait_item.span(),
                "Only methods are supported.",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let supertraits = if let Some(colon_token) = item_trait.colon_token {
        Supertraits::Some {
            colon_token,
            bounds: item_trait.supertraits,
        }
    } else {
        Supertraits::None
    };

    let trait_fns = methods
        .into_iter()
        .enumerate()
        .map(|(index, method)| {
            let input_sig = InputSig::new(&method.sig);
            let originally_async = input_sig.asyncness.is_some();
            let use_associated_future = input_sig.use_associated_future(opts);

            let mut entrait_sig = EntraitSignature::new(method.sig);

            if use_associated_future {
                entrait_sig.convert_to_associated_future(
                    FnIndex(index),
                    ReceiverGeneration::Rewrite,
                    trait_span,
                    crate_idents,
                );
            }

            TraitFn {
                deps: FnDeps::NoDeps,
                entrait_sig,
                originally_async,
            }
        })
        .collect();

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
        fns: trait_fns,
    })
}
