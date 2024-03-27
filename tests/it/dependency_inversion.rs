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

    pub struct MyImpl2;

    #[entrait]
    impl FoobarImpl for MyImpl2 {
        fn bar<D>(_: &D) -> u32 {
            1337
        }

        fn foo(deps: &impl super::Baz) -> i32 {
            deps.baz()
        }
    }

    impl DelegateFoobar<Self> for () {
        type Target = MyImpl2;
    }

    #[test]
    fn test_mod() {
        let app = Impl::new(());

        assert_eq!(42, app.foo());
    }

    impl DelegateFoobar<Self> for bool {
        type Target = MyImpl2;
    }

    #[test]
    fn test_impl_block() {
        let app = Impl::new(true);

        assert_eq!(42, app.foo());
        assert_eq!(1337, app.bar());
    }
}

mod simple_dyn {
    use entrait::*;

    #[entrait(FoobarImpl, delegate_by=ref)]
    trait Foobar {
        fn foo(&self) -> i32;
        fn bar(&self) -> u32;
    }

    struct Implementor2;

    #[entrait(ref)]
    impl FoobarImpl for Implementor2 {
        pub fn bar<D>(_: &D) -> u32 {
            1337
        }

        pub fn foo(deps: &impl super::Baz) -> i32 {
            deps.baz()
        }
    }

    struct App {
        foobar: Box<dyn FoobarImpl<Self> + Sync>,
    }

    impl AsRef<dyn FoobarImpl<Self>> for App {
        fn as_ref(&self) -> &dyn FoobarImpl<Self> {
            self.foobar.as_ref()
        }
    }

    #[test]
    fn test_impl_block() {
        let app = Impl::new(App {
            foobar: Box::new(Implementor2),
        });

        assert_eq!(42, app.foo());
        assert_eq!(1337, app.bar());
    }
}

mod async_static {
    use entrait::*;

    #[entrait(FoobarImpl, delegate_by = DelegateFoobar)]
    pub trait Foobar {
        async fn foo(&self) -> i32;
        async fn bar(&self) -> u32;
    }

    pub struct Implementor2;

    #[entrait]
    impl FoobarImpl for Implementor2 {
        pub async fn bar<D>(_: &D) -> u32 {
            1337
        }

        pub async fn foo(deps: &impl super::Baz) -> i32 {
            deps.baz()
        }
    }

    impl DelegateFoobar<Self> for bool {
        type Target = Implementor2;
    }

    #[tokio::test]
    async fn test_impl_block() {
        let app = Impl::new(true);

        assert_eq!(42, app.foo().await);
        assert_eq!(1337, app.bar().await);
    }
}

mod async_dyn {
    use entrait::*;

    #[entrait(FoobarImpl, delegate_by=ref)]
    #[async_trait::async_trait]
    pub trait Foobar {
        async fn foo(&self) -> i32;
        async fn bar(&self) -> u32;
    }

    pub struct Implementor2;

    #[entrait(ref)]
    #[async_trait::async_trait]
    impl FoobarImpl for Implementor2 {
        pub async fn bar<D>(_: &D) -> u32 {
            1337
        }

        pub async fn foo(deps: &impl super::Baz) -> i32 {
            deps.baz()
        }
    }

    struct App2(Implementor2);

    impl AsRef<dyn FoobarImpl<Self> + Sync> for App2 {
        fn as_ref(&self) -> &(dyn FoobarImpl<Self> + Sync) {
            &self.0
        }
    }

    #[tokio::test]
    async fn test_impl_block() {
        let app = Impl::new(App2(Implementor2));

        assert_eq!(42, app.foo().await);
        assert_eq!(1337, app.bar().await);
    }
}

mod issue_29 {
    use entrait::*;

    #[entrait(FoobarImpl, delegate_by = DelegateFoobar)]
    pub trait Foobar {
        fn foo<'a>(&self, input: &'a str) -> &'a str;
    }

    pub struct MyImpl;

    #[entrait]
    impl FoobarImpl for MyImpl {
        fn foo<'a, D>(deps: &D, input: &'a str) -> &'a str {
            input
        }
    }

    impl DelegateFoobar<Self> for () {
        type Target = MyImpl;
    }

    #[test]
    fn test_mod() {
        let app = Impl::new(());
        assert_eq!("foo", app.foo("foo"));
    }
}
