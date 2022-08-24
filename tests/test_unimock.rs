#![cfg(feature = "unimock")]
#![cfg_attr(feature = "use-associated-future", feature(generic_associated_types))]
#![cfg_attr(feature = "use-associated-future", feature(type_alias_impl_trait))]
#![allow(dead_code)]
#![allow(unused_variables)]

mod sync {
    use entrait::*;

    #[entrait(Foo, debug)]
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
            &mock(Some(AuthenticateMock.stub(|each| {
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
            FetchUserMock
                .each_call(matching!(42))
                .returns(Some(User {
                    username: "foobar".into(),
                    hash: "h4sh".into(),
                }))
                .in_any_order(),
            VerifyPasswordMock
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
                BarMock.each_call(matching!()).returns(40).in_any_order(),
                BazMock.each_call(matching!()).returns(2).in_any_order(),
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
                    BarMock.each_call(matching!()).returns(40).in_any_order(),
                    BazMock.each_call(matching!()).returns(2).in_any_order(),
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
                    BarMock.each_call(matching!()).returns(40).in_any_order(),
                    BazMock.each_call(matching!()).returns(2).in_any_order(),
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

    // important: takes T _by value_
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
            BarMock.next_call(matching!()).returns(1).once().in_order(),
            BarMock.next_call(matching!()).returns(2).once().in_order(),
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
            BarMock
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
    pub struct Borrowing<'a>(&'a i32);

    #[entrait(Borrow4)]
    async fn borrow4<'a>(_: &'a impl Bar, _arg: &i32) -> Borrowing<'a> {
        panic!()
    }

    #[tokio::test]
    async fn test_it() {
        let app = ::entrait::Impl::new(App);
        let _ = app.foo().await;
    }

    #[tokio::test]
    async fn mock_it() {
        let unimock = spy([BazMock.each_call(matching!()).returns(42).in_any_order()]);
        let answer = unimock.foo().await;

        assert_eq!(42, answer);
    }
}

mod generics {
    use entrait::*;
    use std::any::Any;
    use unimock::*;

    #[entrait(GenericDepsGenericReturn)]
    fn generic_deps_generic_return<T: Default>(_: &impl Any) -> T {
        Default::default()
    }

    #[entrait(ConcreteDepsGenericReturn)]
    fn concrete_deps_generic_return<T: Default>(_: &()) -> T {
        Default::default()
    }

    #[entrait(GenericDepsGenericParam)]
    fn generic_deps_generic_param<T>(_: &impl Any, _arg: T) -> i32 {
        42
    }

    #[entrait(ConcreteDepsGenericParam)]
    fn concrete_deps_generic_param<T>(_: &(), _arg: T) -> i32 {
        42
    }

    #[test]
    fn generic_mock_fns() {
        let _ = GenericDepsGenericReturnMock
            .with_types::<String>()
            .each_call(matching!())
            .returns("hey".to_string())
            .in_any_order();

        let _ = GenericDepsGenericParamMock
            .with_types::<String>()
            .each_call(matching!("hey"))
            .returns(42)
            .in_any_order();

        let _ = GenericDepsGenericParamMock
            .with_types::<i32>()
            .each_call(matching!(1337))
            .returns(42)
            .in_any_order();
    }

    #[entrait(ConcreteDepsGenericReturnWhere)]
    fn concrete_deps_generic_return_where<T>(_: &()) -> T
    where
        T: Default,
    {
        Default::default()
    }

    #[entrait(GenericDepsGenericReturnWhere)]
    fn generic_deps_generic_return_where<T>(_: &impl Any) -> T
    where
        T: Default,
    {
        Default::default()
    }
}

mod destructuring_params {
    use entrait::entrait;

    pub struct NewType<T>(T);

    #[entrait(Destructuring1)]
    fn destructuring1(_: &impl std::any::Any, NewType(num): NewType<i32>) {}

    #[entrait(Destructuring2)]
    fn destructuring2(
        _: &impl std::any::Any,
        NewType(num1): NewType<i32>,
        NewType(num2): NewType<String>,
    ) {
    }

    #[entrait(DestructuringNoDeps, no_deps)]
    fn destructuring_no_deps(NewType(num1): NewType<i32>, NewType(num2): NewType<i32>) {}

    // Should become (arg1, _arg1)
    #[entrait(DestructuringNoDepsMixedConflict, no_deps)]
    fn destructuring_no_deps_mixed_confict(arg1: NewType<i32>, NewType(num2): NewType<i32>) {}

    // Should become (arg1, _arg1)
    #[entrait(WildcardParams, no_deps)]
    fn wildcard_params(_: i32, _: i32) {}
}

mod entrait_for_trait_unimock {
    use entrait::*;
    use unimock::*;

    #[entrait]
    trait Trait {
        fn method1(&self) -> i32;
    }

    #[test]
    fn entraited_trait_should_have_unimock_impl() {
        assert_eq!(
            42,
            mock(Some(
                TraitMock::method1
                    .each_call(matching!())
                    .returns(42)
                    .in_any_order()
            ))
            .method1()
        );
    }

    #[test]
    #[should_panic(
        expected = "Trait::method1 cannot be unmocked as there is no function available to call."
    )]
    fn entraited_trait_should_not_be_spyable() {
        spy(None).method1();
    }
}

mod naming_conflict_between_fn_and_param {
    use entrait::*;

    #[entrait(Foo)]
    fn foo<T>(_: &T, foo: i32) {}
}

mod module {
    use entrait::*;
    use std::any::Any;
    use unimock::*;

    use crate::module::bar_baz::BarBazMock;

    #[entrait(pub Foo)]
    fn foo(_: &impl Any) -> i32 {
        7
    }

    #[entrait(pub BarBaz)]
    mod bar_baz {
        pub fn bar(deps: &impl super::Foo) -> i32 {
            deps.foo()
        }
    }

    fn takes_barbaz(deps: &impl BarBaz) -> i32 {
        deps.bar()
    }

    #[test]
    fn test_it() {
        let deps = mock(Some(
            BarBazMock::bar
                .each_call(matching!())
                .returns(42)
                .in_any_order(),
        ));
        assert_eq!(42, takes_barbaz(&deps));
    }
}

#[cfg(any(feature = "use-async-trait", feature = "use-associated-future"))]
mod module_async {
    use entrait::*;

    #[entrait(pub Mixed)]
    mod mixed {
        use std::any::Any;

        pub fn bar(_: &impl Any) {}
        pub async fn bar_async(_: &impl Any) {}
    }

    fn takes_mixed(deps: &impl Mixed) {
        let _ = deps.bar();
        let _ = deps.bar_async();
    }

    #[entrait(pub MultiAsync)]
    mod multi_async {
        use std::any::Any;

        pub async fn foo(_: &impl Any) {}
        pub async fn bar(_: &impl Any) {}
    }

    async fn takes_multi_async(deps: &impl MultiAsync) {
        deps.foo().await;
        deps.bar().await;
    }
}
