use crate::token_util::{push_tokens, Punctuator};

pub struct Generics {
    pub deps: Deps,
    pub trait_generics: syn::Generics,
}

pub struct UseAssociatedFuture(pub bool);

impl Generics {
    pub fn new(deps: Deps, trait_generics: syn::Generics) -> Self {
        Self {
            deps,
            trait_generics,
        }
    }

    pub fn params_generator(&self, use_associated_future: UseAssociatedFuture) -> ParamsGenerator {
        ParamsGenerator {
            trait_generics: &self.trait_generics,
            impl_t: match &self.deps {
                Deps::Generic { idents, .. } => Some(&idents.impl_t),
                Deps::NoDeps { idents } => Some(&idents.impl_t),
                Deps::Concrete(_) => None,
            },
            use_associated_future,
        }
    }

    pub fn arguments_generator(&self) -> ArgumentsGenerator {
        ArgumentsGenerator {
            trait_generics: &self.trait_generics,
        }
    }
}

pub enum Deps {
    Generic {
        generic_param: Option<syn::Ident>,
        trait_bounds: Vec<syn::TypeParamBound>,
        idents: GenericIdents,
    },
    Concrete(Box<syn::Type>),
    NoDeps {
        idents: GenericIdents,
    },
}

pub struct GenericIdents {
    /// "entrait"
    pub entrait_crate: syn::Ident,

    /// "Impl"
    pub impl_self: syn::Ident,

    /// The "T" in `Impl<T>`
    pub impl_t: syn::Ident,
}

impl GenericIdents {
    pub fn new(span: proc_macro2::Span) -> Self {
        Self {
            entrait_crate: syn::Ident::new("entrait", span),
            impl_self: syn::Ident::new("Impl", span),
            impl_t: syn::Ident::new("EntraitT", span),
        }
    }

    /// `::entrait::Impl<EntraitT>`
    pub fn impl_path(&self, span: proc_macro2::Span) -> ImplPath<'_> {
        ImplPath(self, span)
    }
}

/// `::entrait::Impl<EntraitT>`
pub struct ImplPath<'g>(&'g GenericIdents, proc_macro2::Span);

impl<'g> quote::ToTokens for ImplPath<'g> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let span = self.1;

        push_tokens!(
            stream,
            syn::token::Colon2(span),
            self.0.entrait_crate,
            syn::token::Colon2(span),
            self.0.impl_self,
            syn::token::Lt(span),
            self.0.impl_t,
            syn::token::Gt(span)
        );
    }
}

// Params as in impl<..Param>
pub struct ParamsGenerator<'g> {
    trait_generics: &'g syn::Generics,
    impl_t: Option<&'g syn::Ident>,
    use_associated_future: UseAssociatedFuture,
}

impl<'g> quote::ToTokens for ParamsGenerator<'g> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Lt::default(),
            syn::token::Comma::default(),
            syn::token::Gt::default(),
        );

        if let Some(impl_t) = &self.impl_t {
            punctuator.push_fn(|stream| {
                push_tokens!(
                    stream,
                    impl_t,
                    syn::token::Colon::default(),
                    syn::Ident::new("Sync", proc_macro2::Span::call_site())
                );

                if self.use_associated_future.0 {
                    // Deps must be 'static for zero-cost futures to work
                    push_tokens!(
                        stream,
                        syn::token::Add::default(),
                        syn::Lifetime::new("'static", proc_macro2::Span::call_site())
                    );
                }
            });
        }

        for param in &self.trait_generics.params {
            punctuator.push(param);
        }
    }
}

// Args as in impl<..Param> T for U<..Arg>
pub struct ArgumentsGenerator<'g> {
    trait_generics: &'g syn::Generics,
}

impl<'g> quote::ToTokens for ArgumentsGenerator<'g> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Lt::default(),
            syn::token::Comma::default(),
            syn::token::Gt::default(),
        );

        for pair in self.trait_generics.params.pairs() {
            match pair.value() {
                syn::GenericParam::Type(type_param) => {
                    punctuator.push(&type_param.ident);
                }
                syn::GenericParam::Lifetime(lifetime_def) => {
                    punctuator.push(&lifetime_def.lifetime);
                }
                syn::GenericParam::Const(const_param) => {
                    punctuator.push(&const_param.ident);
                }
            }
        }
    }
}
