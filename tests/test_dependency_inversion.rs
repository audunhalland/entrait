#![cfg_attr(feature = "use-associated-future", feature(generic_associated_types))]
#![cfg_attr(feature = "use-associated-future", feature(type_alias_impl_trait))]

#[entrait::entrait(pub Baz)]
fn baz<D>(_: &D) -> i32 {
    42
}

mod simple_static {
    use entrait::*;

    #[entrait(FoobarImpl, delegate_by = DelegateFoobar)]
    pub trait Foobar {
        fn foo(&self) -> i32;
        fn bar(&self) -> u32;
    }

    #[entrait_impl]
    mod foobar_impl {
        pub fn bar<D>(_: &D) -> u32 {
            0
        }

        pub fn foo(deps: &impl super::super::Baz) -> i32 {
            deps.baz()
        }

        #[derive_impl(super::FoobarImpl)]
        pub struct MyImpl;
    }

    impl DelegateFoobar<Self> for () {
        type Target = foobar_impl::MyImpl;
    }

    #[test]
    fn test() {
        let app = Impl::new(());

        assert_eq!(42, app.foo());
    }
}

mod simple_dyn {
    use entrait::*;

    #[entrait(FoobarImpl, delegate_by = Borrow)]
    trait Foobar {
        fn foo(&self) -> i32;
        fn bar(&self) -> u32;
    }

    #[entrait_dyn_impl]
    mod foobar_impl {
        pub fn bar<D>(_: &D) -> u32 {
            0
        }

        pub fn foo(deps: &impl super::super::Baz) -> i32 {
            deps.baz()
        }

        #[derive_impl(super::FoobarImpl)]
        pub struct FoobarImpl;
    }

    struct App {
        foobar: Box<dyn FoobarImpl<Self> + Sync>,
    }

    impl std::borrow::Borrow<dyn FoobarImpl<Self>> for App {
        fn borrow(&self) -> &dyn FoobarImpl<Self> {
            self.foobar.as_ref()
        }
    }

    #[test]
    fn test() {
        let app = Impl::new(App {
            foobar: Box::new(foobar_impl::FoobarImpl),
        });

        assert_eq!(1337, app.foo());
    }
}

#[cfg(any(feature = "use-async-trait", feature = "use-associated-future"))]
mod async_static {
    use entrait::*;

    #[entrait(FoobarImpl, delegate_by = DelegateFoobar)]
    pub trait Foobar {
        async fn foo(&self) -> i32;
        async fn bar(&self) -> u32;
    }

    #[entrait_impl]
    mod foobar_impl {
        pub async fn bar<D>(_: &D) -> u32 {
            0
        }

        pub async fn foo(deps: &impl super::super::Baz) -> i32 {
            deps.baz()
        }

        #[derive_impl(super::FoobarImpl)]
        pub struct MyImpl;
    }

    impl DelegateFoobar<Self> for () {
        type Target = foobar_impl::MyImpl;
    }

    #[tokio::test]
    async fn test() {
        let app = Impl::new(());

        assert_eq!(42, app.foo().await);
    }
}
