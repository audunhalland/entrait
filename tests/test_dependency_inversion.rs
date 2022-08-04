mod simple_static {
    use entrait::*;

    #[entrait(delegate_by = DelegateFoo)]
    trait Foo {
        fn foo(&self) -> i32;
    }

    #[entrait(pub Bar)]
    fn bar<D>(_: &D) -> i32 {
        42
    }

    #[entrait_impl]
    mod my_impl {
        #[derive_impl(super::Foo)]
        pub struct MyImpl;

        pub fn foo(deps: &impl super::Bar) -> i32 {
            deps.bar()
        }
    }

    impl DelegateFoo<Self> for () {
        type By = my_impl::MyImpl;
    }

    #[test]
    fn test() {
        let app = Impl::new(());

        assert_eq!(42, app.foo());
    }
}
