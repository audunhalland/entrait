# entrait

[<img alt="crates.io" src="https://img.shields.io/crates/v/entrait.svg?style=for-the-badge&logo=rust" height="20">](https://crates.io/crates/entrait)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/entrait?style=for-the-badge&logo=docs.rs" height="20">](https://docs.rs/entrait)
[<img alt="CI" src="https://img.shields.io/github/workflow/status/audunhalland/entrait/Rust/main?style=for-the-badge&logo=github" height="20">](https://github.com/audunhalland/entrait/actions?query=branch%3Amain)

<!-- cargo-rdme start -->

A proc macro for designing loosely coupled Rust applications.

[`entrait`](entrait) is used to generate an _implemented trait_ from the definition of a regular function.
The emergent pattern that results from its use enable the following things:
* Zero-cost loose coupling and inversion of control
* Dependency graph as a compile time concept
* Mock library integrations
* Clean, readable, boilerplate-free code

The resulting pattern is referred to as [the entrait pattern](https://audunhalland.github.io/blog/entrait-pattern/) (see also: [philosophy](#philosophy)).

# Introduction

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

`my_function`'s first and only parameter is `deps` which is generic over some unknown type `D`, and represents dependencies injected into the function.
The dependency parameter is always the first parameter, analogous to the `self` parameter of a method.

A dependency is just a trait bound, expressable as `impl Trait`. This is demonstrated by looking at one function calling another:

```rust
#[entrait(Foo)]
fn foo(deps: &impl Bar) {
    println!("{}", deps.bar(42));
}

#[entrait(Bar)]
fn bar<D>(deps: &D, n: i32) -> String {
    format!("You passed {n}")
}
```

### Multiple dependencies
Other frameworks might represent multiple dependencies by having one value for each one, but entrait represents all dependencies _within the same value_.
When the dependency parameter is generic, its trait bounds specifiy what methods we expect to be callable inside the function.

Multiple bounds can be expressed using the `&(impl A + B)` syntax.

The single-value dependency design means that it is always the same reference that is passed around everywhere.
But a reference to what, exactly?
This is what we have managed to abstract away, which is the [whole point](#testing).


### Runtime and implementation
When we want to compile a working application, we need an actual type to inject into the various entrait entrypoints.
Two things will be important:

* All trait bounds used deeper in the graph will implicitly "bubble up" to the entrypoint level, so the type we eventually use will need to implement all those traits in order to type check.
* The implementations of these traits need to do the correct thing: Actually call the entraited function, so that the dependency graph is turned into an actual _call graph_.

Entrait generates _implemented traits_, and the type to use for linking it all together is `Impl<T>`:

```rust
#[entrait(Foo)]
fn foo(deps: &impl Bar) -> i32 {
    deps.bar()
}

#[entrait(Bar)]
fn bar(_deps: &impl std::any::Any) -> i32 {
    42
}

let app = Impl::new(());
assert_eq!(42, app.foo());
```

`Impl` is generic, so we can put whatever type we want into it.
Normally this would be some type that represents the global state/configuration of the running application.
But if dependencies can only be traits, and we always abstract away this type, how can this state ever be accessed?

### Concrete dependencies
So far we have only seen generic trait-based dependencies, but the dependency can also be a _concrete type_:

```rust
struct Config(i32);

#[entrait(UseTheConfig)]
fn use_the_config(config: &Config) -> i32 {
    config.0
}

#[entrait(DoubleIt)]
fn double_it(deps: &impl UseTheConfig) -> i32 {
    deps.use_the_config() * 2
}

assert_eq!(42, Impl::new(Config(21)).double_it());
```

The parameter of `use_the_config` is in the first position, so it represents the dependency.

We will notice two interesting things:
* Functions that depend on `UseTheConfig`, either directly or indirectly, now have only one valid dependency type: `Impl<Config>`<sup>[1](#desugaring-of-concrete-deps)</sup>.
* Inside `use_the_config`, we have a `&Config` reference instead of `&Impl<Config>`. This means we cannot call other entraited functions, because they are not implemented for `Config`.

The last point means that a concrete dependency is the end of the line, a leaf in the dependency graph.

Typically, functions with a concrete dependency should be kept small and avoid extensive business logic.
They ideally function as accessors, providing a loosely coupled abstraction layer over concrete application state.


# Testing
## Trait mocking with `Unimock`

The whole point of entrait is to provide inversion of control, so that alternative dependency implementations can be used when unit testing function bodies.
While test code can contain manual trait implementations, the most ergonomic way to test is to use a mocking library, which provides more features with less code.

Entrait works best together with [unimock](https://docs.rs/unimock/latest/unimock/), as these two crates have been designed from the start with each other in mind.

Unimock exports a single mock struct which can be passed as argument to every function that accept a generic `deps` parameter
  (given that entrait is used with unimock support everywhere).
To enable mocking of entraited functions, they get reified and defined as a type called `Fn` inside a module with the same identifier as the function: `entraited_function::Fn`.

Unimock support is enabled by passing the `unimock` option to entrait (`#[entrait(Foo, unimock)]`), or turning on the `unimock` _feature_, which makes all entraited functions mockable, even in upstream crates.


#### Deep integration testing with unimock
Entrait with unimock supports _un-mocking_. This means that the test environment can be _partially mocked!_


This example used [`unimock::spy`](unimock::spy) to create a mocker that works mostly like `Impl`, except that the call graph can be short-circuited at arbitrary, run-time configurable points.
The example code goes through three layers (`say_hello => fetch_planet_name => fetch_planet`), and only the deepest one gets mocked out.


### Alternative mocking: Mockall
If you instead wish to use a more established mocking crate, there is also support for [mockall](https://docs.rs/mockall/latest/mockall/).
Note that mockall has some limitations.
Multiple trait bounds are not supported, and deep tests will not work.
Also, mockall tends to generate a lot of code, often an order of magnitude more than unimock.

Enabling mockall is done using the `mockall` entrait option.
There is no cargo feature to turn this on implicitly, because mockall doesn't work well when it's re-exported through another crate.

```rust
#[entrait(Foo, mockall)]
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


# Options and features

#### Trait visibility
by default, entrait generates a trait that is module-private (no visibility keyword).
To change this, just put a visibility specifier before the trait name:

```rust
use entrait::*;
#[entrait(pub Foo)]   // <-- public trait
fn foo<D>(deps: &D) { // <-- private function
}
```

#### `async` support
Since Rust at the time of writing does not natively support async methods in traits, you may opt in to having `#[async_trait]` generated for your trait:

This is designed to be forwards compatible with real async fn in traits.
When that day comes, you should be able to just remove the `async_trait` to get a proper zero-cost future.

There is a feature to automatically turn on `async_trait` for every async entrait function: `use-async-trait`.
This feature turns this on for all upstream crates that also exports entraited functions.

#### Zero-cost async inversion of control - preview mode
Entrait has experimental support for zero-cost futures. A nightly Rust compiler is needed for this feature.

The entrait feature is called `associated_future`, and depends on `generic_associated_types` and `type_alias_impl_trait`.
This feature generates an associated future inside the trait, and the implementations use `impl Trait` syntax to infer
the resulting type of the future:

```rust
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use entrait::*;

#[entrait(Foo, associated_future)]
async fn foo<D>(deps: &D) {
}
```

There is a feature for turning this on everywhere: `use-associated-future`.

#### Integrating with other `fn`-targeting macros, and `no_deps`
Some macros are used to transform the body of a function, or generate a body from scratch.
For example, we can use [`feignhttp`](https://docs.rs/feignhttp/latest/feignhttp/) to generate an HTTP client. Entrait will try as best as it
can to co-exist with macros like these. Since `entrait` is a higher-level macro that does not touch fn bodies (it does not even try to parse them),
entrait should be processed after, which means it should be placed _before_ lower level macros. Example:


Here we had to use the `no_deps` entrait option.
This is used to tell entrait that the function does not have a `deps` parameter as its first input.
Instead, all the function's inputs get promoted to the generated trait method.

#### Conditional compilation of mocks
Most often, you will only need to generate mock implementations for test code, and skip this for production code.
A notable exception to this is when building libraries.
When an application consists of several crates, downstream crates would likely want to mock out functionality from libraries.

Entrait calls this _exporting_, and it unconditionally turns on autogeneration of mock implementations:

```rust
#[entrait_export(pub Bar)]
fn bar(deps: &()) {}
```
or
```rust
#[entrait(pub Foo, export)]
fn foo(deps: &()) {}
```

It is also possible to reduce noise by doing `use entrait::entrait_export as entrait`.




# Modular applications consisting of several crates

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

    #[entrait_export(pub GetFoo)]
    fn get_foo(config: &LibConfig) -> &str {
        &config.foo
    }

    #[entrait_export(pub LibFunction)]
    fn lib_function(deps: &impl GetFoo) {
        let foo = deps.get_foo();
    }
}

// main.rs
struct App {
    lib_config: lib::LibConfig,
}

fn main() {
    use entrait::*;

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
3. All things which implement `GetFoo` may call `lib_function`.
4. The main crate defines an `App`, which contains `LibConfig`.
5. The app has the type `Impl<App>`, which means it can call entraited functions.
6. Calling `LibFunction` requires the caller to implement `GetFoo`.
7. `GetFoo` is somehow only implemented for `Impl<LibConfig>`, not `Impl<App>`.

#### Desugaring of concrete deps
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

We end up with quite a dance to actually dig out the config string:

```text
<Impl<App> as lib::LibFunction>::lib_function() lib.rs
=> <Impl<App> as lib::GetFoo>::get_foo() lib.rs
  => <App as lib::GetFoo>::get_foo() main.rs: hand-written implementation
    => <lib::LibConfig as lib::GetFoo>::get_foo() lib.rs
      => lib::get_foo(config) lib.rs
```

Optmized builds should inline a lot of these calls, because all types are fully known at every step.

# "Philosophy"
The `entrait` crate is a building block of a design pattern - the _entrait pattern_.
The entrait pattern is simply a convenient way to achieve unit testing of business logic.

Entrait is not intended for achieving polymorphism. If you want that, you should instead hand-write a trait.

_Entrait should only be used to define an abstract computation that has a single implementation in realase mode, but is mockable in test mode._

`entrait` does not implement Dependency Injection (DI) in the classical, Object Oriented sense.
Classical DI is an object-oriented concept that will often look awkward in Rust.
The author thinks of DI as the "reification of code modules":
  In a DI-enabled programming environment, code modules are grouped together as _objects_ and other modules may depend upon the _interface_ of such an object by receiving some instance that implements it.
When this pattern is applied successively, one ends up with an in-memory dependency graph of high-level modules.

`entrait` tries to turn this around by saying that the primary abstraction that is depended upon is a set of _functions_, not a set of code modules.

# Limitations
This section lists known limitations of entrait:

### Cyclic dependency graphs
Cyclic dependency graphs are impossible with entrait.
In fact, this is not a limit of entrait itself, but with Rust's trait solver.
It is not able to prove that a type implements a trait if it needs to prove that it does in order to prove it.

While this is a limitation, it is not necessarily a bad one.
One might say that a layered application architecture should never contain cycles.
If you do need recursive algorithms, you could model this as utility functions outside of the entraited APIs of the application.

<!-- cargo-rdme end -->
