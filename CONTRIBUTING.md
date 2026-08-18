# Contributing to SafeAI Model Finder

Thanks for your interest in SafeAI Model Finder. This document covers the
local development loop for this repository only. Issues and pull requests
are welcome.

## Prerequisites

- **Rust** toolchain, edition 2024, MSRV 1.95 or newer. Install from
  <https://rustup.rs>.
- A standard C toolchain (`cc`, `gcc` or `clang`).
- **Git**.
- (Optional) **Ollama** installed and running locally so you can exercise
  the live Ollama endpoints. The build and the unit/integration test
  suite do not require Ollama; only manual smoke tests do.

## Repository layout

SafeAI Model Finder is a Cargo workspace with two members:

- `safeai-model-finder/` — the SafeAI Model Finder binary (the product).
- `llmfit-core/` — the hardware detection, model fitting and Ollama
  integration library that backs the product. This crate is forked
  from upstream `AlexsJones/llmfit` pinned at upstream tag v1.1.8.

The browser interface is fully embedded by `safeai-model-finder/build.rs`
into the binary at build time, so a separate "build the UI" step is not
required.

## Building from source

From the repository root:

```sh
cargo build -p safeai-model-finder
```

Or use the convenience target:

```sh
make build          # debug build
make release        # release build
make test           # run all unit and integration tests
make fmt            # cargo fmt
make clippy         # cargo clippy
make install        # install release binary to ~/.cargo/bin
```

## Running

After `make build` (or `cargo build -p safeai-model-finder`), launch the
binary directly:

```sh
./target/debug/safeai-model-finder
```

The tool prints a per-launch session token, opens the default browser
and serves its UI from `http://127.0.0.1:<port>/`. Press Ctrl+C to stop.

## Tests

```sh
cargo test -p safeai-model-finder
cargo test -p llmfit-core
```

The first command runs the SafeAI Model Finder unit and integration tests
(host-binding, session token precedence, route contract, etc.). The
second runs the upstream-derived library tests (schema validation, etc.).

## Coding style

- `cargo fmt` for Rust formatting.
- `cargo clippy` for linting.
- Public functions should keep their security reasoning visible in
  comments where it is non-obvious (loopback binding, session token
  precedence, body parsing, concurrent-download protection, etc.).
- Do not introduce silent fallbacks that change the network address,
  the model source, or the data path.
- Do not pull new heavy dependencies without prior discussion.

## Submitting changes

1. Fork the repository and create a feature branch.
2. Keep commits focused: one bounded change per commit when practical.
3. Ensure `cargo build -p safeai-model-finder`,
   `cargo test -p safeai-model-finder` and `cargo clippy` all pass
   before requesting review.
4. Reference any related issue in the commit body.
5. Open a pull request describing the change, root cause, and how it
   was verified.

## Reporting security issues

Please do not file public issues for security-sensitive defects. Use
the repository's private vulnerability reporting channel (or the
maintainer contact in the project README) so a fix can be prepared
before disclosure.
