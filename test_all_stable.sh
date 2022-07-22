#! /bin/sh
set -x

cargo hack --feature-powerset --optional-deps "unimock" --exclude-features "default use-associated-future" --exclude-no-default-features test
cargo test --doc --features "unimock use-async-trait"
