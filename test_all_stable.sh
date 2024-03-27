#! /bin/sh
set -e
set -x

cargo hack --feature-powerset --exclude-features "default" --exclude-no-default-features test
cargo test --workspace --features "unimock"
cargo test --doc --features "unimock"
