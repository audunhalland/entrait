use super::{
    EntraitLifetime, EntraitSignature, ReceiverGeneration, SigComponent, UserProvidedLifetime,
};

use std::collections::HashSet;
use syn::visit_mut::VisitMut;

pub fn de_elide_lifetimes(
    entrait_sig: &mut EntraitSignature,
    receiver_generation: ReceiverGeneration,
) {
    let mut elision_detector = ElisionDetector::new(receiver_generation);
    elision_detector.detect(&mut entrait_sig.sig);

    let mut visitor = LifetimeMutVisitor::new(true, elision_detector.elided_params);

    process(entrait_sig, &mut visitor, receiver_generation);
}

pub fn collect_lifetimes(
    entrait_sig: &mut EntraitSignature,
    receiver_generation: ReceiverGeneration,
) {
    let mut visitor = LifetimeMutVisitor::new(false, HashSet::new());
    process(entrait_sig, &mut visitor, receiver_generation);
}

fn process(
    entrait_sig: &mut EntraitSignature,
    visitor: &mut LifetimeMutVisitor,
    receiver_generation: ReceiverGeneration,
) {
    match receiver_generation {
        ReceiverGeneration::None => {
            for (index, arg) in entrait_sig.sig.inputs.iter_mut().enumerate() {
                visitor.de_elide_param(index, arg);
            }
        }
        ReceiverGeneration::Rewrite | ReceiverGeneration::Insert => {
            visitor.de_elide_receiver(entrait_sig.sig.inputs.first_mut().unwrap());

            for (index, arg) in entrait_sig.sig.inputs.iter_mut().skip(1).enumerate() {
                visitor.de_elide_param(index, arg);
            }
        }
    }

    visitor.de_elide_output(&mut entrait_sig.sig.output);

    entrait_sig.lifetimes.append(&mut visitor.lifetimes);
}

/// Looks at elided lifetimes and makes them explicit.
/// Also collects all lifetimes into `lifetimes`.
struct LifetimeMutVisitor {
    de_elide: bool,
    current_component: SigComponent,
    elided_params: HashSet<usize>,
    lifetimes: Vec<EntraitLifetime>,
}

impl LifetimeMutVisitor {
    fn new(de_elide: bool, elided_params: HashSet<usize>) -> Self {
        Self {
            de_elide,
            current_component: SigComponent::Receiver,
            elided_params,
            lifetimes: vec![],
        }
    }

    fn de_elide_receiver(&mut self, arg: &mut syn::FnArg) {
        self.current_component = SigComponent::Receiver;
        self.visit_fn_arg_mut(arg);
    }

    fn de_elide_param(&mut self, index: usize, arg: &mut syn::FnArg) {
        self.current_component = SigComponent::Param(index);
        self.visit_fn_arg_mut(arg);
    }

    fn de_elide_output(&mut self, output: &mut syn::ReturnType) {
        self.current_component = SigComponent::Output;
        self.visit_return_type_mut(output);
    }

    fn process_opt_lifetime(&mut self, lifetime: Option<syn::Lifetime>) -> Option<syn::Lifetime> {
        if self.de_elide {
            Some(self.make_lifetime_explicit(lifetime))
        } else if let Some(lifetime) = lifetime {
            Some(self.register_user_lifetime(lifetime))
        } else {
            None
        }
    }

    fn make_lifetime_explicit(&mut self, lifetime: Option<syn::Lifetime>) -> syn::Lifetime {
        match self.current_component {
            SigComponent::Receiver | SigComponent::Param(_) => match lifetime {
                Some(lifetime) => self.register_user_lifetime(lifetime),
                None => self.register_new_entrait_lifetime(),
            },
            // Do not register user-provided output lifetimes, should already be registered from inputs:
            SigComponent::Output => lifetime
                // If lifetime was elided, try to find it:
                .or_else(|| self.find_output_lifetime())
                // If not, there must be some kind of compile error somewhere else
                .unwrap_or_else(|| self.broken_lifetime()),
        }
    }

    fn find_output_lifetime(&self) -> Option<syn::Lifetime> {
        let from_component = match self.only_elided_input() {
            // If only one input was elided, use that input:
            Some(elided_input) => SigComponent::Param(elided_input),
            // If not, use the receiver lifetime:
            None => SigComponent::Receiver,
        };

        self.lifetimes
            .iter()
            .find(|lt| lt.source == from_component)
            .map(|lt| lt.lifetime.clone())
    }

    fn only_elided_input(&self) -> Option<usize> {
        if self.elided_params.len() == 1 {
            self.elided_params.iter().next().copied()
        } else {
            None
        }
    }

    fn register_user_lifetime(&mut self, lifetime: syn::Lifetime) -> syn::Lifetime {
        self.register_lifetime(EntraitLifetime {
            lifetime,
            source: self.current_component,
            user_provided: UserProvidedLifetime(true),
        })
    }

    fn register_new_entrait_lifetime(&mut self) -> syn::Lifetime {
        let index = self.lifetimes.len();
        self.register_lifetime(EntraitLifetime {
            lifetime: syn::Lifetime::new(
                &format!("'entrait{}", index),
                proc_macro2::Span::call_site(),
            ),
            source: self.current_component,
            user_provided: UserProvidedLifetime(false),
        })
    }

    fn register_lifetime(&mut self, entrait_lifetime: EntraitLifetime) -> syn::Lifetime {
        let lifetime = entrait_lifetime.lifetime.clone();
        self.lifetimes.push(entrait_lifetime);
        lifetime
    }

    fn broken_lifetime(&self) -> syn::Lifetime {
        syn::Lifetime::new("'entrait_broken", proc_macro2::Span::call_site())
    }
}

impl syn::visit_mut::VisitMut for LifetimeMutVisitor {
    fn visit_receiver_mut(&mut self, receiver: &mut syn::Receiver) {
        if let Some((_, lifetime)) = &mut receiver.reference {
            *lifetime = self.process_opt_lifetime(lifetime.clone());
        }
        syn::visit_mut::visit_receiver_mut(self, receiver);
    }

    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {
        reference.lifetime = self.process_opt_lifetime(reference.lifetime.clone());
        syn::visit_mut::visit_type_reference_mut(self, reference);
    }

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_" {
            *lifetime = self.make_lifetime_explicit(Some(lifetime.clone()));
        }
    }
}

struct ElisionDetector {
    receiver_generation: ReceiverGeneration,
    current_input: usize,
    elided_params: HashSet<usize>,
}

impl ElisionDetector {
    fn new(receiver_generation: ReceiverGeneration) -> Self {
        Self {
            receiver_generation,
            current_input: 0,
            elided_params: Default::default(),
        }
    }

    fn detect(&mut self, sig: &mut syn::Signature) {
        for (index, input) in sig.inputs.iter_mut().enumerate() {
            match self.receiver_generation {
                ReceiverGeneration::None => {
                    self.current_input = index;
                    self.visit_fn_arg_mut(input);
                }
                _ => {
                    if index > 1 {
                        self.current_input = index - 1;
                        self.visit_fn_arg_mut(input);
                    }
                }
            }
        }
    }
}

impl syn::visit_mut::VisitMut for ElisionDetector {
    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {
        if reference.lifetime.is_none() {
            self.elided_params.insert(self.current_input);
        }
        syn::visit_mut::visit_type_reference_mut(self, reference);
    }

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_" {
            self.elided_params.insert(self.current_input);
        }
    }
}
