#[entrait::entrait(pub Baz)]
fn baz<D>(_: &D) -> i32 {
    42
}

mod simple_static {
    use entrait::*;

    #[entrait(delegate_by = DelegateFoobar)]
    trait Foobar {
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

        #[derive_impl(super::Foobar)]
        pub struct FoobarImpl;
    }

    impl DelegateFoobar<Self> for () {
        type By = foobar_impl::FoobarImpl;
    }

    #[test]
    fn test() {
        let app = Impl::new(());

        assert_eq!(42, app.foo());
    }
}

mod simple_dyn {
    use entrait::*;

    #[entrait(delegate_by = dyn DelegateFoobar)]
    trait Foobar {
        fn foo(&self) -> i32;
        fn bar(&self) -> u32;
    }

    #[entrait_dyn_impl]
    mod foobar_impl {
        pub fn bar<D>(_: &D) -> u32 {
            0
        }

        pub fn foo<D>(_: &D) -> i32 {
            1337
        }

        #[derive_impl(super::DelegateFoobar)]
        pub struct FoobarImpl;
    }

    struct App {
        foobar: Box<dyn DelegateFoobar<Self> + Sync>,
    }

    impl std::borrow::Borrow<dyn DelegateFoobar<Self>> for App {
        fn borrow(&self) -> &dyn DelegateFoobar<Self> {
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
