#!/usr/bin/env -S just --justfile

_default:
  @just --list -u

init:
  cargo binstall cargo-watch taplo-cli

watch command:
  cargo watch -x '{{command}}'

fmt:
  cargo fmt
  taplo format
