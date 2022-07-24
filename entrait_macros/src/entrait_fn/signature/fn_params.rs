use std::collections::HashSet;

struct IdentSearcher<'ast> {
    idents: HashSet<&'ast syn::Ident>,
}

impl<'ast> syn::visit::Visit<'ast> for IdentSearcher<'ast> {
    fn visit_ident(&mut self, i: &'ast proc_macro2::Ident) {
        self.idents.insert(i);
    }
}
