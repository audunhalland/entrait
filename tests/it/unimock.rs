#![allow(dead_code)]
#![allow(unused_variables)]

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

#[cfg(any(feature = "use-boxed-futures", feature = "use-associated-futures"))]
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

    #[entrait(Authenticate, mock_api=AuthenticateMock)]
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

    #[entrait(FetchUser, mock_api=FetchUserMock)]
    fn fetch_user<T>(_: &T, _id: u32) -> Option<User> {
        Some(User {
            username: "name".into(),
            hash: "h4sh".into(),
        })
    }

    #[entrait(VerifyPassword, mock_api=VerifyPasswordMock)]
    fn verify_password<T>(_: &T, _password: &str, _hash: &str) -> bool {
        true
    }

    #[tokio::test]
    async fn test_get_username() {
        let username = get_username(
            &Unimock::new(AuthenticateMock.stub(|each| {
                each.call(matching!(_, _)).returns(Ok(User {
                    username: "foobar".into(),
                    hash: "h4sh".into(),
                }));
            })),
            42,
            "pw",
        )
        .await
        .unwrap();
        assert_eq!("foobar", username);
    }

    #[tokio::test]
    async fn test_authenticate() {
        let mocks = Unimock::new((
            FetchUserMock.each_call(matching!(42)).returns(Some(User {
                username: "foobar".into(),
                hash: "h4sh".into(),
            })),
            VerifyPasswordMock
                .each_call(matching!("pw", "h4sh"))
                .returns(true)
                .once(),
        ));

        let user = authenticate(&mocks, 42, "pw").await.unwrap();
        assert_eq!("foobar", user.username);
    }

    #[tokio::test]
    async fn test_partial_no_overrides() {
        let user = authenticate(&Unimock::new_partial(()), 42, "pw")
            .await
            .unwrap();

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

#[cfg(any(feature = "use-boxed-futures", feature = "use-associated-futures"))]
mod multi_mock {
    use entrait::*;
    use unimock::*;

    #[entrait(Bar, mock_api = BarMock)]
    async fn bar<A>(_: &A) -> i32 {
        unimplemented!()
    }

    #[entrait(Baz, mock_api = BazMock)]
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
            let mock = Unimock::new((
                BarMock.each_call(matching!()).returns(40),
                BazMock.each_call(matching!()).returns(2),
            ));

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
                sum(&Unimock::new((
                    BarMock.each_call(matching!()).returns(40),
                    BazMock.each_call(matching!()).returns(2),
                )))
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
                sum(&Unimock::new((
                    BarMock.each_call(matching!()).returns(40),
                    BazMock.each_call(matching!()).returns(2),
                )))
                .await
            );
        }
    }
}

#[cfg(any(feature = "use-boxed-futures", feature = "use-associated-futures"))]
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
    #[entrait(Bar, mock_api = BarMock)]
    async fn bar<T>(_: T) -> i32 {
        1
    }

    #[tokio::test]
    async fn test_spawning_impl() {
        let result = spawning(&implementation::Impl::new(())).await;
        assert_eq!(2, result);
    }

    #[tokio::test]
    async fn test_spawning_partial_unmocked() {
        let result = spawning(&Unimock::new_partial(())).await;
        assert_eq!(2, result);
    }

    #[tokio::test]
    async fn test_spawning_override_bar() {
        let result = spawning(&Unimock::new_partial((
            BarMock.next_call(matching!()).returns(1).once(),
            BarMock.next_call(matching!()).returns(2).once(),
        )))
        .await;
        assert_eq!(3, result);
    }
}

#[cfg(any(feature = "use-boxed-futures", feature = "use-associated-futures"))]
mod more_async {
    use entrait::*;
    use unimock::*;
    struct State(u32);

    #[entrait(Foo)]
    async fn foo<A: Bar>(a: &A) -> u32 {
        a.bar().await
    }

    #[entrait(Bar, mock_api = BarMock)]
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
        let result = foo(&Unimock::new(
            BarMock.each_call(matching!()).returns(84_u32),
        ))
        .await;

        assert_eq!(84, result);
    }

    #[tokio::test]
    async fn test_impl() {
        let state = Impl::new(State(42));
        assert_eq!(42, state.foo().await);
    }
}

#[cfg(any(feature = "use-boxed-futures", feature = "use-associated-futures"))]
mod async_no_deps_etc {
    use entrait::*;
    use unimock::*;

    struct App;

    #[entrait(Foo, mock_api = FooMock)]
    async fn foo(deps: &impl Bar) -> i32 {
        deps.bar().await
    }

    #[entrait(Bar, mock_api = BarMock)]
    async fn bar(deps: &impl Baz) -> i32 {
        deps.baz().await
    }

    #[entrait(Baz, mock_api = BazMock)]
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
        let unimock = Unimock::new_partial(BazMock.each_call(matching!()).returns(42));
        let answer = unimock.foo().await;

        assert_eq!(42, answer);
    }
}

mod generics {
    use entrait::*;
    use std::any::Any;
    use unimock::*;

    #[entrait(GenericDepsGenericReturn, mock_api = Mock1)]
    fn generic_deps_generic_return<T: Default>(_: &impl Any) -> T {
        Default::default()
    }

    #[entrait(ConcreteDepsGenericReturn, mock_api = Mock2)]
    fn concrete_deps_generic_return<T: Default>(_: &()) -> T {
        Default::default()
    }

    #[entrait(GenericDepsGenericParam, mock_api = Mock3)]
    fn generic_deps_generic_param<T>(_: &impl Any, _arg: T) -> i32 {
        42
    }

    #[entrait(ConcreteDepsGenericParam, mock_api = Mock4)]
    fn concrete_deps_generic_param<T>(_: &(), _arg: T) -> i32 {
        42
    }

    #[test]
    fn generic_mock_fns() {
        let s: String = Unimock::new(
            Mock1
                .with_types::<String>()
                .some_call(matching!())
                .returns("hey".to_string()),
        )
        .generic_deps_generic_return();

        assert_eq!("hey".to_string(), s);

        assert_eq!(
            42,
            Unimock::new(
                Mock3
                    .with_types::<String>()
                    .some_call(matching!("hey"))
                    .returns(42),
            )
            .generic_deps_generic_param(format!("hey"))
        );

        assert_eq!(
            42,
            Unimock::new(
                Mock3
                    .with_types::<i32>()
                    .some_call(matching!(1337))
                    .returns(42),
            )
            .generic_deps_generic_param(1337),
        );
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

    #[entrait(mock_api=TraitMock)]
    trait Trait {
        fn method1(&self) -> i32;
    }

    #[test]
    fn entraited_trait_should_have_unimock_impl() {
        assert_eq!(
            42,
            Unimock::new(TraitMock::method1.each_call(matching!()).returns(42)).method1()
        );
    }

    #[test]
    #[should_panic(
        expected = "Trait::method1 cannot be unmocked as there is no function available to call."
    )]
    fn entraited_trait_should_not_be_unmockable() {
        Unimock::new_partial(()).method1();
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

    #[entrait(pub Foo, mock_api=FooMock)]
    fn foo(_: &impl Any) -> i32 {
        7
    }

    #[entrait(pub BarBaz, mock_api=BarBazMock)]
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
        let deps = Unimock::new(bar_baz::BarBazMock::bar.each_call(matching!()).returns(42));
        assert_eq!(42, takes_barbaz(&deps));
    }
}

#[cfg(any(feature = "use-boxed-futures", feature = "use-associated-futures"))]
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

#[test]
fn level_without_mock_support() {
    use entrait::*;
    use unimock::*;

    #[entrait(A)]
    fn a(deps: &(impl B + C)) {
        deps.b();
        deps.c();
    }

    #[entrait(B, mock_api=BMock)]
    fn b(deps: &impl std::any::Any) {}

    #[entrait(CImpl, delegate_by = DelegateC)]
    pub trait C {
        fn c(&self);
    }

    #[entrait(pub D)]
    mod d {}

    fn takes_a(a: &impl A) {}
    fn takes_b(b: &impl B) {}
    fn takes_c(b: &impl C) {}

    takes_a(&Unimock::new(()));
    takes_b(&Unimock::new(()));
    takes_c(&Unimock::new(()));
}
