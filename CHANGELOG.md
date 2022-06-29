# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
### Changed
- The implementation of leaf/concrete dependencies now works a bit differently.
  Instead of the trait being implemented for some concrete `T` in `Impl<T>`, `T` is made generic, but with a `T: Trait` bound.
  Because of that, the trait gets implemented a second time: Directly for the concrete `T`.
  This makes it much easier to seamlessly integrate modular apps divided into many crates.

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
