#! /bin/sh
set -e
set -x

cargo hack --feature-powerset --exclude-features "default use-associated-futures nightly-tests" --exclude-no-default-features test
cargo test --workspace --features "unimock use-boxed-futures"
cargo test --doc --features "unimock use-boxed-futures"
