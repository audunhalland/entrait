# Examples

This directory lists examples of entrait used in combination with other
Rust frameworks.

If you miss some particular example, then please tell me by filing an issue.
PRs are also very welcome!

## Frameworks lacking support for generics
These frameworks have limited support for generics, and seem to have a harder time supporting inversion of control:

* `rocket.rs`: [#408](https://github.com/SergioBenitez/Rocket/issues/408)
* `actix-web`: It'll work, but not in a very idiomatic way. E.g. macros are out: https://stackoverflow.com/a/65646165
