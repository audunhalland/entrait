# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
### Added
- Dependency inversion support with the `#[entrait(TraitImpl, delegate_by = DelegationTrait)] trait Trait {}` syntax.
### Changed
- Make zero-cost futures using a separate macro (`unimock::static_async::async_trait`), comparable to `async_trait`.

## [0.4.3] - 2022-08-01
### Added
- Support for using the entrait attribute on a module.

## [0.4.2] - 2022-07-31
### Added
- `delegate_by = Borrow` option for traits (supports dyn trait leaf dependencies).
### Fixed
- Fix hygiene problem when a parameter has the same ident as the function. Fix uses a hack that appends an underscore to the trait fn param.
- Improved generic params and where clause generation, should generate some fewer tokens overall.
- Doc: Bring back `Impl<T>` impl block code generation example.

## [0.4.1] - 2022-07-25
### Fixed
- Extract idents from destructured fn params and use those in trait fn signature, given that the ident is unambigous.
### Changed
- Refactor/optimize internal where clause generator, avoiding syn::parse_quote

## [0.4.0] - 2022-07-24
### Added
- `implementation` as a dependency, to help users getting started.
- `unimock` feature. Enabling the features downstream will generate mocks upstream.
- `entrait_export` macro and `export` option, for exporting optional mocks from libraries.
- `async-trait` feature for adding a re-export of the async-trait crate.
- `use-async-trait` and `use-associated-future` features for global selection of async strategy.
- Support for generic functions.
- Support for entraiting a trait.
### Changed
- Restructure lib docs.
### Removed
- Support for parameter-less functions without use of `no_deps`. This is technically 'breaking' but can also be seen as a bugfix.
- Submodule import paths (`entrait::unimock`, etc). This is instead enabled by using features.
### Fixed
- Destructured fn params in the original function. Entrait will generate a param name to use in the trait.

## [0.3.4] - 2022-06-30
### Added
- More cargo keywords, categories.

## [0.3.3] - 2022-06-29
### Changed
- The implementation of leaf/concrete dependencies now works a bit differently.
  Instead of the trait being implemented for some concrete `T` in `Impl<T>`, `T` is made generic, but with a `T: Trait` bound.
  Because of that, the trait gets implemented a second time: Directly for the concrete `T`.
  This makes it much easier to seamlessly integrate modular apps divided into many crates.
### Fixed
- Every kind of deps parameter that is not recognized as generic is now legal, and interpreted as being concrete.

## [0.3.2] - 2022-06-27
### Added
- `associated_future` experimental nightly feature, for zero cost futures.

## [0.3.1] - 2022-06-22
### Added
- `no_deps` support. Add this attribute to not interpret the first parameter as a deps parameter.
- default values for config attributes (may skip '= value')

## [0.3.0] - 2022-06-03
### Changed
- Bump unimock to 0.2.0, which removes the need for generic assocated types support

## [0.3.0-beta.0] - 2022-05-15
### Changed
- Improve outputted spans to reflect macro arguments instead of input function
- Bump unimock to next major version (0.2.0)
- Support explicit trait visibility, private/inherited by default

### Removed
- Support for `for T` syntax. The implementations are instead automatically registered with the `implementation` crate.

## [0.2.1] - 2022-03-13
### Added
- `mockall=test` + `unimock=test` support

## [0.2.0] - 2022-03-13
### Added
- `unimock` support.
- `cfg_attr(test, ...)` mock support.
### Removed
- `mock_deps_as`, replaced by `unimock`
- The `entrait_mock_X` procedural macro for multiple-trait-bound mocking.
- The `expand_mock` macro.

## [0.1.0] - 2022-03-07
### Added
- Explicit and opt-in support for `#[async_trait]` with `async_trait = true`.
- Support for `mockall`, with `mockable = true`.
- Support for generating mockall impls for dependencies having multiple trait bounds.

### Changed
- Remove all cargo features. Specific features are now passed as key/value arguments to the macro.
- Split crate into a regular lib and a proc macro crate. `macro_rules` macros and other library functions go in the "outer" `entrait` library.

## [0.0.2] - 2022-02-24
### Changed
- Avoid parsing the full `fn` body. The macro only needs to analyze the signature.

## [0.0.1] - 2022-02-23
### Added
- Basic macro with optional async support using `async-trait`
