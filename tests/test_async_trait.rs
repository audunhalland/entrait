#![feature(generic_associated_types)]

use entrait::entrait;
use unimock::*;

struct App(u32);

#[entrait(Foo for App, async_trait=true)]
async fn foo<A: Bar>(a: &A) -> u32 {
    a.bar().await
}

#[entrait(Bar for App, async_trait=true, unimock=true, unmock=false)]
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
    let result = foo(&mock(bar::Fn, |each| {
        each.call(matching!()).returns(84_u32);
    }))
    .await;

    assert_eq!(84, result);
}
