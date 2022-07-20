#![allow(clippy::blacklisted_name)]

mod basic {
    use entrait::*;

    #[entrait(MockallFoo, mockall)]
    fn mockall_foo(_deps: &(), arg: i32) -> i32 {
        arg
    }

    fn takes_foo(foo: &impl MockallFoo, arg: i32) -> i32 {
        foo.mockall_foo(arg)
    }

    #[test]
    fn test() {
        let mut mock = MockMockallFoo::new();
        mock.expect_mockall_foo().return_const(42);

        let result = takes_foo(&mock, 1337);

        assert_eq!(42, result);
    }
}

mod delegate_impl {
    use entrait::*;

    #[delegate_impl(mockall)]
    trait Delegated {
        fn delegate(&self) -> i32;
    }

    #[test]
    fn test() {
        let mut mock = MockDelegated::new();
        mock.expect_delegate().return_const(42);

        assert_eq!(42, mock.delegate());
    }
}
