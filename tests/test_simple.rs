#![allow(dead_code)]
#![allow(unused)]
#![cfg_attr(feature = "use-associated-future", feature(generic_associated_types))]
#![cfg_attr(feature = "use-associated-future", feature(type_alias_impl_trait))]

mod bounds {
    use entrait::*;

    mod app {
        pub struct State {
            pub number: u32,
        }
    }

    mod inline_bounds {
        use super::*;

        #[entrait(pub Foo)]
        fn foo<A: Bar + Baz>(app: &A) -> u32 {
            println!("Foo");
            app.bar();
            app.baz("from foo")
        }
    }

    mod where_bounds {
        use super::*;

        #[entrait(pub Foo)]
        fn foo<A>(app: &A) -> u32
        where
            A: Bar + Baz,
        {
            println!("Foo");
            app.bar();
            app.baz("from foo")
        }
    }

    mod impl_bounds {
        use super::*;

        #[entrait(pub Foo)]
        fn foo(deps: &(impl Bar + Baz)) -> u32 {
            println!("Foo");
            deps.bar();
            deps.baz("from foo")
        }
    }

    #[entrait(Bar)]
    fn bar<A>(deps: &A)
    where
        A: Baz,
    {
        println!("Bar");
        deps.baz("from bar");
    }

    #[entrait(Baz)]
    fn baz(app: &app::State, from_where: &str) -> u32 {
        println!("Baz {from_where}");
        app.number
    }

    #[test]
    fn test_where_bounds() {
        use where_bounds::Foo;
        let impl_state = Impl::new(app::State { number: 42 });
        let result = impl_state.foo();
        assert_eq!(42, result);
    }

    #[test]
    fn test_impl_bounds() {
        use impl_bounds::Foo;
        let impl_state = Impl::new(app::State { number: 42 });
        let result = impl_state.foo();
        assert_eq!(42, result);
    }

    mod mixed_inline_bounds {
        use super::*;

        #[entrait(pub Foo)]
        fn foo<D, E>(deps: &D, arg: &E) {}
    }
}

#[cfg(any(feature = "use-async-trait", feature = "use-associated-future"))]
mod no_deps_and_feign {
    use entrait::entrait;

    use feignhttp::get;

    #[entrait(NoDeps, no_deps)]
    fn no_deps(_a: i32, _b: i32) {}

    #[entrait(CallMyApi, no_deps)]
    #[get("https://my.api.org/api/{param}")]
    async fn call_my_api(#[path] param: String) -> feignhttp::Result<String> {}
}

mod test_tracing_instrument {
    use entrait::entrait;
    use tracing::instrument;

    #[entrait(IWantToDebug1)]
    #[instrument(skip(deps))]
    fn i_want_to_debug1(deps: &impl OtherFunc) {
        deps.other_func(1337);
    }

    #[instrument(skip(deps))]
    #[entrait(IWantToDebug2)]
    fn i_want_to_debug2(deps: &impl OtherFunc) {
        deps.other_func(1337);
    }

    #[entrait(OtherFunc, no_deps)]
    fn other_func(_some_arg: i32) {}
}

mod test_entrait_for_trait {
    use entrait::*;

    #[entrait]
    trait Plain {
        fn method0(&self, arg: i32) -> i32;
    }

    #[entrait]
    trait Generic1<T> {
        fn generic_return(&self, arg: i32) -> T;
        fn generic_param(&self, arg: T) -> i32;
    }

    impl Plain for () {
        fn method0(&self, arg: i32) -> i32 {
            1337
        }
    }

    impl Plain for &'static str {
        fn method0(&self, arg: i32) -> i32 {
            42
        }
    }

    #[test]
    fn entraited_trait_should_have_impl_impl() {
        assert_eq!(1337, Impl::new(()).method0(0));
        assert_eq!(42, Impl::new("app").method0(0));
    }
}

mod module {
    use entrait::*;
    use std::any::Any;

    #[entrait(pub Dep1)]
    fn dep1(_: &impl Any) {}

    #[entrait(pub Dep2)]
    fn dep2(_: &impl Any) {}

    #[entrait(pub EmptyModule)]
    mod empty_module {}

    #[entrait(pub FooBarBazQux)]
    mod foo_bar_baz_qux {
        use super::Dep1;

        pub fn foo(deps: &impl Dep1, arg: i32) {}

        struct Foo {}

        pub fn bar(deps: &impl super::Dep2, arg: &str) {}

        mod hei {}

        pub(super) fn baz(deps: &impl Dep1) {}

        const _: () = {};

        pub(crate) fn qux(deps: &impl super::Dep2) {
            not_included();
        }

        static S: &'static str = "";

        fn not_included() {}
    }

    // There should be an automatic `pub use foo_bar_baz_qux::FooBarBazQux`;
    fn takes_foo_bar_baz_qux(deps: &impl FooBarBazQux) {
        deps.foo(42);
        deps.bar("");
        deps.baz();
        deps.qux();
    }

    fn test() {
        let app = Impl::new(());
        takes_foo_bar_baz_qux(&app);
    }

    #[entrait(PrivateTrait)]
    mod private_trait {}
}
