# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.3](https://github.com/lexoliu/executor-core/compare/v0.7.2...v0.7.3) - 2026-11-12

### Other

- Merge remote-tracking branch 'origin/main' into deps/22-compio-fork
- release from main and retire dev
- *(deps)* take compio from the skyzen fork branch

## [0.7.2](https://github.com/lexoliu/executor-core/compare/v0.7.1...v0.7.2) - 2026-11-09

### Added

- compio backend

### Fixed

- mark the erased task handles `#[must_use]`
- gate the tokio backend to non-wasm targets

### Other

- Merge pull request #9 from lexoliu/fix/task-must-use
- open the release-plz bump PR against dev, publish on main
- accept an empty test run for the default feature set
- merge main into dev
- raise the MSRV to 1.95
- gate compio off wasm32 and raise the MSRV to 1.93
- run tests with cargo nextest ([#10](https://github.com/lexoliu/executor-core/pull/10))
- publish to crates.io via OIDC trusted publishing ([#6](https://github.com/lexoliu/executor-core/pull/6))
- gate pull requests into main so only dev may merge ([#7](https://github.com/lexoliu/executor-core/pull/7))
- use Waker::noop() instead of a hand-rolled TestWaker
- use stable toolchain
- add rust-cache to dep-check workflow
- add weekly dependency check workflow

## [0.7.1](https://github.com/lexoliu/executor-core/compare/v0.7.0...v0.7.1) - 2025-12-09

### Other

- remove changelog_config from release configuration
- remove commit message template and unused package update configuration
- update CI workflow to generate lockfile before security audit
- update dependencies in Cargo.toml and adjust smol feature flag
- add Smol executor support and update TokioGlobal struct
- implement TokioGlobal executor for task spawning
- enable publishing and git tagging in release configuration
- remove redundant build and documentation test steps from CI workflow
- update CI workflows and release configuration for improved efficiency and clarity
- refactor CI and release workflows for improved clarity and efficiency
- update release configuration to include version replacers for README.md
- add release configuration to skip CI on release commits

## [0.7.0](https://github.com/lexoliu/executor-core/compare/v0.6.0...v0.7.0) - 2025-11-26

### Other

- Update release workflow to trigger on successful CI completion
- Remove example code snippets from documentation in `lib.rs` and `mailbox.rs`
- Set default feature to "std" in Cargo.toml
- Update WASM build command and adjust default features in Cargo.toml
- Run `cargo fmt`
- Remove CODECOV_TOKEN from coverage upload step in CI configuration
- Update CI configuration to use nightly Rust toolchain and remove MSRV checks
- Update CI configuration for improved workflow and add release workflow
- Enhance panic safety in async task handling and update Tokio tests for synchronous execution
- Update README and enhance async task management with new features
- Update dependencies and simplify task cancellation in Tokio integration
- Add mailbox module for safe cross-thread message passing
- Remove Web/WASM support and related documentation; add initial VSCode settings file
- Update default feature in Cargo.toml and remove TokioExecutor implementation from tokio.rs
- Update default features in Cargo.toml and enhance task management with cancel and result methods in Task implementation
