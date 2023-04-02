cargo_artefact_name := "libtoml_vtab"
artefact_ext := if os() == "macos" { ".dylib" } else if os() == "linux" { ".so" } else { ".dll" }
artefact_name := "toml_vtab" + artefact_ext

_default:
  @just --list


build target="test":
  cargo build {{if target == "release" { "--release" } else { "" } }}

clean_dist:
  rm -rf dist
  mkdir dist

[macos]
test_dist: build clean_dist
  cp target/debug/{{cargo_artefact_name}}.dylib dist/toml_vtab

[linux]
test_dist: build clean_dist
  cp target/debug/{{cargo_artefact_name}}.so dist/toml_vtab

# Runs cargo test building the artefact first so it can be loaded by integration tests.
test: test_dist
  cargo test
