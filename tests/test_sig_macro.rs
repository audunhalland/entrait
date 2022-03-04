use entrait::print_consume_tokens;
use entrait::print_tokens;

macro_rules! fortytwo {
    () => {
        fortytwo_yes!()
    };
}

macro_rules! fortytwo_yes {
    () => {
        42
    };
}

#[test]
fn test_macro_in_macro() {
    let lol = print_tokens!(fortytwo!());
    println!("{lol}");
}

// Test macro

fn foo<A>(app: &A) {}

macro_rules! entrait_Foo {
    (expand_fn_sig) => {
        fn foo(&self) -> u32;
    };
    (expand_trait) => {
        trait Foo {
            entrait_Foo!(expand_fn_sig);
        }
    };
    (expand_mockall_impl, $struct_ty:tt) => {
        impl Foo for $struct_ty {
            entrait_foo!(expand_fn_sig);
        }
    };
    (expand_mock, $struct_ty:tt, $zelf:ident, [yo]) => {
        entrait_foo!(
            expand_mock,
            $struct_ty,
            impl Foo for $struct_ty {
                fn foo(&$zelf) -> u32;
            }
        );
    };
    (expand_mock, $struct_ty:tt, $impl_item:item) => {
        mockall::mock! {
            $struct_ty {}
            $impl_item
        }
    };
}

mod invoke {
    #[macro_export]
    macro_rules! stupid_macro {
        () => {
            struct Lol {}
        };
    }

    #[macro_export]
    macro_rules! invoke_macro {
        ($macro:ident) => {
            $macro!();
        };
    }
}

invoke_macro!(stupid_macro);

impl Lol {}

entrait_Foo!(expand_trait);

print_consume_tokens!(entrait_Foo!(expand_mockall_impl, C););

//entrait_Foo!(expand_mock, C, self, [yo]);

//entrait_foo!(mockall_impl, C);

mockall::mock! {
    C {}
    impl Foo for C {
        fn foo(&self) -> u32;
    }
}

macro_rules! test2 {
    ($t:expr) => {
        $t
    };
}

macro_rules! test3 {
    () => {
        42
    };
}

#[test]
fn test_mockall() {
    let mut lol = MockC::new();
    lol.expect_foo().returning(|| 32);

    let a: i32 = test2!(test3!());
}
