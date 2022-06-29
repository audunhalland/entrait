# entrait

[<img alt="crates.io" src="https://img.shields.io/crates/v/entrait.svg?style=for-the-badge&logo=rust" height="20">](https://crates.io/crates/entrait)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/entrait?style=for-the-badge&logo=docs.rs" height="20">](https://docs.rs/entrait)
[<img alt="CI" src="https://img.shields.io/github/workflow/status/audunhalland/entrait/Rust/main?style=for-the-badge&logo=github" height="20">](https://github.com/audunhalland/entrait/actions?query=branch%3Amain)

<!-- cargo-rdme start -->

A proc macro to ease development using _Inversion of Control_ patterns in Rust.

`entrait` is used to generate a trait from the definition of a regular function.
The main use case for this is that other functions may depend upon the trait instead of the concrete implementation, enabling better test isolation.

The macro looks like this:

```rust
#[entrait(MyFunction)]
fn my_function<D>(deps: &D) {
}
```

which generates the trait `MyFunction`:

```rust
trait MyFunction {
    fn my_function(&self);
}
```

`my_function`'s first and only parameter is `deps` which is generic over some unknown type `D`.
This would correspond to the `self` parameter in the trait.
But what is this type supposed to be? The trait gets automatically implemented for [`Impl<T>`](https://docs.rs/implementation/latest/implementation/struct.Impl.html):

```rust
use implementation::Impl;
struct App;

#[entrait::entrait(MyFunction)]
fn my_function<D>(deps: &D) { // <--------------------+
}                             //                      |
                              //                      |
// Generated:                                         |
// trait MyFunction {                                 |
//     fn my_function(&self);                         |
// }                                                  |
//                                                    |
// impl<T> MyFunction for ::implementation::Impl<T> { |
//     fn my_function(&self) {                        |
//         my_function(self) // calls this! ----------+
//     }
// }

let app = Impl::new(App);
app.my_function();
```

The advantage of this pattern comes into play when a function declares its dependencies, as _trait bounds_:


```rust
#[entrait(Foo)]
fn foo(deps: &impl Bar) {
    deps.bar();
}

#[entrait(Bar)]
fn bar<D>(deps: &D) {
}
```

The functions may take any number of parameters, but the first one is always considered specially as the "dependency parameter".

Functions may also be non-generic, depending directly on the App:

```rust
use implementation::Impl;

struct App { something: SomeType };
type SomeType = u32;

#[entrait(Generic)]
fn generic(deps: &impl Concrete) -> SomeType {
    deps.concrete()
}

#[entrait(Concrete)]
fn concrete(app: &App) -> SomeType {
    app.something
}

let app = Impl::new(App { something: 42 });
assert_eq!(42, app.generic());
```

These kinds of functions may be considered "leaves" of a dependency tree.

## "Philosophy"
The `entrait` crate is a building block of a design pattern - the _entrait pattern_.
The entrait pattern is simply a convenient way to achieve unit testing of business logic.

Entrait is not intended for achieving polymorphism. If you want that, you should instead hand-write a trait.

_Entrait should only be used to define an abstract computation that has a single implementation in realase mode, but is mockable in test mode._

`entrait` does not implement Dependency Injection (DI). DI is a strictly object-oriented concept that will often look awkward in Rust.
The author thinks of DI as the "reification of code modules":
  In a DI-enabled programming environment, code modules are grouped together as _objects_ and other modules may depend upon the _interface_ of such an object by receiving some instance that implements it.
When this pattern is applied successively, one ends up with an in-memory dependency graph of high-level modules.

`entrait` tries to turn this around by saying that the primary abstraction that is depended upon is a set of _functions_, not a set of code modules.

An architectural consequence is that one ends up with _one ubiquitous type_ that represents a running application that implements all these function abstraction traits.
But the point is that this is all loosely coupled:
  Most function definitions themselves do not refer to this god-like type, they only depend upon traits.

## Trait visibility
by default, entrait generates a trait that is module-private (no visibility keyword).
To change this, just put a visibility specifier before the trait name:

```rust
use entrait::*;
#[entrait(pub Foo)]   // <-- public trait
fn foo<D>(deps: &D) { // <-- private function
}
```

## `async` support
Since Rust at the time of writing does not natively support async methods in traits, you may opt in to having `#[async_trait]` generated for your trait:

```rust
#[entrait(Foo, async_trait)]
async fn foo<D>(deps: &D) {
}
```
This is designed to be forwards compatible with real async fn in traits.
When that day comes, you should be able to just remove the `async_trait=true` to get a proper zero-cost future.

### Zero-cost `async` inversion of control - preview mode
Entrait has experimental support for zero-cost futures. A nightly Rust compiler is needed for this feature.

The entrait feature is called `associated_future`, and depends on `generic_associated_types` and `type_alias_impl_trait`.
This feature generates an associated future inside the trait, and the implementations use `impl Trait` syntax to infer
the resulting type of the future:

```rust
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use entrait::unimock::*;

#[entrait(Foo, associated_future)]
async fn foo<D>(deps: &D) {
}
```

## Integrating with other `fn`-targeting macros, and `no_deps`
Some macros are used to transform the body of a function, or generate a body from scratch.
For example, we can use [`feignhttp`](https://docs.rs/feignhttp/latest/feignhttp/) to generate an HTTP client. Entrait will try as best as it
can to co-exist with macros like these. Since `entrait` is a higher-level macro that does not touch fn bodies (it does not even try to parse them),
entrait should be processed after, which means it should be placed _before_ lower level macros. Example:

```rust
#[entrait(FetchThing, no_deps, async_trait)]
#[feignhttp::get("https://my.api.org/api/{param}")]
async fn fetch_thing(#[path] param: String) -> feignhttp::Result<String> {}
```

Here we had to use the `no_deps` entrait option.
This is used to tell entrait that the function does not have a `deps` parameter as its first input.
Instead, all the function's inputs get promoted to the generated trait method.

## Trait mocking with Unimock

Entrait works best together with [unimock](https://docs.rs/unimock/latest/unimock/), as these two crates have been designed from the start with each other in mind.

Unimock exports a single mock struct which can be passed in as parameter to every function that accept a `deps` parameter
  (given that entrait is used with unimock support everywhere).
To enable mocking of entraited functions, they get reified and defined as a type called `Fn` inside a module with the same identifier as the function: `entraited_function::Fn`.

Unimock support is enabled by importing entrait from the path `entrait::unimock::*`.

```rust
use entrait::unimock::*;
use unimock::*;

#[entrait(Foo)]
fn foo<D>(_: &D) -> i32 {
    unimplemented!()
}
#[entrait(Bar)]
fn bar<D>(_: &D) -> i32 {
    unimplemented!()
}

fn my_func(deps: &(impl Foo + Bar)) -> i32 {
    deps.foo() + deps.bar()
}

let mocked_deps = mock([
    foo::Fn.each_call(matching!()).returns(40).in_any_order(),
    bar::Fn.each_call(matching!()).returns(2).in_any_order(),
]);

assert_eq!(42, my_func(&mocked_deps));
```

Entrait with unimock supports _un-mocking_. This means that the test environment can be _partially mocked!_

```rust
use entrait::unimock::*;
use unimock::*;
use std::any::Any;

#[entrait(SayHello)]
fn say_hello(deps: &impl FetchPlanetName, planet_id: u32) -> Result<String, ()> {
    Ok(format!("Hello {}!", deps.fetch_planet_name(planet_id)?))
}

#[entrait(FetchPlanetName)]
fn fetch_planet_name(deps: &impl FetchPlanet, planet_id: u32) -> Result<String, ()> {
    let planet = deps.fetch_planet(planet_id)?;
    Ok(planet.name)
}

pub struct Planet {
    name: String
}

#[entrait(FetchPlanet)]
fn fetch_planet(deps: &impl Any, planet_id: u32) -> Result<Planet, ()> {
    unimplemented!("This doc test has no access to a database :(")
}

let hello_string = say_hello(
    &spy([
        fetch_planet::Fn
            .each_call(matching!(_))
            .answers(|_| Ok(Planet {
                name: "World".to_string(),
            }))
            .in_any_order(),
    ]),
    123456,
).unwrap();

assert_eq!("Hello World!", hello_string);
```


## Alternative mocking: Mockall
If you instead wish to use a more established mocking crate, there is also support for [mockall](https://docs.rs/mockall/latest/mockall/).
Note that mockall has some limitations.
Multiple trait bounds are not supported, and deep tests will not work.
Also, mockall tends to generate a lot of code, often an order of magnitude more than unimock.

Just import entrait from `entrait::mockall:*` to have those mock structs autogenerated:

```rust
use entrait::mockall::*;

#[entrait(Foo)]
fn foo<D>(_: &D) -> u32 {
    unimplemented!()
}

fn my_func(deps: &impl Foo) -> u32 {
    deps.foo()
}

fn main() {
    let mut deps = MockFoo::new();
    deps.expect_foo().returning(|| 42);
    assert_eq!(42, my_func(&deps));
}
```

## Conditional compilation of mocks
Most often, you will only need to generate mock implementations in test code, and skip this for production code.
For this configuration there are more alternative import paths:

* `use entrait::unimock_test::*` puts unimock support inside `#[cfg_attr(test, ...)]`.
* `use entrait::mockall_test::*` puts mockall support inside `#[cfg_attr(test, ...)]`.

## Modular applications consisting of several crates
A common technique for Rust application development is to divide them into multiple crates.
Entrait does its best to provide great support for this kind of architecture.
This would be very trivial to do and wouldn't even be worth mentioning here if it wasn't for _concrete deps_.

Further up, concrete dependency was mentioned as leaves of a depdendency tree. Let's imagine we have
an app built from two crates: A `main` which depends on a `lib`:

```rust
mod lib {
    //! lib.rs - pretend this is a separate crate
    pub struct LibConfig {
        pub foo: String,
    }

    #[entrait(pub GetFoo)]
    fn get_foo(config: &LibConfig) -> &str {
        &config.foo
    }

    #[entrait(pub LibFunction)]
    fn lib_function(deps: &impl GetFoo) {
        let foo = deps.get_foo();
    }
}

// main.rs
use implementation::Impl;

struct App {
    lib_config: lib::LibConfig,
}

fn main() {
    let app = Impl::new(App {
        lib_config: lib::LibConfig {
            foo: "value".to_string(),
        }
    });

    use lib::LibFunction;
    app.lib_function();
}
```

How can this be made to work at all? Let's deconstruct what is happening:

1. The library defines it's own configuration: `LibConfig`.
2. It defines a leaf dependency to get access to some property: `GetFoo`.
3. All things which implement `GetFoo` may call `LibFunction`.
4. The main crate defines an `App`, which contains `LibConfig`.
5. The app has the type `Impl<App>`, which means it can call entraited functions.
6. Calling `LibFunction` requires the caller to implement `GetFoo`.
7. `GetFoo` is somehow only implemented for `Impl<LibConfig>`, not `Impl<App>`.

The way Entrait lets you get around this problem is how implementations are generated for concrete leafs:

```rust
// desugared entrait:
fn get_foo(config: &LibConfig) -> &str {
    &config.foo // (3)
}

// generic:
impl<T> GetFoo for Impl<T>
where
    T: GetFoo
{
    fn get_foo(&self) -> &str {
        self.as_ref().get_foo() // calls `<LibConfig as GetFoo>::get_foo`
    }
}

// concrete:
impl GetFoo for LibConfig {
    fn get_foo(&self) -> &str {
        get_foo(self) // calls get_foo, the original function
    }
}
```

We see that `GetFoo` is implemented for all `Impl<T>` where `T: GetFoo`.
So the only thing we need to do to get our app working, is to manually implement `lib::GetFoo for App`, which would just delegate to `self.lib_config.get_foo()`.

We end up with something that has to go through a number of calls to dig out the actual string:

* `<Impl<T> as GetFoo>::get_foo` calls
* `<App as GetFoo>::get_foo` calls
* `<lib::LibConfig as GetFoo>::get_foo` calls
* `lib::get_foo`.

Optmized builds would likely inline a lot of these calls, because all types are fully known at every step.

## Limitations
This section lists known limitations of entrait:

#### Cyclic dependency graphs
Cyclic dependency graphs are impossible with entrait.
In fact, this is not a limit of entrait itself, but with Rust's trait solver.
It is not able to prove that a type implements a trait if it needs to prove that it does in order to prove it.

While this is a limitation, it is not necessarily a bad one.
One might say that a layered application architecture should never contain cycles.
If you do need recursive algorithms, you could model this as utility functions outside of the entraited APIs of the application.

## Crate compatibility
As `entrait` is just a macro, it does not pull in any dependencies besides the code needed to execute the macro.
But in order to _compile_ the generated code, some additional dependencies will be needed alongside `entrait`.
The following table shows compatible major versions:

| `entrait` | `implementation` | `unimock` (optional) | `mockall` (optional) |
| --------- | ---------------- | -------------------- | -------------------- |
| `0.3`     | `0.1`            | `0.2`, `0.3`         | `0.11`               |
| `0.2`     | `-`              | `0.1`                | `0.11`               |
| `0.1`     | `-`              | `-`                  | `0.11`               |

<!-- cargo-rdme end -->
