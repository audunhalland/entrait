use entrait::entrait;

struct App(u32);

#[entrait(Foo for App, use async_trait + mockall)]
async fn foo<A: Bar>(a: &A) -> u32 {
    a.bar().await
}

#[entrait(#[async_trait] Bar for App)]
async fn bar(app: &App) -> u32 {
    app.0
}

/*


Foo for App; [async_trait, mockall]

#[async_trait] #[mockall] Foo for App

Foo for App, use async_trait+mockall

Foo for App macro(async_trait, mockall)
Foo for App, use macro(async_trait, mockall)

(Foo for App, async=async_trait, mockall=)
*/

#[tokio::test]
async fn test() {
    let app = App(42);

    let result = app.foo().await;

    assert_eq!(42, result);
}
