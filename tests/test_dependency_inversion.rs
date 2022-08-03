mod simple_static {
    use entrait::*;

    #[entrait(delegate_by = DelegateFoo)]
    trait Foo {
        fn foo(&self) -> i32;
    }

    /*
    impl DelegateFoo<Self> for () {
        type By = Provider;
    }

    struct Provider;

    fn test() {
        let app = Impl::new(());

        app.foo();
    }
    */
}
