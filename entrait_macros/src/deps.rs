use crate::input::InputFn;
use syn::spanned::Spanned;

pub enum Deps<'f> {
    GenericOrAbsent {
        trait_bounds: Vec<&'f syn::TypeParamBound>,
    },
    Concrete(&'f syn::Type),
    NoDeps,
}

impl<'f> Deps<'f> {
    pub fn is_deps_param(&self, index: usize) -> bool {
        match self {
            Self::NoDeps => false,
            _ => index == 0,
        }
    }
}

pub fn analyze_deps<'f>(func: &'f InputFn) -> syn::Result<Deps<'f>> {
    let first_input = match func.fn_sig.inputs.first() {
        Some(fn_arg) => fn_arg,
        None => {
            return Ok(Deps::GenericOrAbsent {
                trait_bounds: vec![],
            });
        }
    };

    let pat_type = match first_input {
        syn::FnArg::Typed(pat_type) => pat_type,
        syn::FnArg::Receiver(_) => {
            return Err(syn::Error::new(
                first_input.span(),
                "Function cannot have a self receiver",
            ))
        }
    };

    extract_deps_from_type(func, pat_type, pat_type.ty.as_ref())
}

fn extract_deps_from_type<'f>(
    func: &'f InputFn,
    arg_pat: &'f syn::PatType,
    ty: &'f syn::Type,
) -> syn::Result<Deps<'f>> {
    match ty {
        syn::Type::ImplTrait(type_impl_trait) => {
            // Simple case, bounds are actually inline, no lookup necessary
            Ok(Deps::GenericOrAbsent {
                trait_bounds: extract_trait_bounds(&type_impl_trait.bounds),
            })
        }
        syn::Type::Path(type_path) => {
            // Type path. Should be defined as a generic parameter.
            if type_path.qself.is_some() {
                return Err(syn::Error::new(type_path.span(), "No self allowed"));
            }
            if type_path.path.leading_colon.is_some() {
                return Err(syn::Error::new(
                    type_path.span(),
                    "No leading colon allowed",
                ));
            }
            if type_path.path.segments.len() != 1 {
                return Ok(Deps::Concrete(ty));
            }

            let first_segment = type_path.path.segments.first().unwrap();

            match find_generic_bounds(func, &first_segment.ident) {
                Some(deps) => Ok(deps),
                None => Ok(Deps::Concrete(ty)),
            }
        }
        syn::Type::Reference(type_reference) => {
            extract_deps_from_type(func, arg_pat, type_reference.elem.as_ref())
        }
        syn::Type::Paren(paren) => extract_deps_from_type(func, arg_pat, paren.elem.as_ref()),
        _ => Err(syn::Error::new(
            ty.span(),
            format!("Cannot process this argument"),
        )),
    }
}

fn find_generic_bounds<'f>(func: &'f InputFn, generic_arg_ident: &syn::Ident) -> Option<Deps<'f>> {
    let generic_params = &func.fn_sig.generics.params;

    let matching_type_param = generic_params.into_iter().find_map(|param| match param {
        syn::GenericParam::Type(type_param) => {
            if &type_param.ident == generic_arg_ident {
                Some(type_param)
            } else {
                None
            }
        }
        _ => None,
    });

    let matching_type_param = match matching_type_param {
        Some(type_param) => type_param,
        None => return None,
    };

    // Extract "direct" bounds, not from where clause
    let mut trait_bounds = extract_trait_bounds(&matching_type_param.bounds);

    // Check the where clause too
    if let Some(where_clause) = &func.fn_sig.generics.where_clause {
        for predicate in &where_clause.predicates {
            match predicate {
                syn::WherePredicate::Type(predicate_type) => match &predicate_type.bounded_ty {
                    syn::Type::Path(type_path) => {
                        if type_path.qself.is_some() || type_path.path.leading_colon.is_some() {
                            continue;
                        }
                        if type_path.path.segments.len() != 1 {
                            continue;
                        }
                        let first_segment = type_path.path.segments.first().unwrap();

                        if &first_segment.ident == generic_arg_ident {
                            let where_paths = extract_trait_bounds(&predicate_type.bounds);

                            trait_bounds.extend(where_paths);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    Some(Deps::GenericOrAbsent { trait_bounds })
}

fn extract_trait_bounds(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::token::Add>,
) -> Vec<&syn::TypeParamBound> {
    bounds.iter().collect()
}
