//! A proc macro to ease development using _Inversion of Control_ patterns in Rust.
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
//! ## "Philosophy"
//! The idea behind `entrait` is to explore a specific architectural pattern:
//! * Interfaces with _one_ runtime implementation
//! * named traits as the interface of single functions
//!
//! `entrait` does not implement Dependency Injection (DI). DI is a strictly object-oriented concept that will often look awkward in Rust.
//! The author thinks of DI as the "reification of code modules": In a DI-enabled programming environment, code modules are grouped together
//! as _objects_ and other modules may depend upon the _interface_ of such an object by receiving some instance that implements it.
//! When this patteern is applied successively, one ends up with an in-memory dependency graph of high-level modules.
//!
//! `entrait` tries to turn this around by saying that the primary abstraction that is depended upon is a set of _functions_, not a set of code modules.
//!
//! An architectural consequence is that one ends up with _one ubiquitous type_ that represents a running application that implements all
//! these function abstraction traits. But the point is that this is all loosely coupled: Most function definitions themselves do not refer
//! to this god-like type, they only depend upon traits.
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
//! ## Mock support
//! The macro supports autogenerating [mockall] mock structs:
//!
//! [mockall]: https://docs.rs/mockall/latest/mockall/
//!
//! ```rust
//! # use entrait::*;
//! #[entrait(Foo, mockall=true)]
//! fn foo<D>(_: &D) -> u32 {
//!     unimplemented!()
//! }
//!
//! fn my_func(deps: &(impl Foo)) -> u32 {
//!     deps.foo()
//! }
//!
//! fn main() {
//!     let mut deps = MockFoo::new();
//!     deps.expect_foo().returning(|| 42);
//!     assert_eq!(42, my_func(&deps));
//! }
//! ```
//! Using `mockall` is easy enough when there is only one trait bound, because the generated trait need only be attributed with `mockall::automock`.
//!
//! ### multiple trait bounds with `unimock`
//! With multiple trait bounds, this becomes a little harder: We need some concrete struct that implement all the given traits.
//! This is easily solved by the crate [unimock], and using `unimock = true`:
//!
//! [unimock]: https://docs.rs/unimock/latest/unimock/
//!
//! ```rust
//! # use entrait::*;
//! use unimock::Unimock;
//!
//! #[entrait(Foo, unimock=true)]
//! fn foo<D>(_: &D) -> u32 {
//!     unimplemented!()
//! }
//! #[entrait(Bar, unimock=true)]
//! fn bar<D>(_: &D) -> u32 {
//!     unimplemented!()
//! }
//!
//! fn my_func(deps: &(impl Foo + Bar)) -> u32 {
//!     deps.foo() + deps.bar()
//! }
//!
//! fn main() {
//!     let deps = Unimock::new()
//!         .mock(|foo: &mut MockFoo| {
//!             foo.expect_foo().returning(|| 40);
//!         })
//!         .mock(|bar: &mut MockBar| {
//!             bar.expect_bar().returning(|| 2);
//!         });
//!
//!     assert_eq!(42, my_func(&deps));
//! }
//! ```
//!
//! `unimock = true` implies `mockall = true`.
//!
//! ### conditional mock implementations
//! Most often, you will only need to generate mock implementations in test code, and skip this for production code. For this, there are the `test_*` variants:
//!
//! * `test_mockall = true`
//! * `test_unimock = true`
//!
//! which puts the corresponding attributes in `#[cfg_attr(test, ...)]`:
//!
//! ```rust
//! # use entrait::*;
//! #[entrait(Foo, test_unimock=true)]
//! fn foo<D>(_: &D) -> u32 {
//!     unimplemented!()
//! }
//!
//! fn takes_foo(foo: impl Foo) {}
//!
//! fn main() {
//!     // we can still instantiate Unimock, but it's not useful,
//!     // because now it doesn't implement `Foo`:
//!     let mock = unimock::Unimock::new();
//!     //takes_foo(mock);
//!     //--------- ^^^^ the trait `Foo` is not implemented for `Unimock`
//! }
//!
//! #[test]
//! fn test() {
//!     // this compiles!
//!     let mock = unimock::Unimock::new();
//!     takes_foo(mock);
//! }
//! ```
//!
//! This is opt-in because there could be scenarios where this behaviour is not desired, e.g. when you write a library and want mocks exported for those.
//!

#![forbid(unsafe_code)]

pub use entrait_macros::entrait;
