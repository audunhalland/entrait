//! # entrait
//!
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
//! fn my_function<A>(a: &A) {
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
//! `my_function`'s first and only parameter is `a` which is generic over some unknown type `A`. This would correspond to the `self` parameter in the trait. But what is this type supposed to be? We can generate an implementation in the same go, using `for Type`:
//!
//! ```rust
//! struct App;
//!
//! #[entrait::entrait(MyFunction for App)]
//! fn my_function<A>(app: &A) {
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
//! fn foo<A>(a: &A)
//! where
//!     A: Bar
//! {
//!     a.bar();
//! }
//!
//! #[entrait(Bar for App)]
//! fn bar<A>(a: &A) {
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

#![forbid(unsafe_code)]

pub use entrait_macros::entrait;
pub use entrait_macros::generate_mock;

#[macro_export]
macro_rules! expand_mock {
    ($target:tt, [] $($traits:item)*) => {
        entrait::generate_mock!($target $($traits)*);
    };
    ($target:tt, [$macro:ident $($rest_macros:ident)*] $($traits:item)*) => {
        $macro!($target, [$($rest_macros),*] $($traits)*);
    };
}
