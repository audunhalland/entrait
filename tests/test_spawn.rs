use entrait::*;
use unimock::*;

#[entrait(Spawning, async_trait = true)]
async fn spawning(deps: &(impl Bar + Clone + Send + Sync + 'static)) -> i32 {
    let handles = [deps.clone(), deps.clone()]
        .into_iter()
        .map(|deps| tokio::spawn(async move { deps.bar().await }));

    let mut result = 0;

    for handle in handles {
        result += handle.await.unwrap();
    }

    result
}

#[entrait(Bar, async_trait = true)]
async fn bar<T>(_: T) -> i32 {
    1
}

#[tokio::test]
async fn test_spawning_impl() {
    let result = spawning(&implementation::Impl::new(())).await;
    assert_eq!(2, result);
}

#[tokio::test]
async fn test_spawning_spy() {
    let result = spawning(&unimock::spy(None)).await;
    assert_eq!(2, result);
}

#[tokio::test]
async fn test_spawning_override_bar() {
    let result = spawning(&unimock::spy([
        bar::Fn.next_call(matching!()).returns(1).once().in_order(),
        bar::Fn.next_call(matching!()).returns(2).once().in_order(),
    ]))
    .await;
    assert_eq!(3, result);
}
