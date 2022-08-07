use crate::{
    analyze_generics::TraitFn,
    generics::{FnDeps, TraitGenerics},
    opt::Opts,
    signature::{lifetimes, AssociatedFut, EntraitSignature, InputSig, ReceiverGeneration},
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

pub fn analyze_trait<'i>(item_trait: syn::ItemTrait, opts: &Opts) -> syn::Result<OutTrait> {
    let trait_span = item_trait.ident.span();
    let mut associated_types = vec![];
    let mut fns = vec![];

    for item in item_trait.items.into_iter() {
        match item {
            syn::TraitItem::Method(method) => {
                let input_sig = InputSig::new(&method.sig);
                let originally_async = input_sig.asyncness.is_some();
                let use_associated_future = input_sig.use_associated_future(opts);

                let mut entrait_sig = EntraitSignature::new(method.sig);

                if use_associated_future {
                    entrait_sig.convert_to_associated_future(ReceiverGeneration::None, trait_span);
                } else {
                    lifetimes::collect_lifetimes(&mut entrait_sig, ReceiverGeneration::Rewrite);
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
        let future_bound = match find_future_bound(&associated_type) {
            Some(bound) => bound,
            None => return Err(syn::Error::new(associated_type.span(), "This associated type is not a future. Only returned futures are accepted as associated types.")),
        };
        let output_binding = match find_output_binding(future_bound) {
            Some(binding) => binding,
            None => return Err(syn::Error::new(associated_type.span(), "No Output found")),
        };
        let output_ty = output_binding.ty.clone();

        let trait_fn = fns
            .iter_mut()
            .find(|trait_fn| is_return_type_associated_to(&trait_fn.sig().output, &associated_type))
            .ok_or_else(|| {
                syn::Error::new(
                    associated_type.span(),
                    "Did not find a method returning this associated future",
                )
            })?;

        trait_fn.entrait_sig.associated_fut = Some(AssociatedFut {
            ident: associated_type.ident,
            output: output_ty,
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

fn find_future_bound(associated_type: &syn::TraitItemType) -> Option<&syn::TraitBound> {
    associated_type.bounds.iter().find_map(|bound| {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            if trait_bound
                .path
                .segments
                .iter()
                .any(|segment| segment.ident == "Future")
            {
                Some(trait_bound)
            } else {
                None
            }
        } else {
            None
        }
    })
}

fn find_output_binding(bound: &syn::TraitBound) -> Option<&syn::Binding> {
    let last_segment = bound.path.segments.last()?;

    match &last_segment.arguments {
        syn::PathArguments::AngleBracketed(arguments) => {
            arguments.args.iter().find_map(|arg| match arg {
                syn::GenericArgument::Binding(binding) => {
                    if binding.ident == "Output" {
                        Some(binding)
                    } else {
                        None
                    }
                }
                _ => None,
            })
        }
        _ => None,
    }
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
