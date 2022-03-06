use crate::input::EntraitFn;
use syn::spanned::Spanned;

pub fn extract_first_arg_bounds(func: &EntraitFn) -> syn::Result<Vec<&syn::Path>> {
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

    extract_generic_bounds_from_type(func, pat_type.ty.as_ref())
}

fn extract_generic_bounds_from_type<'f>(
    func: &'f EntraitFn,
    ty: &'f syn::Type,
) -> syn::Result<Vec<&'f syn::Path>> {
    match ty {
        syn::Type::ImplTrait(type_impl_trait) => {
            // Simple case, bounds are actually inline, no lookup necessary
            Ok(extract_path_bounds(&type_impl_trait.bounds))
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
                return Err(syn::Error::new(
                    type_path.span(),
                    "Type for this argument should refer to a generic parameter",
                ));
            }

            let first_segment = type_path.path.segments.first().unwrap();

            extract_generic_bounds(func, &first_segment.ident)
        }
        syn::Type::Reference(type_reference) => {
            extract_generic_bounds_from_type(func, type_reference.elem.as_ref())
        }
        syn::Type::Paren(paren) => extract_generic_bounds_from_type(func, paren.elem.as_ref()),
        unknown => Err(syn::Error::new(
            ty.span(),
            format!("Cannot process this type: {unknown:?}"),
        )),
    }
}

fn extract_generic_bounds<'f>(
    func: &'f EntraitFn,
    generic_arg_ident: &syn::Ident,
) -> syn::Result<Vec<&'f syn::Path>> {
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
        None => {
            return Err(syn::Error::new(
                generic_arg_ident.span(),
                "Found no matching generic parameter",
            ));
        }
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

    Ok(paths)
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
