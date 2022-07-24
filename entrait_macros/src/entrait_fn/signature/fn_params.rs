use std::collections::HashSet;
use syn::visit::Visit;

pub fn convert_params_to_ident(sig: &mut syn::Signature) {
    if !sig.inputs.iter().any(needs_param_ident) {
        return;
    }

    lift_inner_pat_idents(sig);
    autogenerate_for_non_idents(sig);
}

fn needs_param_ident(fn_arg: &syn::FnArg) -> bool {
    match fn_arg {
        syn::FnArg::Receiver(_) => false,
        syn::FnArg::Typed(pat_type) => !matches!(pat_type.pat.as_ref(), syn::Pat::Ident(_)),
    }
}

fn lift_inner_pat_idents(sig: &mut syn::Signature) {
    fn try_lift_unambiguous_inner(pat: &syn::Pat) -> Option<&syn::Ident> {
        struct PatIdentSearcher<'ast> {
            binding_pat_idents: Vec<&'ast syn::Ident>,
        }

        impl<'ast> syn::visit::Visit<'ast> for PatIdentSearcher<'ast> {
            fn visit_pat_ident(&mut self, i: &'ast syn::PatIdent) {
                let ident_string = i.ident.to_string();

                match ident_string.chars().next() {
                    Some(char) if char.is_lowercase() => {
                        self.binding_pat_idents.push(&i.ident);
                    }
                    _ => {}
                }
            }
        }

        let mut searcher = PatIdentSearcher {
            binding_pat_idents: vec![],
        };

        searcher.visit_pat(pat);

        if searcher.binding_pat_idents.len() == 1 {
            searcher.binding_pat_idents.into_iter().next()
        } else {
            None
        }
    }

    for fn_arg in &mut sig.inputs {
        match fn_arg {
            syn::FnArg::Receiver(_) => {}
            syn::FnArg::Typed(pat_type) => match pat_type.pat.as_mut() {
                syn::Pat::Ident(_) => {}
                pat => match try_lift_unambiguous_inner(pat) {
                    Some(ident) => {
                        *pat_type.pat = syn::parse_quote! { #ident };
                    }
                    None => {}
                },
            },
        }
    }
}

fn autogenerate_for_non_idents(sig: &mut syn::Signature) {
    let mut taken_idents: HashSet<String> = sig
        .inputs
        .iter()
        .filter_map(|fn_arg| match fn_arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
                _ => None,
            },
        })
        .collect();

    fn generate_ident(index: usize, attempts: usize, taken_idents: &mut HashSet<String>) -> String {
        let ident = format!(
            "{}arg{}",
            (0..attempts).map(|_| '_').collect::<String>(),
            index,
        );

        if taken_idents.contains(&ident) {
            generate_ident(index, attempts + 1, taken_idents)
        } else {
            taken_idents.insert(ident.clone());
            ident
        }
    }

    let pat_type_args = sig.inputs.iter_mut().filter_map(|fn_arg| match fn_arg {
        syn::FnArg::Typed(pat_type) => Some(pat_type),
        _ => None,
    });

    for (index, pat_type_arg) in pat_type_args.enumerate() {
        match pat_type_arg.pat.as_mut() {
            syn::Pat::Ident(_) => {}
            _ => {
                let new_ident_string = generate_ident(index, 0, &mut taken_idents);
                let new_ident = quote::format_ident!("{}", new_ident_string);
                *pat_type_arg.pat = syn::parse_quote! { #new_ident };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::ToTokens;

    use super::*;

    fn convert_expect(mut source: syn::Signature, expected: syn::Signature) {
        convert_params_to_ident(&mut source);
        assert_eq!(
            source.to_token_stream().to_string(),
            expected.to_token_stream().to_string(),
        );
    }

    #[test]
    fn should_not_generate_conflicts() {
        convert_expect(
            syn::parse_quote! {
                fn foo(arg1: T, _: T, T(arg3): T, _: T)
            },
            syn::parse_quote! {
                fn foo(arg1: T, _arg1: T, arg3: T, _arg3: T)
            },
        );

        convert_expect(
            syn::parse_quote! {
                fn foo(_: T, T(arg0): T)
            },
            syn::parse_quote! {
                fn foo(_arg0: T, arg0: T)
            },
        );
    }

    #[test]
    fn should_extract_only_unambiguous_pat_idents() {
        convert_expect(
            syn::parse_quote! {
                fn f(
                    ident0: T,
                    T(ident1): T,
                    T(T(ident2)): T,
                    T(ident3, _): T,
                    T(ident4, None): T,
                    T(None): T,
                    T(foo, bar): T,
                    T(foo, T(bar)): T,
                )
            },
            syn::parse_quote! {
                fn f(
                    ident0: T,
                    ident1: T,
                    ident2: T,
                    ident3: T,
                    ident4: T,
                    arg5: T,
                    arg6: T,
                    arg7: T,
                )
            },
        );
    }
}
