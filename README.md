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

which generates a new single-method trait named `MyFunction`, with the method signature derived from the original function.
Entrait is a pure append-only macro: It will never alter the syntax of your function.
The new language items it generates will appear below the function.

In the first example, `my_function` has a single parameter called `deps` which is generic over a type `D`, and represents dependencies injected into the function.
The dependency parameter is always the first parameter, which is analogous to the `&self` parameter of the generated trait method.

To add a dependency, we just introduce a trait bound, now expressable as `impl Trait`.
This is demonstrated by looking at one function calling another:

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

```rust
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

let mocked_deps = unimock::mock([
    foo::Fn.each_call(matching!()).returns(40).in_any_order(),
    bar::Fn.each_call(matching!()).returns(2).in_any_order(),
]);

assert_eq!(42, my_func(&mocked_deps));
```

#### Deep integration testing with unimock
Entrait with unimock supports _un-mocking_. This means that the test environment can be _partially mocked!_

```rust
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
fn fetch_planet(deps: &(), planet_id: u32) -> Result<Planet, ()> {
    unimplemented!("This doc test has no access to a database :(")
}

let hello_string = say_hello(
    &unimock::spy([
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

### Using entrait with a trait
An alternative way to achieve something similar to the above is to use the entrait macro _directly on a trait_.

A typical use case for this is to put core abstractions in some "core" crate, letting other libraries use those core abstractions as dependencies.

```rust
// core_crate
#[entrait]
trait System {
    fn current_time(&self) -> u128;
}

// lib_crate
#[entrait(ComputeSomething)]
fn compute_something(deps: &impl System) {
    let system_time = deps.current_time();
    // do something with the time...
}

// main.rs
struct App;
impl System for App {
    fn current_time(&self) -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }
}

Impl::new(App).compute_something();
```

This is similar to defining a leaf dependency for a concrete type, only in this case, `core_crate` really has no type available to use.
We know that `System` eventually has to be implemented for the application type, and that can happen in the main crate.

The reason that the `#[entrait]` attribute has to be present in `core_crate`, is that it needs to define a blanket implementation for `Impl<T>` (as well as mocks),
    and those need to live in the same crate that defined the trait.
If not, this would have broken the orphan rule.

(NB: This example's purpose is to demonstrate entrait, not to be a guide on how to deal with system time. It should contain some ideas for how to _mock_ time, though!)



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

```rust
#[entrait(Foo, async_trait)]
async fn foo<D>(deps: &D) {
}
```
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

```rust
#[entrait(FetchThing, no_deps)]
#[feignhttp::get("https://my.api.org/api/{param}")]
async fn fetch_thing(#[path] param: String) -> feignhttp::Result<String> {}
```

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

#### Feature overview
| Feature                 | Implies       | Description         |
| -------------------     | ------------- | ------------------- |
| `unimock`               |               | Adds the [unimock] dependency, and turns on Unimock implementations for all traits. |
| `use-async-trait`       | `async_trait` | Automatically applies the [async_trait] macro to async trait methods. |
| `use-associated-future` |               | Automatically transforms the return type of async trait methods into an associated future by using type-alias-impl-trait syntax. Requires a nightly compiler. |
| `async-trait`           |               | Pulls in the [async_trait] optional dependency, enabling the `async_trait` entrait option (macro parameter). |



# "Philosophy"
The `entrait` crate is central to the _entrait pattern_, an opinionated yet flexible way to build testable applications/business logic.

To understand the entrait model and how to achieve Dependency Injection (DI) with it, we can compare it with a more widely used and classical alternative pattern:
    _Object-Oriented DI_.

In object-oriented DI, each named dependency is a separate object instance.
Each dependency exports a set of public methods, and internally points to a set of private dependencies.
A working application is built by fully instantiating such an _object graph_ of interconnected dependencies.

Entrait was built to address two drawbacks inherent to this design:

* Representing a _graph_ of objects (even if acyclic) in Rust usually requires reference counting/heap allocation.
* Each "dependency" abstraction often contains a lot of different functionality.
    As an example, consider [DDD](https://en.wikipedia.org/wiki/Domain-driven_design)-based applications consisting of `DomainServices`.
    There will typically be one such class per domain object, with a lot of methods in each.
    This results in dependency graphs with fewer nodes overall, but the number of possible _call graphs_ is much larger.
    A common problem with this is that the _actual dependencies_—the functions actually getting called—are encapsulated
        and hidden away from public interfaces.
    To construct valid dependency mocks in unit tests, a developer will have to read through full function bodies instead of looking at signatures.

`entrait` solves this by:

* Representing dependencies as _traits_ instead of types, automatically profiting from Rust's builtin zero-cost abstraction tool.
* Having each dependency do only one thing, by abstracting over _functions_ instead of _modules_.
    This is possible because we do not pay anything extra for having more detailed dependency graphs.



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
