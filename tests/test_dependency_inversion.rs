mod simple_static {
    use entrait::*;

    #[entrait(delegate_by = DelegateFoo)]
    trait Foo {
        fn foo(&self) -> i32;
    }

    pub struct MyImpl;

    #[entrait_impl(Foo for MyImpl)]
    mod my_impl {
        pub fn foo<D>(deps: &D) -> i32 {
            42
        }
    }

    impl DelegateFoo<Self> for () {
        type By = MyImpl;
    }

    fn test() {
        let app = Impl::new(());

        // app.foo();
    }
}
