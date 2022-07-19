use crate::util::generics;
use crate::util::opt::*;

use quote::quote;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::parse_quote;

pub struct DelegateImplAttr {
    pub unimock: Option<SpanOpt<bool>>,

    /// Mocking with mockall
    pub mockall: Option<SpanOpt<bool>>,
}

impl Parse for DelegateImplAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut unimock = None;
        let mut mockall = None;

        if !input.is_empty() {
            loop {
                match input.parse::<EntraitOpt>()? {
                    EntraitOpt::Unimock(opt) => unimock = Some(opt),
                    EntraitOpt::Mockall(opt) => mockall = Some(opt),
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

        Ok(Self { unimock, mockall })
    }
}

pub fn gen_delegate_impl(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
    attr_modifier: impl FnOnce(&mut DelegateImplAttr),
) -> proc_macro::TokenStream {
    let mut attr = syn::parse_macro_input!(attr as DelegateImplAttr);
    let item_trait = syn::parse_macro_input!(input as syn::ItemTrait);

    attr_modifier(&mut attr);

    let generics = generics::Generics::new(generics::Deps::NoDeps, item_trait.generics.clone());
    let trait_ident = &item_trait.ident;

    // NOTE: all of the trait attributes are outputted, unchanged

    let impl_attrs = item_trait.attrs.iter().filter(|attr| {
        matches!(
            attr.path.segments.last(),
            Some(last_segment) if last_segment.ident == "async_trait"
        )
    }).collect::<Vec<_>>();

    let params_gen = generics.params_generator(generics::ImplementationGeneric(true));
    let args_gen = generics.arguments_generator();
    let mut where_clause_gen = generics.where_clause_generator();

    where_clause_gen.push_impl_predicate(parse_quote! {
        EntraitT: #trait_ident #args_gen + Sync
    });

    let impl_assoc_types = item_trait
        .items
        .iter()
        .filter_map(|trait_item| match trait_item {
            syn::TraitItem::Type(trait_item_type) => Some(impl_assoc_type(trait_item_type)),
            _ => None
        });

    let method_items = item_trait
        .items
        .iter()
        .filter_map(|trait_item| match trait_item {
            syn::TraitItem::Method(method) => Some(gen_method(method)),
            _ => None,
        });

    let tokens = quote! {
        #item_trait

        #(#impl_attrs)*
        impl #params_gen #trait_ident #args_gen for ::entrait::Impl<EntraitT> #where_clause_gen {
            #(#impl_assoc_types)*
            #(#method_items)*
        }
    };

    tokens.into()
}

fn gen_method(method: &syn::TraitItemMethod) -> proc_macro2::TokenStream {
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

fn impl_assoc_type(assoc_type: &syn::TraitItemType) -> proc_macro2::TokenStream {
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
        syn::TypeParamBound::Trait(trait_bound) => {
            match trait_bound.path.segments.last() {
                Some(segment) if segment.ident == "Future" => Some(&segment.arguments),
                _ => None
            }
        }
        _ => None
    }
}
