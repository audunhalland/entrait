#![allow(clippy::blacklisted_name)]

use entrait::*;

#[entrait(Foo, mockall)]
fn foo(_deps: &(), arg: i32) -> i32 {
    arg
}

fn takes_foo(foo: &impl Foo, arg: i32) -> i32 {
    foo.foo(arg)
}

#[test]
fn test() {
    let mut mock = MockFoo::new();
    mock.expect_foo().return_const(42);

    let result = takes_foo(&mock, 1337);

    assert_eq!(42, result);
}
