use entrait::*;

#[entrait]
mod foobar {
    pub fn foo() {}
    struct Foo {}
    pub fn bar() {}
    mod hei {}
    pub fn baz() {}
    const _: () = {

    };
    pub fn qux() {}
}

fn test() {
    foobar::foo();
    foobar::bar();
    foobar::baz();
    foobar::qux();
}
