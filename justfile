#!/usr/bin/env -S just --justfile

_default:
  @just --list -u

alias r := ready

init:
  cargo binstall watchexec-cli taplo-cli typos-cli cargo-insta

watch *args='':
  watchexec {{args}}

build-release:
  cargo build --release

run-release command:
  ./target/release/cargo-shear {{command}}

fmt:
  cargo fmt
  taplo format

lint:
  cargo clippy

snapshots:
  cargo insta test --accept

ready:
  typos
  just fmt
  cargo check
  just lint
  cargo test
