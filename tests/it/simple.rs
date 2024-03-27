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

    #[entrait]
    trait Generic2<T> {
        fn generic_return<U: 'static>(&self, arg: i32) -> (T, U);
        fn generic_params<U: 'static>(&self, arg0: T, arg1: U) -> i32;
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

        static S: &str = "";

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

    // This test is behind this flag because
    // we cannot have private/crate-private types in interfaces
    // implemented by external crates
    #[cfg(not(feature = "unimock"))]
    mod crate_private {
        use entrait::*;

        pub(crate) struct PrivateType;

        #[entrait(pub(crate) PubCrateTrait)]
        mod pub_crate_trait {
            pub(crate) fn foo<D>(_: &D) -> super::PrivateType {
                super::PrivateType
            }
        }
    }

    // Note: pub(super) things will never work well, probably.
    // The macro cannot just append a another `::super`, because `pub(super::super)` is invalid syntax.
}

mod cfg_attributes {
    use entrait::*;

    #[entrait(mock_api = TraitMock)]
    trait Trait {
        fn compiled(&self);

        #[cfg(feature = "always-disabled")]
        fn not_compiled(&self) -> NonExistentType;
    }

    impl Trait for () {
        fn compiled(&self) {}
    }

    #[test]
    fn call_compiled() {
        let app = Impl::new(());
        app.compiled();
    }
}

mod future_send_opt_out {
    use std::rc::Rc;

    use entrait::*;

    #[entrait(Spawning, ?Send)]
    async fn spawning(deps: &(impl Bar + Clone + Send + Sync + 'static)) -> Rc<i32> {
        let deps = deps.clone();

        tokio::task::spawn_local(async move { deps.bar().await })
            .await
            .unwrap()
    }

    #[entrait(Bar, ?Send, mock_api = BarMock)]
    async fn bar<T>(_: T) -> Rc<i32> {
        Rc::new(42)
    }
}
