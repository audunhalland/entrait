#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use entrait::*;

struct App;

#[entrait(Foo, gat_future)]
async fn foo(deps: &impl Bar) -> i32 {
    deps.bar().await
}

#[entrait(Bar, gat_future)]
async fn bar(deps: &impl Baz) -> i32 {
    deps.baz().await
}

#[entrait(Baz, gat_future)]
async fn baz(_: &App) -> i32 {
    42
}

#[entrait(NoDeps, gat_future, no_deps)]
async fn no_deps(arg: i32) -> i32 {
    arg
}

#[tokio::test]
async fn test_it() {
    let app = ::implementation::Impl::new(App);
    let _ = app.foo().await;
}
