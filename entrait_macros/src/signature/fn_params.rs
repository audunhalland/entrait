use std::collections::HashSet;
use syn::visit_mut::VisitMut;

#[derive(Copy, Clone, Eq, PartialEq)]
enum ParamStatus {
    Ok,
    NeedsFix,
}

impl ParamStatus {
    fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    fn combine(self, other: ParamStatus) -> Self {
        match (self, other) {
            (Self::Ok, Self::Ok) => Self::Ok,
            _ => Self::NeedsFix,
        }
    }
}

pub fn fix_fn_param_idents(sig: &mut syn::Signature) {
    if fix_ident_conflicts(sig).is_ok() {
        return;
    }

    if lift_inner_pat_idents(sig).is_ok() {
        return;
    }

    autogenerate_for_non_idents(sig);
}

fn fix_ident_conflicts(sig: &mut syn::Signature) -> ParamStatus {
    let mut status = ParamStatus::Ok;
    let fn_ident_string = sig.ident.to_string();

    for fn_arg in sig.inputs.iter_mut() {
        let arg_status = match fn_arg {
            syn::FnArg::Receiver(_) => ParamStatus::Ok,
            syn::FnArg::Typed(pat_type) => match pat_type.pat.as_mut() {
                syn::Pat::Ident(param_ident) => {
                    if param_ident.ident == fn_ident_string {
                        param_ident.ident = syn::Ident::new(
                            &format!("{}_", param_ident.ident),
                            param_ident.ident.span(),
                        );
                    }

                    ParamStatus::Ok
                }
                _ => ParamStatus::NeedsFix,
            },
        };

        status = status.combine(arg_status);
    }

    status
}

fn lift_inner_pat_idents(sig: &mut syn::Signature) -> ParamStatus {
    fn try_lift_unambiguous_inner(pat: &mut syn::Pat) -> ParamStatus {
        struct PatIdentSearcher {
            first_binding_pat_ident: Option<syn::Ident>,
            binding_pat_count: usize,
        }

        impl syn::visit_mut::VisitMut for PatIdentSearcher {
            fn visit_pat_ident_mut(&mut self, i: &mut syn::PatIdent) {
                let ident_string = i.ident.to_string();

                match ident_string.chars().next() {
                    Some(char) if char.is_lowercase() => {
                        self.binding_pat_count += 1;
                        if self.first_binding_pat_ident.is_none() {
                            self.first_binding_pat_ident = Some(i.ident.clone());
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut searcher = PatIdentSearcher {
            first_binding_pat_ident: None,
            binding_pat_count: 0,
        };

        searcher.visit_pat_mut(pat);

        if searcher.binding_pat_count == 1 {
            let ident = searcher.first_binding_pat_ident;
            *pat = syn::parse_quote! { #ident };

            ParamStatus::Ok
        } else {
            ParamStatus::NeedsFix
        }
    }

    let mut status = ParamStatus::Ok;

    for fn_arg in &mut sig.inputs {
        let param_status = match fn_arg {
            syn::FnArg::Receiver(_) => ParamStatus::Ok,
            syn::FnArg::Typed(pat_type) => match pat_type.pat.as_mut() {
                syn::Pat::Ident(_) => ParamStatus::Ok,
                pat => try_lift_unambiguous_inner(pat),
            },
        };

        status = status.combine(param_status);
    }

    status
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
        fix_fn_param_idents(&mut source);
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
