#![allow(clippy::disallowed_names)]

use entrait::*;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Foo {
    value: String,
}

/// "Business logic"
mod business {
    use super::*;

    #[entrait(pub GetFoo, no_deps, mock_api=GetFooMock)]
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

        let deps = Unimock::new(business::GetFooMock.each_call(matching!()).returns(Foo {
            value: "mocked".to_string(),
        }));
        let router = Routes::<Unimock>::router().layer(Extension(deps.clone()));
        let response = router
            .oneshot(
                Request::get("/foo")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap();
        let foo: Foo = serde_json::from_slice(&bytes).unwrap();

        assert_eq!("mocked", foo.value);
    }
}

#[tokio::main]
async fn main() {
    use axum::extract::Extension;
    use entrait::Impl;

    #[derive(Clone)]
    struct App;

    let app = Impl::new(App);
    let router = rest::Routes::<Impl<App>>::router().layer(Extension(app));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    axum::serve(listener, router).await.unwrap();
}
