use super::input;
use super::input::InputFn;
use syn::spanned::Spanned;

pub struct Generics {
    pub deps: Deps,
    pub trait_generics: syn::Generics,
    pub entrait_t: syn::Ident,
}

pub struct ImplementationGeneric(pub bool);

impl Generics {
    fn new(deps: Deps, trait_generics: syn::Generics) -> Self {
        Self {
            deps,
            trait_generics,
            entrait_t: syn::Ident::new("EntraitT", proc_macro2::Span::call_site()),
        }
    }

    pub fn params_generator(&self, impl_generic: ImplementationGeneric) -> ParamsGenerator {
        ParamsGenerator {
            trait_generics: &self.trait_generics,
            entrait_t: impl_generic.0.then_some(&self.entrait_t),
        }
    }

    pub fn arguments_generator(&self) -> ArgumentsGenerator {
        ArgumentsGenerator {
            trait_generics: &self.trait_generics,
        }
    }

    pub fn where_clause_generator(&self) -> WhereClauseGenerator {
        WhereClauseGenerator {
            trait_generics: &self.trait_generics,
            impl_predicates: Default::default(),
        }
    }
}

pub enum Deps {
    Generic {
        generic_param: Option<syn::Ident>,
        trait_bounds: Vec<syn::TypeParamBound>,
    },
    Concrete(Box<syn::Type>),
    NoDeps,
}

impl Deps {
    pub fn is_deps_param(&self, index: usize) -> bool {
        match self {
            Self::NoDeps => false,
            _ => index == 0,
        }
    }
}

pub fn analyze_generics(func: &InputFn, attr: &input::EntraitAttr) -> syn::Result<Generics> {
    if attr.no_deps_value() {
        return Ok(Generics::new(
            Deps::NoDeps,
            clone_type_generics(&func.fn_sig.generics),
        ));
    }

    let first_input =
        match func.fn_sig.inputs.first() {
            Some(fn_arg) => fn_arg,
            None => return Err(syn::Error::new(
                func.fn_sig.ident.span(),
                "Function must have a dependency 'receiver' as its first parameter. Pass `no_deps` to entrait to disable dependency injection.",
            )),
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
) -> syn::Result<Generics> {
    match ty {
        syn::Type::ImplTrait(type_impl_trait) => {
            // Simple case, bounds are actually inline, no lookup necessary
            Ok(Generics::new(
                Deps::Generic {
                    generic_param: None,
                    trait_bounds: extract_trait_bounds(&type_impl_trait.bounds),
                },
                clone_type_generics(&func.fn_sig.generics),
            ))
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
                return Ok(Generics::new(
                    Deps::Concrete(Box::new(ty.clone())),
                    clone_type_generics(&func.fn_sig.generics),
                ));
            }

            let first_segment = type_path.path.segments.first().unwrap();

            match find_deps_generic_bounds(func, &first_segment.ident) {
                Some(generics) => Ok(generics),
                None => Ok(Generics::new(
                    Deps::Concrete(Box::new(ty.clone())),
                    clone_type_generics(&func.fn_sig.generics),
                )),
            }
        }
        syn::Type::Reference(type_reference) => {
            extract_deps_from_type(func, arg_pat, type_reference.elem.as_ref())
        }
        syn::Type::Paren(paren) => extract_deps_from_type(func, arg_pat, paren.elem.as_ref()),
        ty => Ok(Generics::new(
            Deps::Concrete(Box::new(ty.clone())),
            clone_type_generics(&func.fn_sig.generics),
        )),
    }
}

fn find_deps_generic_bounds(func: &InputFn, generic_param_ident: &syn::Ident) -> Option<Generics> {
    let generics = &func.fn_sig.generics;
    let generic_params = &generics.params;

    let (matching_index, matching_type_param) =
        generic_params
            .into_iter()
            .enumerate()
            .find_map(|(index, param)| match param {
                syn::GenericParam::Type(type_param) => {
                    if &type_param.ident == generic_param_ident {
                        Some((index, type_param))
                    } else {
                        None
                    }
                }
                _ => None,
            })?;

    let mut remaining_params =
        syn::punctuated::Punctuated::<syn::GenericParam, syn::token::Comma>::new();

    for (index, param) in generic_params.iter().enumerate() {
        if index != matching_index {
            remaining_params.push(param.clone());
        }
    }

    // Extract "direct" bounds, not from where clause
    let mut deps_trait_bounds = extract_trait_bounds(&matching_type_param.bounds);

    // Check the where clause too
    let new_where_clause = if let Some(where_clause) = &generics.where_clause {
        let mut new_predicates =
            syn::punctuated::Punctuated::<syn::WherePredicate, syn::token::Comma>::new();

        for predicate in &where_clause.predicates {
            match predicate {
                syn::WherePredicate::Type(predicate_type) => match &predicate_type.bounded_ty {
                    syn::Type::Path(type_path) => {
                        if type_path.qself.is_some() || type_path.path.leading_colon.is_some() {
                            new_predicates.push(predicate.clone());
                            continue;
                        }
                        if type_path.path.segments.len() != 1 {
                            new_predicates.push(predicate.clone());
                            continue;
                        }
                        let first_segment = type_path.path.segments.first().unwrap();

                        if &first_segment.ident == generic_param_ident {
                            let where_paths = extract_trait_bounds(&predicate_type.bounds);

                            deps_trait_bounds.extend(where_paths);
                        }
                    }
                    _ => {
                        new_predicates.push(predicate.clone());
                    }
                },
                _ => {
                    new_predicates.push(predicate.clone());
                }
            }
        }

        if !new_predicates.is_empty() {
            Some(syn::WhereClause {
                where_token: where_clause.where_token,
                predicates: new_predicates,
            })
        } else {
            None
        }
    } else {
        None
    };

    let has_modified_generics = !remaining_params.is_empty() || new_where_clause.is_some();

    let modified_generics = syn::Generics {
        lt_token: generics.lt_token.filter(|_| has_modified_generics),
        params: remaining_params,
        where_clause: new_where_clause,
        gt_token: generics.gt_token.filter(|_| has_modified_generics),
    };

    Some(Generics::new(
        Deps::Generic {
            generic_param: Some(generic_param_ident.clone()),
            trait_bounds: deps_trait_bounds,
        },
        modified_generics,
    ))
}

fn extract_trait_bounds(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::token::Add>,
) -> Vec<syn::TypeParamBound> {
    bounds.iter().cloned().collect()
}

fn clone_type_generics(generics: &syn::Generics) -> syn::Generics {
    let mut type_generics = syn::Generics {
        lt_token: generics.lt_token,
        params: syn::punctuated::Punctuated::new(),
        where_clause: generics.where_clause.clone(),
        gt_token: generics.gt_token,
    };

    for param in &generics.params {
        match param {
            syn::GenericParam::Type(_) => type_generics.params.push(param.clone()),
            syn::GenericParam::Const(_) => type_generics.params.push(param.clone()),
            syn::GenericParam::Lifetime(_) => {}
        }
    }

    type_generics
}

// Params as in impl<..Param>
pub struct ParamsGenerator<'g> {
    trait_generics: &'g syn::Generics,
    entrait_t: Option<&'g syn::Ident>,
}

impl<'g> quote::ToTokens for ParamsGenerator<'g> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let has_trait_generics = !self.trait_generics.params.is_empty();

        if !has_trait_generics && self.entrait_t.is_none() {
            return;
        }

        syn::token::Lt::default().to_tokens(tokens);
        if let Some(entrait_t) = &self.entrait_t {
            entrait_t.to_tokens(tokens);

            if has_trait_generics {
                syn::token::Comma::default().to_tokens(tokens);
            }
        }
        self.trait_generics.params.to_tokens(tokens);
        syn::token::Gt::default().to_tokens(tokens);
    }
}

// Args as in impl<..Param> T for U<..Arg>
pub struct ArgumentsGenerator<'g> {
    trait_generics: &'g syn::Generics,
}

impl<'g> quote::ToTokens for ArgumentsGenerator<'g> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let has_trait_generics = !self.trait_generics.params.is_empty();

        if !has_trait_generics {
            return;
        }

        syn::token::Lt::default().to_tokens(tokens);
        for pair in self.trait_generics.params.pairs() {
            match pair.value() {
                syn::GenericParam::Type(type_param) => {
                    type_param.ident.to_tokens(tokens);
                }
                syn::GenericParam::Lifetime(lifetime_def) => {
                    lifetime_def.lifetime.to_tokens(tokens);
                }
                syn::GenericParam::Const(const_param) => {
                    const_param.ident.to_tokens(tokens);
                }
            }
            pair.punct().to_tokens(tokens);
        }

        syn::token::Gt::default().to_tokens(tokens);
    }
}

/// Join where clauses from the input function and the required ones for Impl<T>
pub struct WhereClauseGenerator<'g> {
    trait_generics: &'g syn::Generics,
    impl_predicates: syn::punctuated::Punctuated<syn::WherePredicate, syn::token::Comma>,
}

impl<'g> WhereClauseGenerator<'g> {
    pub fn push_impl_predicate(&mut self, predicate: syn::WherePredicate) {
        self.impl_predicates.push(predicate);
    }
}

impl<'g> quote::ToTokens for WhereClauseGenerator<'g> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let has_impl_preds = !self.impl_predicates.is_empty();
        if self.trait_generics.where_clause.is_none() && !has_impl_preds {
            return;
        }

        syn::token::Where::default().to_tokens(tokens);
        if let Some(where_clause) = &self.trait_generics.where_clause {
            let mut trailing_comma = false;
            for pair in where_clause.predicates.pairs() {
                pair.value().to_tokens(tokens);
                trailing_comma = match pair.punct() {
                    Some(punct) => {
                        punct.to_tokens(tokens);
                        true
                    }
                    None => false,
                }
            }

            if has_impl_preds && !trailing_comma {
                syn::token::Comma::default().to_tokens(tokens);
            }
        }

        self.impl_predicates.to_tokens(tokens);
    }
}
