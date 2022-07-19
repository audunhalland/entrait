#![allow(clippy::blacklisted_name)]

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
