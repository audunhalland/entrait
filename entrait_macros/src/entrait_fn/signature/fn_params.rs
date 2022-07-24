use std::collections::HashSet;

pub fn convert_params_to_ident(sig: &mut syn::Signature) {
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

struct IdentSearcher<'ast> {
    idents: HashSet<&'ast syn::Ident>,
}

impl<'ast> syn::visit::Visit<'ast> for IdentSearcher<'ast> {
    fn visit_ident(&mut self, i: &'ast proc_macro2::Ident) {
        self.idents.insert(i);
    }
}
