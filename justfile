# Compile PlatypOS for the default platform
build:
  @cargo xtask build

run:
  @cargo xtask run

test:
  @cargo xtask test

fmt:
  cargo fmt --all
