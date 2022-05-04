#![feature(generic_associated_types)]

use entrait::entrait;
use implementation::*;
use unimock::*;

struct State(u32);

#[entrait(Foo, async_trait = true)]
async fn foo<A: Bar>(a: &A) -> u32 {
    a.bar().await
}

#[entrait(Bar, async_trait = true, unimock = true)]
async fn bar(state: &State) -> u32 {
    state.0
}

#[tokio::test]
async fn test() {
    let state = Impl::new(State(42));
    let result = state.foo().await;

    assert_eq!(42, result);
}

#[tokio::test]
async fn test_mock() {
    let result = foo(&mock(Some(
        bar::Fn::each_call(matching!())
            .returns(84_u32)
            .in_any_order(),
    )))
    .await;

    assert_eq!(84, result);
}

#[tokio::test]
async fn test_impl() {
    let state = Impl::new(State(42));
    assert_eq!(42, state.foo().await);
}
