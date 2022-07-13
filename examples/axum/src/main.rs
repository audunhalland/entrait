use entrait::*;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Foo {
    value: String,
}

/// "Business logic"
mod business {
    use super::*;

    #[entrait(pub GetFoo, no_deps, async_trait)]
    async fn get_foo() -> Foo {
        Foo {
            value: "real".to_string(),
        }
    }
}

/// Axum specific
mod rest {
    use super::*;
    use axum::extract::Extension;
    use axum::routing::get;
    use axum::Json;

    pub struct Routes<A>(std::marker::PhantomData<A>);

    impl<A> Routes<A>
    where
        A: business::GetFoo + Send + Sync + Sized + Clone + 'static,
    {
        pub fn router() -> axum::Router {
            axum::Router::new().route("/foo", get(Self::get_foo))
        }

        async fn get_foo(Extension(app): Extension<A>) -> Json<Foo> {
            Json(app.get_foo().await)
        }
    }

    #[tokio::test]
    async fn unit_test_router() {
        use axum::http::Request;
        use tower::ServiceExt;
        use unimock::*;

        let deps = mock(Some(
            business::get_foo::Fn
                .each_call(matching!())
                .returns(Foo {
                    value: "mocked".to_string(),
                })
                .in_any_order(),
        ));
        let router = Routes::<Unimock>::router().layer(Extension(deps.clone()));
        let response = router
            .oneshot(
                Request::get("/foo")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let foo: Foo = serde_json::from_slice(&bytes).unwrap();

        assert_eq!("mocked", foo.value);
    }
}

#[tokio::main]
async fn main() {
    use axum::extract::Extension;
    use entrait::Impl;
    use std::net::SocketAddr;

    #[derive(Clone)]
    struct App;

    let app = Impl::new(App);
    let router = rest::Routes::<Impl<App>>::router().layer(Extension(app));

    axum::Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000)))
        .serve(router.into_make_service())
        .await
        .unwrap();
}
