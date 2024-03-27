#! /bin/sh
set -e
set -x

cargo hack --feature-powerset --exclude-features "default nightly-tests" --exclude-no-default-features test
cargo test --workspace --features "unimock"
cargo test --doc --features "unimock"
