use entrait::entrait;

struct App(u32);

#[entrait(Foo for App, async_trait=true)]
async fn foo<A: Bar>(a: &A) -> u32 {
    a.bar().await
}

#[entrait(Bar for App, async_trait=true, mockable=true)]
async fn bar(app: &App) -> u32 {
    app.0
}

#[tokio::test]
async fn test() {
    let app = App(42);

    let result = app.foo().await;

    assert_eq!(42, result);
}

#[tokio::test]
async fn test_mock() {
    let mut bar = MockBar::new();
    bar.expect_bar().returning(|| 84);

    let result = foo(&bar).await;

    assert_eq!(84, result);
}
