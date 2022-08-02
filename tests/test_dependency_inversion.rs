mod simple_static {
    use entrait::*;

    #[entrait(delegate_by = DelegateFoo)]
    trait Foo {
        fn foo(&self) -> i32;
    }
}
