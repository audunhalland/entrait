trait Foo {
    fn foo(&self) -> u32;
}

trait Bar {
    fn bar(&self);
}

trait Baz {
    fn baz(&self) -> String;
}

macro_rules! entrait_mock_Foo {
    ($target:tt, [$($rest_macros:ident),*] $($traits:item)*) => {
        entrait::expand_mock!(
            $target,
            [$($rest_macros),*]
            $($traits)*
            trait Foo { fn foo(&self) -> u32; }
        );
    };
}

macro_rules! entrait_mock_Bar {
    ($target:tt, [$($rest_macros:ident),*] $($traits:item)*) => {
        entrait::expand_mock!(
            $target,
            [$($rest_macros),*]
            $($traits)*
            trait Bar { fn bar(&self); }
        );
    };
}

macro_rules! entrait_mock_Baz {
    ($target:tt, [$($rest_macros:ident),*] $($traits:item)*) => {
        entrait::expand_mock!(
            $target,
            [$($rest_macros),*]
            $($traits)*
            trait Baz { fn baz(&self) -> String; }
        );
    };
}

entrait::expand_mock!(C, [entrait_mock_Foo, entrait_mock_Bar, entrait_mock_Baz]);

#[test]
fn test_mockall() {
    let mut lol = MockC::new();
    lol.expect_foo().returning(|| 32);
}
