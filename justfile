loom_crates := "hal slab"

# Compile PlatypOS for the default platform
build:
  @cargo xtask build

run:
  @cargo xtask run

test:
  @cargo xtask test

fmt:
  cargo fmt --all

loom:
  #!/usr/bin/env bash
  set -euo pipefail
  export RUSTFLAGS="--cfg loom"
  for crate in {{ loom_crates }}; do
    cargo test --release -p "platypos_$crate"
  done

loom-debug crate test:
  @mkdir -p target/loom
  RUSTFLAGS="--cfg loom" \
    LOOM_CHECKPOINT_FILE="$PWD/target/loom/{{crate}}_{{test}}.json" \
    LOOM_CHECKPOINT_INTERVAL=1 \
    LOOM_LOG=1 \
    LOOM_LOCATION=1 \
    RUST_BACKTRACE=1 \
    cargo test --release -p "platypos_{{crate}}" "{{test}}"


setup:
  rustup target add x86_64-unknown-none