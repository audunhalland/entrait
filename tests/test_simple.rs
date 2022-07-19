#![allow(unused_variables)]
#![cfg_attr(feature = "use-associated-future", feature(generic_associated_types))]
#![cfg_attr(feature = "use-associated-future", feature(type_alias_impl_trait))]

mod bounds {
    use entrait::*;

    mod app {
        pub struct State {
            pub number: u32,
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

mod test_delegate_impl {
    use entrait::delegate_impl;

    #[delegate_impl]
    trait Plain {
        fn method0(&self, arg: i32) -> i32;
    }

    #[delegate_impl]
    trait Generic1<T> {
        fn generic_return(&self, arg: i32) -> T;
        fn generic_param(&self, arg: T) -> i32;
    }
}
