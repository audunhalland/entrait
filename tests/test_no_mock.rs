use entrait::entrait;

/// This should be a UI test that only runs when no mocking is present
#[entrait(Foo)]
fn foo(_deps: &()) {}

#[test]
fn test() {}
