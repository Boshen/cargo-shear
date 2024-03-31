#!/usr/bin/env -S just --justfile

_default:
  @just --list -u

init:
  cargo binstall cargo-watch taplo-cli

watch command:
  cargo watch -x '{{command}}'

build-release:
  cargo build --release

run-release command:
  ./target/release/cargo-shear {{command}}

fmt:
  cargo fmt
  taplo format

lint:
  cargo clippy

ready:
  typos
  just fmt
  cargo check
  just lint
  cargo test
