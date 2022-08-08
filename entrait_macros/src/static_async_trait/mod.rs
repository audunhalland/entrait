use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{spanned::Spanned, ImplItemMethod, ItemTrait};

use crate::{
    generics::TraitIndirection,
    idents::CrateIdents,
    signature::{EntraitSignature, ReceiverGeneration},
};

pub fn output_tokens(item: syn::Item) -> syn::Result<TokenStream> {
    match item {
        syn::Item::Trait(item_trait) => process_trait(item_trait),
        syn::Item::Impl(item_impl) => process_impl(item_impl),
        other => Err(syn::Error::new(
            other.span(),
            "Cannot make this static-async",
        )),
    }
}

fn process_trait(item_trait: syn::ItemTrait) -> syn::Result<TokenStream> {
    let crate_idents = CrateIdents::new(item_trait.ident.span());
    let trait_span = item_trait.ident.span();

    let ItemTrait {
        attrs,
        vis,
        unsafety,
        auto_token,
        trait_token,
        ident,
        generics,
        colon_token,
        supertraits,
        brace_token: _,
        items,
    } = item_trait;
    let mut new_items = TokenStream::new();

    for item in items.into_iter() {
        match item {
            syn::TraitItem::Method(method) => {
                if method.sig.asyncness.is_some() {
                    let (sig, has_receiver) = convert_sig(method.sig, trait_span);
                    let fut = sig.associated_fut_decl(
                        if has_receiver {
                            TraitIndirection::None
                        } else {
                            TraitIndirection::Static
                        },
                        &crate_idents,
                    );
                    let trait_fn_sig = &sig.sig;

                    quote! {
                        #fut
                        #trait_fn_sig;
                    }
                    .to_tokens(&mut new_items);
                } else {
                    method.to_tokens(&mut new_items);
                }
            }
            item => {
                item.to_tokens(&mut new_items);
            }
        }
    }

    Ok(quote! {
        #(#attrs)*
        #vis #unsafety #auto_token #trait_token #ident #generics #colon_token #supertraits {
            #new_items
        }
    })
}
fn process_impl(item_impl: syn::ItemImpl) -> syn::Result<TokenStream> {
    let syn::ItemImpl {
        attrs,
        defaultness,
        unsafety,
        impl_token,
        generics,
        trait_,
        self_ty,
        brace_token: _,
        items,
    } = item_impl;

    let mut new_items = TokenStream::new();

    if trait_.is_some() {
        let impl_span = impl_token.span();
        let crate_idents = CrateIdents::new(impl_token.span());
        for item in items.into_iter() {
            match item {
                syn::ImplItem::Method(method) => {
                    if method.sig.asyncness.is_some() {
                        let ImplItemMethod {
                            attrs,
                            vis,
                            defaultness,
                            sig,
                            block: syn::Block { stmts, .. },
                        } = method;

                        let (sig, has_receiver) = convert_sig(sig, impl_span);
                        let fut = sig.associated_fut_impl(
                            if has_receiver {
                                TraitIndirection::None
                            } else {
                                TraitIndirection::Static
                            },
                            &crate_idents,
                        );
                        let trait_fn_sig = &sig.sig;

                        quote! {
                            #fut

                            #(#attrs)*
                            #vis #defaultness #trait_fn_sig {
                                async move { #(#stmts)* }
                            }
                        }
                        .to_tokens(&mut new_items);
                    } else {
                        method.to_tokens(&mut new_items);
                    }
                }
                item => {
                    item.to_tokens(&mut new_items);
                }
            }
        }
    } else {
        new_items = quote! { #(#items)* };
    }

    let trait_ = if let Some((bang, path, for_)) = trait_ {
        Some(quote! { #bang #path #for_ })
    } else {
        None
    };

    let where_clause = &generics.where_clause;

    Ok(quote! {
        #(#attrs)*
        #defaultness #unsafety #impl_token #generics #trait_ #self_ty #where_clause {
            #new_items
        }
    })
}

fn convert_sig(sig: syn::Signature, span: Span) -> (EntraitSignature, bool) {
    let has_receiver = match sig.inputs.first() {
        Some(syn::FnArg::Receiver(_)) => true,
        _ => false,
    };

    let mut entrait_sig = EntraitSignature::new(sig);
    entrait_sig.convert_to_associated_future(ReceiverGeneration::Rewrite, span);

    (entrait_sig, has_receiver)
}
