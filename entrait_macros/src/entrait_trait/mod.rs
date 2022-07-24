//! Implementation for invoking entrait on a trait!

use crate::generics;
use crate::opt::*;

use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};

pub struct EntraitTraitAttr {
    pub opts: Opts,
}

impl Parse for EntraitTraitAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut debug = None;
        let mut unimock = None;
        let mut mockall = None;

        if !input.is_empty() {
            loop {
                match input.parse::<EntraitOpt>()? {
                    EntraitOpt::Unimock(opt) => unimock = Some(opt),
                    EntraitOpt::Mockall(opt) => mockall = Some(opt),
                    EntraitOpt::Debug(opt) => debug = Some(opt),
                    entrait_opt => {
                        return Err(syn::Error::new(entrait_opt.span(), "Unsupported option"))
                    }
                };

                if input.peek(syn::token::Comma) {
                    input.parse::<syn::token::Comma>()?;
                } else {
                    break;
                }
            }
        }

        Ok(Self {
            opts: Opts {
                no_deps: None,
                debug,
                async_strategy: None,
                export: None,
                unimock,
                mockall,
            },
        })
    }
}

pub fn output_tokens(
    attr: EntraitTraitAttr,
    item_trait: syn::ItemTrait,
) -> syn::Result<proc_macro2::TokenStream> {
    let generics = generics::Generics::new(
        generics::Deps::NoDeps {
            idents: generics::GenericIdents::new(item_trait.ident.span()),
        },
        item_trait.generics.clone(),
    );
    let generic_idents = match &generics.deps {
        generics::Deps::NoDeps { idents } => idents,
        _ => panic!(),
    };
    let trait_ident = &item_trait.ident;

    // NOTE: all of the trait _input attributes_ are outputted, unchanged

    let opt_unimock_attr = if attr.opts.unimock.map(|opt| *opt.value()).unwrap_or(false) {
        Some(quote! {
            #[::entrait::__unimock::unimock(prefix=::entrait::__unimock)]
        })
    } else {
        None
    };
    let opt_mockall_automock_attr = if attr.opts.mockall.map(|opt| *opt.value()).unwrap_or(false) {
        Some(quote! { #[::mockall::automock] })
    } else {
        None
    };

    let impl_attrs = item_trait
        .attrs
        .iter()
        .filter(|attr| {
            matches!(
                attr.path.segments.last(),
                Some(last_segment) if last_segment.ident == "async_trait"
            )
        })
        .collect::<Vec<_>>();

    let params = generics.params_generator();
    let args = generics.arguments_generator();
    let self_ty = generic_idents.impl_path(item_trait.ident.span());
    let where_clause = ImplWhereClause {
        trait_ident,
        generics: &generics,
        generic_idents,
        span: item_trait.ident.span(),
    };

    let impl_assoc_types = item_trait
        .items
        .iter()
        .filter_map(|trait_item| match trait_item {
            syn::TraitItem::Type(trait_item_type) => Some(impl_assoc_type(trait_item_type)),
            _ => None,
        });

    let method_items = item_trait
        .items
        .iter()
        .filter_map(|trait_item| match trait_item {
            syn::TraitItem::Method(method) => Some(gen_method(method)),
            _ => None,
        });

    let tokens = quote! {
        #opt_unimock_attr
        #opt_mockall_automock_attr
        #item_trait

        #(#impl_attrs)*
        impl #params #trait_ident #args for #self_ty #where_clause {
            #(#impl_assoc_types)*
            #(#method_items)*
        }
    };

    Ok(tokens)
}

fn gen_method(method: &syn::TraitItemMethod) -> TokenStream {
    let fn_sig = &method.sig;
    let fn_ident = &fn_sig.ident;
    let arguments = fn_sig.inputs.iter().filter_map(|arg| match arg {
        syn::FnArg::Receiver(_) => None,
        syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
            syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_token_stream()),
            _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
        },
    });
    let opt_dot_await = fn_sig.asyncness.map(|_| quote! { .await });

    quote! {
        #fn_sig {
            self.as_ref().#fn_ident(#(#arguments),*) #opt_dot_await
        }
    }
}

fn impl_assoc_type(assoc_type: &syn::TraitItemType) -> TokenStream {
    if let Some(future_arguments) = assoc_type.bounds.iter().find_map(find_future_arguments) {
        let ident = &assoc_type.ident;
        let generics = &assoc_type.generics;
        let where_clause = &generics.where_clause;

        quote! {
            type #ident #generics = impl ::core::future::Future #future_arguments #where_clause;
        }
    } else {
        // Probably a compile error
        quote! { #assoc_type }
    }
}

fn find_future_arguments(bound: &syn::TypeParamBound) -> Option<&syn::PathArguments> {
    match bound {
        syn::TypeParamBound::Trait(trait_bound) => match trait_bound.path.segments.last() {
            Some(segment) if segment.ident == "Future" => Some(&segment.arguments),
            _ => None,
        },
        _ => None,
    }
}

struct ImplWhereClause<'g> {
    trait_ident: &'g syn::Ident,
    generics: &'g generics::Generics,
    generic_idents: &'g generics::GenericIdents,
    span: proc_macro2::Span,
}

impl<'g> quote::ToTokens for ImplWhereClause<'g> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        syn::token::Where(self.span).to_tokens(tokens);

        // T: Trait<G> + Sync
        {
            // T:
            self.generic_idents.impl_t.to_tokens(tokens);
            syn::token::Colon(self.span).to_tokens(tokens);

            // Trait<G>
            self.trait_ident.to_tokens(tokens);
            self.generics.arguments_generator().to_tokens(tokens);

            // + Sync
            syn::token::Add(self.span).to_tokens(tokens);
            syn::Ident::new("Sync", self.span).to_tokens(tokens);
        }

        if let Some(where_clause) = &self.generics.trait_generics.where_clause {
            syn::token::Comma(self.span).to_tokens(tokens);
            where_clause.predicates.to_tokens(tokens);
        }
    }
}
