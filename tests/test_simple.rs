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

mod no_deps_and_feign {
    use entrait::entrait;

    use feignhttp::get;

    #[entrait(NoDeps, no_deps)]
    fn no_deps(_a: i32, _b: i32) {}

    #[entrait(CallMyApi, no_deps, async_trait)]
    #[get("https://my.api.org/api/{param}")]
    async fn call_my_api(#[path] param: String) -> feignhttp::Result<String> {}
}
