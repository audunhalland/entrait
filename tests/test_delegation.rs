use entrait::unimock::*;
use implementation::*;
use unimock::*;

// Upstream crate:
struct InnerAppState {
    num: i32,
}

// Downstream crate:
// TODO:
// #[derive(entrait::Project)]
struct OuterAppState {
    // #[project(ProjectInnerAppState: Baz, Baz2, Baz3, etc)]
    inner: Impl<InnerAppState>,
}

// Might create a derive macro for this:
trait ProjectInnerAppState {
    type Inner: Baz + Send + Sync;

    fn project_inner(&self) -> &Self::Inner;
}

impl ProjectInnerAppState for implementation::Impl<OuterAppState> {
    type Inner = implementation::Impl<InnerAppState>;

    fn project_inner(&self) -> &Self::Inner {
        &self.inner
    }
}

impl ProjectInnerAppState for Unimock {
    type Inner = Unimock;

    fn project_inner(&self) -> &Self::Inner {
        self
    }
}

#[entrait(Foo, async_trait = true)]
async fn foo(deps: &impl Bar) -> i32 {
    deps.bar().await
}

#[entrait(Bar, async_trait = true)]
async fn bar(deps: &impl ProjectInnerAppState) -> i32 {
    deps.project_inner().baz().await
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
    let state = Impl::new(OuterAppState {
        inner: Impl::new(InnerAppState { num: 42 }),
    });

    assert_eq!(42, state.foo().await);
}

#[tokio::test]
#[should_panic(expected = "Qux::qux cannot be unmocked as there is no function available to call.")]
async fn test_spy_does_not_work_with_concrete_impl() {
    foo(&spy(None)).await;
}

#[tokio::test]
async fn test_spy_with_fallback_for_qux() {
    assert_eq!(
        1337,
        foo(&spy([qux::Fn::next_call(matching!(_))
            .returns(1337)
            .once()
            .in_order()]))
        .await
    );
}
