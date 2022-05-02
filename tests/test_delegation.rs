#![feature(generic_associated_types)]

use entrait::unimock::*;
use implementation::*;
use unimock::*;

struct InnerAppState {
    num: i32,
}

struct OuterAppState {
    inner: InnerAppState,
}

#[entrait(Foo, async_trait = true)]
async fn foo(deps: &impl Bar) -> i32 {
    deps.bar().await
}

#[entrait(Bar, async_trait = true)]
async fn bar(state: &OuterAppState) -> i32 {
    state.inner.borrow_impl().baz().await
}

#[entrait(Baz, async_trait = true)]
async fn baz(deps: &impl Qux) -> i32 {
    deps.qux().await
}

#[entrait(Qux, async_trait = true)]
async fn qux(state: &InnerAppState) -> i32 {
    state.num
}

#[tokio::test]
async fn test_impl() {
    let state = OuterAppState {
        inner: InnerAppState { num: 42 },
    };

    assert_eq!(42, state.borrow_impl().foo().await);
}

#[tokio::test]
#[should_panic(expected = "Bar::bar cannot be unmocked as there is no function available to call.")]
async fn test_spy_does_not_work_with_concrete_impl() {
    foo(&spy(None)).await;
}

#[tokio::test]
async fn test_spy_with_fallback() {
    assert_eq!(
        1337,
        foo(&spy([bar::Fn::next_call(matching!(_))
            .returns(1337)
            .once()
            .in_order()]))
        .await
    );
}
