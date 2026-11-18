# 0022. WASM plugin runtime

**Date**: 2026-11-18
**Status**: Complete

## Summary

The WASM plugin runtime loads and validates WebAssembly binary modules (`.wasm` MVP format) in a strict, fuel metered, pure Rust sandbox with zero `unsafe` code and zero external native C runtimes. Plugins can inspect numeric or linear memory data and register custom result transformations or SQL metrics without access to the filesystem, network, or credentials.

## Context

Users want extensibility for custom value formatters, numeric transforms, and SQL complexity checks without compromising the security boundary of the native desktop application (`unsafe_code = "forbid"`).

## Requirements

**Acceptance criteria**
* AC-1: `WasmPluginModule` in `unidb-core::plugin` parses WebAssembly binary headers (`\0asm` magic and version 1), Type, Function, Export, and Code sections with strict bounds validation.
* AC-2: A fuel metered stack machine executes exported `i32` functions safely (`i32.const`, `local.get`, `i32.add`, `i32.sub`, `i32.mul`, `i32.div_s`, `i32.rem_s`, `i32.eq`, `i32.ne`, `i32.lt_s`, `i32.gt_s`, `drop`, `end`), halting with a safe error if fuel is exhausted or stack underflows.
* AC-3: `AppService` supports registering `.wasm` plugins (`install_plugin`), listing installed plugins (`list_plugins`), invoking an exported plugin function (`run_plugin`), and removing plugins (`remove_plugin`), persisted in `settings` or store metadata and audited via `plugin_installed` / `plugin_executed`.
* AC-4: Unit tests verify valid `.wasm` binary parsing, execution, divide by zero trapping, fuel exhaustion trapping, and invalid magic rejection.
* AC-5: The Platform page includes a Plugins view to inspect, run, and manage sandboxed WASM plugins.

## Decision

* **Pure Rust MVP Sandbox**: `crates/unidb-core/src/plugin.rs` parses real WebAssembly binary sections (`0x01` Type, `0x03` Function, `0x07` Export, `0x0a` Code) without adding external JIT crates or `unsafe` code. Execution uses an explicit operand stack and configurable instruction fuel limit (default 10,000 steps).
* **Persistence**: Installed plugin metadata and base64 encoded `.wasm` payloads are stored in `unidb-store` (via `plugin_records` or settings registry) and verified on load.

## Build plan

1. Add `crates/unidb-core/src/plugin.rs` with binary parser, validator, fuel metered VM, and unit tests.
2. Add plugin management methods to `AppService` in `crates/unidb-app` with integration tests.
3. Wire Plugins view into the Platform section in `crates/unidb-ui`.
4. Verify with workspace tests, clippy, fmt, and container GUI screenshots.
