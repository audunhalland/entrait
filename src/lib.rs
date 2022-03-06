//! Experimental proc macro to ease development using _Inversion of Control_ patterns in Rust.
//!
//! `entrait` is used to generate a trait from the definition of a regular function.
//! The main use case for this is that other functions may depend upon the trait
//! instead of the concrete implementation, enabling better test isolation.
//!
//! The macro looks like this:
//!
//! ```rust
//! # use entrait::*;
//! #[entrait(MyFunction)]
//! fn my_function<D>(deps: &D) {
//! }
//! ```
//!
//! which generates the trait `MyFunction`:
//!
//! ```rust
//! trait MyFunction {
//!     fn my_function(&self);
//! }
//! ```
//!
//! `my_function`'s first and only parameter is `deps` which is generic over some unknown type `D`.
//! This would correspond to the `self` parameter in the trait.
//! But what is this type supposed to be? We can generate an implementation in the same go, using `for Type`:
//!
//! ```rust
//! struct App;
//!
//! #[entrait::entrait(MyFunction for App)]
//! fn my_function<D>(deps: &D) {
//! }
//!
//! // Generated:
//! // trait MyFunction {
//! //     fn my_function(&self);
//! // }
//! //
//! // impl MyFunction for App {
//! //     fn my_function(&self) {
//! //         my_function(self)
//! //     }
//! // }
//!
//! fn main() {
//!     let app = App;
//!     app.my_function();
//! }
//! ```
//!
//! The advantage of this pattern comes into play when a function declares its dependencies, as _trait bounds_:
//!
//!
//! ```rust
//! # use entrait::*;
//! # struct App;
//! #[entrait(Foo for App)]
//! fn foo(deps: &(impl Bar))
//! {
//!     deps.bar();
//! }
//!
//! #[entrait(Bar for App)]
//! fn bar<D>(deps: &D) {
//! }
//! ```
//!
//! The functions may take any number of parameters, but the first one is always considered specially as the "dependency parameter".
//!
//! Functions may also be non-generic, depending directly on the App:
//!
//! ```rust
//! # use entrait::*;
//! # struct App { some_thing: SomeType };
//! # type SomeType = u32;
//! #[entrait(ExtractSomething for App)]
//! fn extract_something(app: &App) -> SomeType {
//!     app.some_thing
//! }
//! ```
//!
//! These kinds of functions may be considered "leaves" of a dependency tree.
//!
//! ## `async` support
//! Since Rust at the time of writing does not natively support async methods in traits, you may opt in to having `#[async_trait]` generated
//! for your trait:
//!
//! ```rust
//! # use entrait::*;
//! #[entrait(Foo, async_trait=true)]
//! async fn foo<D>(deps: &D) {
//! }
//! ```
//! This is designed to be forwards compatible with real async fn in traits. When that day comes, you should be able to just remove the `async_trait=true`
//! to get a proper zero-cost future.
//!
//! ## `mockall` support
//! The macro supports autogenerating `mockall` mock structs:
//!
//! ```rust
//! # use entrait::*;
//! #[entrait(Foo, mockable=true)]
//! fn foo<D>(_: &D) -> u32 {
//!     unimplemented!()
//! }
//!
//! fn my_func(deps: &(impl Foo)) -> u32 {
//!     deps.foo()
//! }
//!
//! #[test]
//! fn test_my_func() {
//!     let deps = MockFoo();
//!     deps.expect_foo().returning(|| 42);
//!     assert_eq!(42, my_func(&deps));
//! }
//! ```
//! This is easy enough when there is only one trait bound, because the generated trait need only be attributed with `mockall::automock`.
//!
//! With multiple trait bounds, this becomes a little harder: We need some concrete struct that implement all the given traits:
//!
//! ```rust
//! # use entrait::*;
//! #[entrait(Foo, mockable=true)]
//! fn foo<D>(_: &D) -> u32 {
//!     unimplemented!()
//! }
//! #[entrait(Bar, mockable=true)]
//! fn bar<D>(_: &D) -> u32 {
//!     unimplemented!()
//! }
//!
//! #[entrait(MyFunc, mock_deps_as=FooPlusBar)]
//! fn my_func(deps: &(impl Foo + Bar)) -> u32 {
//!     deps.foo() + deps.bar()
//! }
//!
//! #[test]
//! fn test_my_func() {
//!     let deps = MockFooPlusBar();
//!     deps.expect_foo().returning(|| 40);
//!     deps.expect_bar().returning(|| 2);
//!     assert_eq!(42, my_func(&deps));
//! }
//! ```
//!
//! This works, by entraiting `my_func` and passing `mock_deps_as=FooPlusBar`. However, this requires some rather hairy macro magic behind the scenes:
//! In order to generate `MockFooPlusBar`, `mockall` needs access to the trait definitions and method signatures of both `Foo` and `Bar`. These
//! traits are not definied locally inside the `my_func` item, they are _standalone items_ defined elsewhere.
//!
//! The magic works by having each `mockable=true` entraitment define its own `macro_rules` where one can get this trait definition derived from the trait name.
//! For `$[entrait(Foo, mockable=true)` the macro generated is:
//!
//! ```text
//! macro_rules! entrait_mock_Foo { ... }
//! ```
//!
//! `entrait` invokes this macro to expand `Foo`'s trait definition,
//! at the end passing all this information into `mockall::mock`.
//!
//! At the time of writing, procedural macros outputting new `macro_rules` do not play very well with `rust-analyzer`. So you may want to type out the mock
//! generator yourself instead.

#![forbid(unsafe_code)]

pub use entrait_macros::entrait;
pub use entrait_macros::generate_mock;

#[doc(hidden)]
#[macro_export]
macro_rules! expand_mock {
    ($target:tt, [] $($traits:item)*) => {
        entrait::generate_mock!($target $($traits)*);
    };
    ($target:tt, [$macro:ident $(,$rest_macros:ident)*] $($traits:item)*) => {
        $macro!($target, [$($rest_macros),*] $($traits)*);
    };
}
