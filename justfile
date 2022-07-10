# Compile PlatypOS for the default platform
build:
  @cargo xtask build

# Update the local copy of vendored crates
pull-modules:
  git subtree pull --prefix defmt https://github.com/knurling-rs/defmt.git main --squash

fmt:
  cargo fmt --all