//! A proc macro to ease development using _Inversion of Control_ patterns in Rust.
//!
//! `entrait` is used to generate a trait from the definition of a regular function.
//! The main use case for this is that other functions may depend upon the trait instead of the concrete implementation, enabling better test isolation.
//!
//! The macro looks like this:
//!
//! ```rust
//! # use entrait::entrait;
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
//! But what is this type supposed to be? The trait gets automatically implemented for [`Impl<T>`](https://docs.rs/implementation/latest/implementation/struct.Impl.html):
//!
//! ```rust
//! use implementation::Impl;
//! struct App;
//!
//! #[entrait::entrait(MyFunction)]
//! fn my_function<D>(deps: &D) { // <--------------------+
//! }                             //                      |
//!                               //                      |
//! // Generated:                                         |
//! // trait MyFunction {                                 |
//! //     fn my_function(&self);                         |
//! // }                                                  |
//! //                                                    |
//! // impl<T> MyFunction for ::implementation::Impl<T> { |
//! //     fn my_function(&self) {                        |
//! //         my_function(self) // calls this! ----------+
//! //     }
//! // }
//!
//! let app = Impl::new(App);
//! app.my_function();
//! ```
//!
//! The advantage of this pattern comes into play when a function declares its dependencies, as _trait bounds_:
//!
//!
//! ```rust
//! # use entrait::entrait;
//! # struct App;
//! #[entrait(Foo)]
//! fn foo(deps: &impl Bar) {
//!     deps.bar();
//! }
//!
//! #[entrait(Bar)]
//! fn bar<D>(deps: &D) {
//! }
//! ```
//!
//! The functions may take any number of parameters, but the first one is always considered specially as the "dependency parameter".
//!
//! Functions may also be non-generic, depending directly on the App:
//!
//! ```rust
//! # use entrait::entrait;
//! use implementation::Impl;
//!
//! struct App { something: SomeType };
//! type SomeType = u32;
//!
//! #[entrait(Generic)]
//! fn generic(deps: &impl Concrete) -> SomeType {
//!     deps.concrete()
//! }
//!
//! #[entrait(Concrete)]
//! fn concrete(app: &App) -> SomeType {
//!     app.something
//! }
//!
//! let app = Impl::new(App { something: 42 });
//! assert_eq!(42, app.generic());
//! ```
//!
//! These kinds of functions may be considered "leaves" of a dependency tree.
//!
//! ## "Philosophy"
//! The `entrait` crate is a building block of a design pattern - the _entrait pattern_.
//! The entrait pattern is simply a convenient way to achieve unit testing of business logic.
//!
//! Entrait is not intended for achieving polymorphism. If you want that, you should instead hand-write a trait.
//!
//! _Entrait should only be used to define an abstract computation that has a single implementation in realase mode, but is mockable in test mode._
//!
//! `entrait` does not implement Dependency Injection (DI). DI is a strictly object-oriented concept that will often look awkward in Rust.
//! The author thinks of DI as the "reification of code modules":
//!   In a DI-enabled programming environment, code modules are grouped together as _objects_ and other modules may depend upon the _interface_ of such an object by receiving some instance that implements it.
//! When this pattern is applied successively, one ends up with an in-memory dependency graph of high-level modules.
//!
//! `entrait` tries to turn this around by saying that the primary abstraction that is depended upon is a set of _functions_, not a set of code modules.
//!
//! An architectural consequence is that one ends up with _one ubiquitous type_ that represents a running application that implements all these function abstraction traits.
//! But the point is that this is all loosely coupled:
//!   Most function definitions themselves do not refer to this god-like type, they only depend upon traits.
//!
//! ## Trait visibility
//! by default, entrait generates a trait that is module-private (no visibility keyword).
//! To change this, just put a visibility specifier before the trait name:
//!
//! ```rust
//! use entrait::*;
//! #[entrait(pub Foo)]   // <-- public trait
//! fn foo<D>(deps: &D) { // <-- private function
//! }
//! ```
//!
//! ## `async` support
//! Since Rust at the time of writing does not natively support async methods in traits, you may opt in to having `#[async_trait]` generated for your trait:
//!
//! ```rust
//! # use entrait::entrait;
//! #[entrait(Foo, async_trait)]
//! async fn foo<D>(deps: &D) {
//! }
//! ```
//! This is designed to be forwards compatible with real async fn in traits.
//! When that day comes, you should be able to just remove the `async_trait=true` to get a proper zero-cost future.
//!
//! ## Integrating with other `fn`-targeting macros, and `no_deps`
//! Some macros are used to transform the body of a function, or generate a body from scratch.
//! For example, we can use [`feignhttp`](https://docs.rs/feignhttp/latest/feignhttp/) to generate an HTTP client. Entrait will try as best as it
//! can to co-exist with macros like these. Since `entrait` is a higher-level macro that does not touch fn bodies (it does not even try to parse them),
//! entrait should be processed after, which means it should be placed _before_ lower level macros. Example:
//!
//! ```rust
//! # use entrait::entrait;
//! #[entrait(FetchThing, no_deps, async_trait)]
//! #[feignhttp::get("https://my.api.org/api/{param}")]
//! async fn fetch_thing(#[path] param: String) -> feignhttp::Result<String> {}
//! ```
//!
//! Here we had to use the `no_deps` entrait option.
//! This is used to tell entrait that the function does not have a `deps` parameter as its first input.
//! Instead, all the function's inputs get promoted to the generated trait method.
//!
//! ## Mock support
//!
//! #### Unimock
//! Entrait works best together with [unimock](https://docs.rs/unimock/latest/unimock/), as these two crates have been designed from the start with each other in mind.
//!
//! Unimock exports a single mock struct which can be passed in as parameter to every function that accept a `deps` parameter
//!   (given that entrait is used with unimock support everywhere).
//! To enable mocking of entraited functions, they get reified and defined as a type called `Fn` inside a module with the same identifier as the function: `entraited_function::Fn`.
//!
//! Unimock support is enabled by importing entrait from the path `entrait::unimock::*`.
//!
//! ```rust
//! use entrait::unimock::*;
//! use unimock::*;
//!
//! #[entrait(Foo)]
//! fn foo<D>(_: &D) -> i32 {
//!     unimplemented!()
//! }
//! #[entrait(Bar)]
//! fn bar<D>(_: &D) -> i32 {
//!     unimplemented!()
//! }
//!
//! fn my_func(deps: &(impl Foo + Bar)) -> i32 {
//!     deps.foo() + deps.bar()
//! }
//!
//! let mocked_deps = mock([
//!     foo::Fn.each_call(matching!()).returns(40).in_any_order(),
//!     bar::Fn.each_call(matching!()).returns(2).in_any_order(),
//! ]);
//!
//! assert_eq!(42, my_func(&mocked_deps));
//! ```
//!
//! Entrait with unimock supports _unmocking_. This means that the test environment can be _partially mocked!_
//!
//! ```rust
//! use entrait::unimock::*;
//! use unimock::*;
//! use std::any::Any;
//!
//! #[entrait(SayHello)]
//! fn say_hello(deps: &impl FetchPlanetName, planet_id: u32) -> Result<String, ()> {
//!     Ok(format!("Hello {}!", deps.fetch_planet_name(planet_id)?))
//! }
//!
//! #[entrait(FetchPlanetName)]
//! fn fetch_planet_name(deps: &impl FetchPlanet, planet_id: u32) -> Result<String, ()> {
//!     let planet = deps.fetch_planet(planet_id)?;
//!     Ok(planet.name)
//! }
//!
//! pub struct Planet {
//!     name: String
//! }
//!
//! #[entrait(FetchPlanet)]
//! fn fetch_planet(deps: &impl Any, planet_id: u32) -> Result<Planet, ()> {
//!     unimplemented!("This doc test has no access to a database :(")
//! }
//!
//! let hello_string = say_hello(
//!     &spy([
//!         fetch_planet::Fn
//!             .each_call(matching!(_))
//!             .answers(|_| Ok(Planet {
//!                 name: "World".to_string(),
//!             }))
//!             .in_any_order(),
//!     ]),
//!     123456,
//! ).unwrap();
//!
//! assert_eq!("Hello World!", hello_string);
//! ```
//!
//!
//! #### mockall
//! If you instead wish to use a more established mocking crate, there is also support for [mockall](https://docs.rs/mockall/latest/mockall/).
//!
//! Just import entrait from `entrait::mockall:*` to have those mock structs autogenerated:
//!
//! ```rust
//! use entrait::mockall::*;
//!
//! #[entrait(Foo)]
//! fn foo<D>(_: &D) -> u32 {
//!     unimplemented!()
//! }
//!
//! fn my_func(deps: &impl Foo) -> u32 {
//!     deps.foo()
//! }
//!
//! fn main() {
//!     let mut deps = MockFoo::new();
//!     deps.expect_foo().returning(|| 42);
//!     assert_eq!(42, my_func(&deps));
//! }
//! ```
//!
//! #### conditional mock implementations
//! Most often, you will only need to generate mock implementations in test code, and skip this for production code.
//! For this configuration there are more alternative import paths:
//!
//! * `use entrait::unimock_test::*` puts unimock support inside `#[cfg_attr(test, ...)]`.
//! * `use entrait::mockall_test::*` puts mockall support inside `#[cfg_attr(test, ...)]`.
//!
//! ## Limitations
//! This section lists known limitations of entrait:
//!
//! #### Cyclic dependency graphs
//! Cyclic dependency graphs are impossible with entrait.
//! In fact, this is not a limit of entrait itself, but with Rust's trait solver.
//! It is not able to prove that a type implements a trait if it needs to prove that it does in order to prove it.
//!
//! While this is a limitation, it is not necessarily a bad one.
//! One might say that a layered application architecture should never contain cycles.
//! If you do need recursive algorithms, you could model this as utility functions outside of the entraited APIs of the application.
//!
//! ## Crate compatibility
//! As `entrait` is just a macro, it does not pull in any dependencies besides the code needed to execute the macro.
//! But in order to _compile_ the generated code, some additional dependencies will be needed alongside `entrait`.
//! The following table shows compatible major versions:
//!
//! | `entrait` | `implementation` | `unimock` (optional) | `mockall` (optional) |
//! | --------- | ---------------- | -------------------- | -------------------- |
//! | `0.3`     | `0.1`            | `0.2`, `0.3`         | `0.11`               |
//! | `0.2`     | `-`              | `0.1`                | `0.11`               |
//! | `0.1`     | `-`              | `-`                  | `0.11`               |
//!

#![forbid(unsafe_code)]

pub use entrait_macros::entrait;

/// Unimock shorthand
pub mod unimock {
    /// Re-export of `entrait` with `unimock` implied.
    ///
    /// # Example
    ///
    /// ```rust
    /// use entrait::unimock::*;
    /// ```
    pub use entrait_macros::entrait_unimock as entrait;
}

/// Unimock cfg-test-only shorthand
pub mod unimock_test {
    /// Re-export of `entrait` with `unimock = test` implied.
    /// # Example
    ///
    /// ```rust
    /// use entrait::unimock_test::*;
    /// ```
    pub use entrait_macros::entrait_unimock_test as entrait;
}

/// Mockall shorthand
pub mod mockall {
    /// Re-export of `entrait` with `mockall` implied.
    ///
    /// # Example
    ///
    /// ```rust
    /// use entrait::mockall::*;
    /// ```
    pub use entrait_macros::entrait_mockall as entrait;
}

/// Mockall test-mode-only shorthand
pub mod mockall_test {
    /// Re-export of `entrait` with `mockall = test` implied.
    ///
    /// # Example
    ///
    /// ```rust
    /// use entrait::mockall_test::*;
    /// ```
    pub use entrait_macros::entrait_mockall_test as entrait;
}
