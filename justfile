#!/usr/bin/env -S just --justfile

_default:
  @just --list -u

alias r := ready

init:
  cargo binstall watchexec-cli taplo-cli typos-cli

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

ready:
  typos
  just fmt
  cargo check
  just lint
  cargo test
