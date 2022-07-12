#![cfg(feature = "nightly-tests")]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use entrait::*;
use unimock::*;

struct App;

#[entrait(Foo, associated_future)]
async fn foo(deps: &impl Bar) -> i32 {
    deps.bar().await
}

#[entrait(Bar, associated_future)]
async fn bar(deps: &impl Baz) -> i32 {
    deps.baz().await
}

#[entrait(Baz, associated_future)]
async fn baz(_: &App) -> i32 {
    42
}

#[entrait(NoDeps, associated_future, no_deps)]
async fn no_deps(arg: i32) -> i32 {
    arg
}

#[tokio::test]
async fn test_it() {
    let app = ::entrait::Impl::new(App);
    let _ = app.foo().await;
}

#[tokio::test]
async fn mock_it() {
    let unimock = spy([baz::Fn.each_call(matching!()).returns(42).in_any_order()]);
    let answer = unimock.foo().await;

    assert_eq!(42, answer);
}
