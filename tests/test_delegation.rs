#![feature(generic_associated_types)]

use entrait::unimock::*;
use implementation::*;

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

    let num = state.borrow_impl().foo().await;
    assert_eq!(42, num);
}
