use crate::{
    analyze_generics::TraitFn,
    generics::{FnDeps, TraitGenerics},
    idents::CrateIdents,
    opt::Opts,
    signature::{EntraitSignature, FnIndex, InputSig, ReceiverGeneration},
    trait_codegen::{self, Supertraits},
};

use quote::quote;
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
    let mut associated_types = vec![];
    let mut fns = vec![];

    for (index, item) in item_trait.items.into_iter().enumerate() {
        match item {
            syn::TraitItem::Method(method) => {
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

                fns.push(TraitFn {
                    deps: FnDeps::NoDeps,
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

    // Find types that are future return values
    for associated_type in associated_types {
        if !is_future(&associated_type) {
            return Err(syn::Error::new(associated_type.span(), "This associated type is not a future. Only returned futures are accepted as associated types."));
        }

        let trait_fn = fns
            .iter_mut()
            .find(|trait_fn| is_return_type_associated_to(&trait_fn.sig().output, &associated_type))
            .ok_or_else(|| {
                syn::Error::new(
                    associated_type.span(),
                    "Did not find a method returning this associated future",
                )
            })?;

        trait_fn.entrait_sig.associated_fut_decl = Some(quote! { #associated_type });

        let syn::TraitItemType {
            attrs,
            type_token,
            ident,
            generics,
            colon_token: _,
            bounds,
            default: _,
            semi_token,
        } = associated_type;
        let where_clause = &generics.where_clause;

        trait_fn.entrait_sig.associated_fut_impl = Some(quote! {
            #(#attrs)* #type_token #ident #generics = impl #bounds
            #where_clause #semi_token
        });
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

fn is_future(associated_type: &syn::TraitItemType) -> bool {
    for bound in &associated_type.bounds {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            if trait_bound
                .path
                .segments
                .iter()
                .any(|segment| segment.ident == "Future")
            {
                return true;
            }
        }
    }

    false
}

fn is_return_type_associated_to(
    return_type: &syn::ReturnType,
    associated: &syn::TraitItemType,
) -> bool {
    fn path_association_match(path: &syn::Path, associated: &syn::TraitItemType) -> Option<()> {
        let mut path_iter = path.segments.iter();
        let first_segment = path_iter.next()?;

        if first_segment.ident != "Self" {
            return None;
        }

        let second_segment = path_iter.next()?;

        if second_segment.ident == associated.ident {
            Some(())
        } else {
            None
        }
    }

    match return_type {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => match ty.as_ref() {
            syn::Type::Path(type_path) => {
                path_association_match(&type_path.path, associated).is_some()
            }
            _ => false,
        },
    }
}
