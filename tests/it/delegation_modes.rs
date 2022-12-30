fn assert_is_send<T: Send>(_: &T) {}
fn assert_is_sync<T: Sync>(_: &T) {}

mod borrow_dyn_sync {
    use super::*;
    use entrait::*;

    #[entrait(Foo)]
    fn foo(deps: &impl Bar) {
        deps.bar();
    }

    #[entrait(delegate_by=ref)]
    trait Bar: 'static {
        fn bar(&self);
    }

    struct App(Box<dyn Bar + Sync>);

    impl AsRef<dyn Bar> for App {
        fn as_ref(&self) -> &dyn Bar {
            self.0.as_ref()
        }
    }

    struct Baz;

    impl Bar for Baz {
        fn bar(&self) {}
    }

    #[test]
    fn test_impl_borrow() {
        let app = Impl::new(App(Box::new(Baz)));

        assert_is_sync(&app);

        app.foo();
    }
}

#[cfg(feature = "boxed-futures")]
mod borrow_dyn_use_boxed_futures {
    use super::*;
    use async_trait::*;
    use entrait::*;

    #[entrait(Foo, box_future)]
    async fn foo(deps: &impl Bar) {
        deps.bar().await;
    }

    #[entrait(delegate_by=ref, box_future)]
    #[async_trait]
    trait Bar: Sync + 'static {
        async fn bar(&self);
    }

    struct Baz;

    struct App(Baz);

    impl AsRef<dyn Bar> for App {
        fn as_ref(&self) -> &dyn Bar {
            &self.0
        }
    }

    #[async_trait]
    impl Bar for Baz {
        async fn bar(&self) {}
    }

    #[tokio::test]
    async fn test_async_borrow() {
        let app = Impl::new(App(Baz));

        assert_is_send(&app);
        assert_is_sync(&app);

        app.foo().await;
    }
}
