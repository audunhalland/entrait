mod simple_static {
    use entrait::*;

    #[entrait(delegate_by = DelegateFoobar)]
    trait Foobar {
        fn foo(&self) -> i32;
        fn bar(&self) -> u32;
    }

    #[entrait(pub Baz)]
    fn baz<D>(_: &D) -> i32 {
        42
    }

    #[entrait_impl]
    mod foobar_impl {
        pub fn bar<D>(_: &D) -> u32 {
            0
        }

        pub fn foo(deps: &impl super::Baz) -> i32 {
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
