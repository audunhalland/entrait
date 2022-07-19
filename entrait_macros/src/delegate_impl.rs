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

    let params_gen = generics.params_generator(generics::ImplementationGeneric(true));
    let args_gen = generics.arguments_generator();
    let mut where_clause_gen = generics.where_clause_generator();

    where_clause_gen.push_impl_predicate(parse_quote! {
        EntraitT: #trait_ident #args_gen + Sync
    });

    let method_impls = item_trait
        .items
        .iter()
        .filter_map(|trait_item| match trait_item {
            syn::TraitItem::Method(method) => Some(gen_method_impl(method)),
            _ => None,
        });

    let tokens = quote! {
        #item_trait

        impl #params_gen #trait_ident #args_gen for ::entrait::Impl<EntraitT> #where_clause_gen {
            #(#method_impls)*
        }
    };

    tokens.into()
}

fn gen_method_impl(method: &syn::TraitItemMethod) -> proc_macro2::TokenStream {
    let fn_sig = &method.sig;
    let fn_ident = &fn_sig.ident;
    let arguments = fn_sig.inputs.iter().filter_map(|arg| match arg {
        syn::FnArg::Receiver(_) => None,
        syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
            syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_token_stream()),
            _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
        },
    });

    quote! {
        #fn_sig {
            self.as_ref().#fn_ident(#(#arguments),*)
        }
    }
}
