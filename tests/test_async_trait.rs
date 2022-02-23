#[cfg(feature = "async_trait")]
mod test_async {
    use entrait::entrait;

    struct App(u32);

    #[entrait(Foo for App)]
    async fn foo<A: Bar>(a: &A) -> u32 {
        a.bar().await
    }

    #[entrait(Bar for App)]
    async fn bar(app: &App) -> u32 {
        app.0
    }

    #[tokio::test]
    async fn test() {
        let app = App(42);

        let result = app.foo().await;

        assert_eq!(42, result);
    }
}
