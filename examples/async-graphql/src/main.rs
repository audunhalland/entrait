mod db {
    use entrait::unimock_test::*;

    #[entrait(pub FetchSomeValue, no_deps, async_trait)]
    async fn fetch_some_value() -> String {
        "real".to_string()
    }
}

mod graphql {
    use super::db;
    use std::marker::PhantomData;

    pub struct Query<A>(PhantomData<A>);

    #[async_graphql::Object]
    impl<A> Query<A>
    where
        A: db::FetchSomeValue + Send + Sync + 'static,
    {
        async fn some_value(&self, ctx: &async_graphql::Context<'_>) -> Result<String, String> {
            let app = ctx.data_unchecked::<A>();
            Ok(app.fetch_some_value().await)
        }
    }

    #[tokio::test]
    async fn unit_test_query() {
        use async_graphql::*;
        use unimock::*;

        let deps = mock(Some(
            db::fetch_some_value::Fn
                .each_call(matching!())
                .returns("mocked".to_string())
                .in_any_order(),
        ));

        let response = async_graphql::Schema::build(
            Query::<Unimock>(PhantomData),
            EmptyMutation,
            EmptySubscription,
        )
        .data(deps.clone())
        .finish()
        .execute("{ someValue }")
        .await;

        assert_eq!(
            response.data,
            value!({
                "someValue": "mocked"
            })
        );
    }

    #[tokio::test]
    async fn integration_test_query() {
        use async_graphql::*;
        use implementation::Impl;

        let app = Impl::new(());
        let response = async_graphql::Schema::build(
            Query::<Impl<()>>(PhantomData),
            EmptyMutation,
            EmptySubscription,
        )
        .data(app)
        .finish()
        .execute("{ someValue }")
        .await;

        assert_eq!(
            response.data,
            value!({
                "someValue": "real"
            })
        );
    }
}

fn main() {}