mod simple_static {
    use entrait::*;

    #[entrait(delegate_by = DelegateFoo)]
    trait Foo {
        fn foo(&self) -> i32;
    }

    #[entrait(Bar)]
    fn bar<D>(deps: &D) -> i32 {
        42
    }

    pub struct MyImpl;

    #[entrait_impl]
    mod my_impl {
        #[derive_impl(super::Foo)]
        pub struct MyImpl;

        pub fn foo(deps: &impl super::Bar) -> i32 {
            42
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

mod scopes {
    /*
    mod lol {
        pub trait Foo {}

        pub mod bar {
            pub trait Foo {}
        }
    }

    use lol::bar;

    mod hei {
        fn harry() -> std::string::String {}

        impl super::bar::Foo for () {}
    }
    */
}
