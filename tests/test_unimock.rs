#![cfg(feature = "unimock")]
#![cfg_attr(feature = "use-associated-future", feature(generic_associated_types))]
#![cfg_attr(feature = "use-associated-future", feature(type_alias_impl_trait))]

mod sync {
    use entrait::*;

    #[entrait(Foo)]
    fn foo(deps: &impl Bar) -> String {
        deps.bar()
    }

    #[entrait(Bar)]
    fn bar(_: &()) -> String {
        "string".to_string()
    }
}

#[cfg(any(feature = "use-async-trait", feature = "use-associated-future"))]
mod auth {
    use entrait::*;
    use unimock::*;

    type Error = ();

    #[derive(Clone)]
    pub struct User {
        username: String,
        hash: String,
    }

    #[entrait(GetUsername)]
    async fn get_username(
        rt: &impl Authenticate,
        id: u32,
        password: &str,
    ) -> Result<String, Error> {
        let user = rt.authenticate(id, password).await?;
        Ok(user.username)
    }

    #[entrait(Authenticate)]
    async fn authenticate(
        deps: &(impl FetchUser + VerifyPassword),
        id: u32,
        password: &str,
    ) -> Result<User, Error> {
        let user = deps.fetch_user(id).ok_or(())?;
        if deps.verify_password(password, &user.hash) {
            Ok(user)
        } else {
            Err(())
        }
    }

    #[entrait(FetchUser)]
    fn fetch_user<T>(_: &T, _id: u32) -> Option<User> {
        Some(User {
            username: "name".into(),
            hash: "h4sh".into(),
        })
    }

    #[entrait(VerifyPassword)]
    fn verify_password<T>(_: &T, _password: &str, _hash: &str) -> bool {
        true
    }

    #[tokio::test]
    async fn test_get_username() {
        let username = get_username(
            &mock(Some(authenticate::Fn.stub(|each| {
                each.call(matching!(_, _)).returns(Ok(User {
                    username: "foobar".into(),
                    hash: "h4sh".into(),
                }));
            }))),
            42,
            "pw",
        )
        .await
        .unwrap();
        assert_eq!("foobar", username);
    }

    #[tokio::test]
    async fn test_authenticate() {
        let mocks = mock([
            fetch_user::Fn
                .each_call(matching!(42))
                .returns(Some(User {
                    username: "foobar".into(),
                    hash: "h4sh".into(),
                }))
                .in_any_order(),
            verify_password::Fn
                .each_call(matching!("pw", "h4sh"))
                .returns(true)
                .once()
                .in_any_order(),
        ]);

        let user = authenticate(&mocks, 42, "pw").await.unwrap();
        assert_eq!("foobar", user.username);
    }

    #[tokio::test]
    async fn test_full_spy() {
        let user = authenticate(&spy(None), 42, "pw").await.unwrap();

        assert_eq!("name", user.username);
    }

    #[tokio::test]
    async fn test_impl() {
        assert_eq!(
            "name",
            Impl::new(()).get_username(42, "password").await.unwrap()
        );
    }
}

#[cfg(any(feature = "use-async-trait", feature = "use-associated-future"))]
mod multi_mock {
    use entrait::*;
    use unimock::*;

    #[entrait(Bar)]
    async fn bar<A>(_: &A) -> i32 {
        unimplemented!()
    }

    #[entrait(Baz)]
    async fn baz<A>(_: &A) -> i32 {
        unimplemented!()
    }

    mod inline_bounds {
        use super::*;
        use entrait::entrait;

        #[entrait(Sum)]
        async fn sum<A: Bar + Baz>(a: &A) -> i32 {
            a.bar().await + a.baz().await
        }

        #[tokio::test]
        async fn test_mock() {
            let mock = mock([
                bar::Fn.each_call(matching!()).returns(40).in_any_order(),
                baz::Fn.each_call(matching!()).returns(2).in_any_order(),
            ]);

            let result = sum(&mock).await;

            assert_eq!(42, result);
        }
    }

    mod where_bounds {
        use super::*;

        #[entrait(Sum)]
        async fn sum<A>(a: &A) -> i32
        where
            A: Bar + Baz,
        {
            a.bar().await + a.baz().await
        }

        #[tokio::test]
        async fn test_mock() {
            assert_eq!(
                42,
                sum(&mock([
                    bar::Fn.each_call(matching!()).returns(40).in_any_order(),
                    baz::Fn.each_call(matching!()).returns(2).in_any_order(),
                ]))
                .await
            );
        }
    }

    mod impl_trait_bounds {
        use super::*;

        #[entrait(Sum)]
        async fn sum(a: &(impl Bar + Baz)) -> i32 {
            a.bar().await + a.baz().await
        }

        #[tokio::test]
        async fn test_mock() {
            assert_eq!(
                42,
                sum(&mock([
                    bar::Fn.each_call(matching!()).returns(40).in_any_order(),
                    baz::Fn.each_call(matching!()).returns(2).in_any_order(),
                ]))
                .await
            );
        }
    }
}

#[cfg(any(feature = "use-async-trait", feature = "use-associated-future"))]
mod tokio_spawn {
    use entrait::*;
    use unimock::*;

    #[entrait(Spawning)]
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

    #[entrait(Bar)]
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
}

#[cfg(any(feature = "use-async-trait", feature = "use-associated-future"))]
mod more_async {
    use entrait::*;
    use unimock::*;
    struct State(u32);

    #[entrait(Foo)]
    async fn foo<A: Bar>(a: &A) -> u32 {
        a.bar().await
    }

    #[entrait(Bar)]
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
            bar::Fn
                .each_call(matching!())
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
}

#[cfg(any(feature = "use-async-trait", feature = "use-associated-future"))]
mod async_no_deps_etc {
    use entrait::*;
    use unimock::*;

    struct App;

    #[entrait(Foo)]
    async fn foo(deps: &impl Bar) -> i32 {
        deps.bar().await
    }

    #[entrait(Bar)]
    async fn bar(deps: &impl Baz) -> i32 {
        deps.baz().await
    }

    #[entrait(Baz)]
    async fn baz(_: &App) -> i32 {
        42
    }

    #[entrait(NoDeps, no_deps)]
    async fn no_deps(arg: i32) -> i32 {
        arg
    }

    #[entrait(Borrow1)]
    async fn borrow1(_: &impl Bar) -> &i32 {
        panic!()
    }

    #[entrait(Borrow2)]
    async fn borrow2<'a, 'b>(_: &'a impl Bar, _arg: &'b i32) -> &'a i32 {
        panic!()
    }

    #[entrait(Borrow3)]
    async fn borrow3<'a>(_: &impl Bar, arg: &'a i32) -> &'a i32 {
        arg
    }

    #[allow(unused)]
    struct Borrowing<'a>(&'a i32);

    // BUG: Does not work in unimock, needs GAT design:
    // ```
    // type Output = Borrowing<'a>;
    // ```
    // Maybe it will never work for lifetimes other than self.
    /*
    #[entrait(Borrow4)]
    async fn borrow4<'a>(_: &'a impl Bar, _arg: &i32) -> Borrowing<'a> {
        panic!()
    }
    */

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
}
