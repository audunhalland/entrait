# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
