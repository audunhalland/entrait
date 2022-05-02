use crate::input::InputFn;
use syn::spanned::Spanned;

pub enum Deps<'f> {
    Bounds(Vec<&'f syn::Path>),
    Concrete(&'f syn::Type),
}

pub fn analyze_deps<'f>(func: &'f InputFn) -> syn::Result<Deps<'f>> {
    let first_input = match func.fn_sig.inputs.first() {
        Some(fn_arg) => fn_arg,
        None => {
            return Err(syn::Error::new(
                func.fn_sig.inputs.span(),
                "Cannot generate mock because function has no arguments",
            ));
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
            Ok(Deps::Bounds(extract_path_bounds(&type_impl_trait.bounds)))
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
    let mut paths = extract_path_bounds(&matching_type_param.bounds);

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
                            let where_paths = extract_path_bounds(&predicate_type.bounds);

                            paths.extend(where_paths);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    Some(Deps::Bounds(paths))
}

fn extract_path_bounds(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::token::Add>,
) -> Vec<&syn::Path> {
    let mut paths: Vec<&syn::Path> = vec![];

    for type_param_bound in bounds.iter() {
        match type_param_bound {
            syn::TypeParamBound::Trait(trait_bound) => {
                paths.push(&trait_bound.path);
            }
            _ => {}
        }
    }

    paths
}
