use entrait::entrait;

mod app {
    pub struct App {
        pub number: u32,
    }
}

mod where_bounds {
    use super::*;

    #[entrait(pub Foo for app::App)]
    fn foo<A>(app: &A) -> u32
    where
        A: Bar + Baz,
    {
        println!("Foo");
        app.bar();
        app.baz("from foo")
    }
}

mod impl_bounds {
    use super::*;

    #[entrait(pub Foo for app::App)]
    fn foo(app: &(impl Bar + Baz)) -> u32 {
        println!("Foo");
        app.bar();
        app.baz("from foo")
    }
}

#[entrait(Bar for app::App)]
fn bar<A>(app: &A)
where
    A: Baz,
{
    println!("Bar");
    app.baz("from bar");
}

#[entrait(Baz for app::App)]
fn baz(app: &app::App, from_where: &str) -> u32 {
    println!("Baz {from_where}");
    app.number
}

#[test]
fn test_where_bounds() {
    use where_bounds::Foo;
    let app = app::App { number: 42 };
    let result = app.foo();
    assert_eq!(42, result);
}

#[test]
fn test_impl_bounds() {
    use impl_bounds::Foo;
    let app = app::App { number: 42 };
    let result = app.foo();
    assert_eq!(42, result);
}
