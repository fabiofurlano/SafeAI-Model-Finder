# SafeAI Model Finder

<p align="center">
  <a href="https://safeai.example"><img src="assets/safeai-model-finder-logo.png" alt="SafeAI Model Finder logo" width="160" /></a>
</p>

<p align="center"><a href="https://safeai.example">safeai.example</a></p>

A free, private, local-first desktop tool that recommends the best Ollama
models for your computer, downloads them safely through your existing Ollama
installation, and verifies them locally — all without sending your data to
the cloud.

SafeAI Model Finder runs as a single Rust binary on your machine. It
detects your hardware, opens a local browser tab as its interface, and uses
your local Ollama install to manage models. Nothing about your machine
leaves it.

## Features

- **Hardware-aware recommendations.** Scans your CPU, RAM, GPU, VRAM and
  operating system, then suggests Ollama models that will actually run on
  this machine.
- **Easy Mode and Advanced Mode.** Easy Mode shows one recommended model,
  one faster/lighter alternative, and one better-quality alternative.
  Advanced Mode exposes the full model catalogue with filters, search and
  quantisation picking.
- **Find / Browse / Installed views.** Discover new models or work with
  what is already in your local Ollama store.
- **Performance benchmarks.** Measure real tokens-per-second on your
  hardware before you commit to a model.
- **Hardware Planner.** Project how a model will behave across different
  memory tiers.
- **English and Italian UI.** Internationalised interface.
- **Strict local-only network policy.** The local HTTP server binds only
  to `127.0.0.1`, refuses unexpected `Host` headers, requires a per-launch
  session token on all mutating endpoints, and never falls back to a
  public bind.

## Requirements

SafeAI Model Finder is installed and started from a terminal. You need:

**To install the tool (one-time)**

- A working **Rust** toolchain (Cargo), `rustc` 1.95 or newer.
  Install from <https://rustup.rs> if you do not already have it.
- A standard C toolchain (`cc` / `gcc` / `clang`) — Rust needs a linker
  to compile native dependencies. Most Linux distributions ship one out
  of the box; on macOS install Xcode Command Line Tools
  (`xcode-select --install`); on Windows install the MSVC build tools.
- Network access during installation so Cargo can download the
  published Rust dependencies. No source code or system data is sent
  out.

**To start SafeAI Model Finder**

- The installed binary: `safeai-model-finder` (lands in `~/.cargo/bin`
  after the install step below).
- A web browser on the same machine (Chrome, Firefox, Edge, Safari —
  any modern browser). The browser is the interface only; no data leaves
  the loopback connection.

**To manage and run local models**

- An installed and running **Ollama** on the same machine. If Ollama is
  not present, SafeAI Model Finder surfaces a clear message with the
  download link from <https://ollama.com/download>. Network access is
  needed when *you* initiate a model download through Ollama; SafeAI
  Model Finder itself does not initiate any download without your
  explicit confirmation.

## Install

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked
```

The `--locked` flag pins every dependency to the exact versions recorded
in the in-repo `Cargo.lock` for a reproducible, deterministic install.
The install places a single binary named `safeai-model-finder` in
`~/.cargo/bin` (which Cargo/Rustup already puts on your `PATH`).

To re-install the latest version over a previous one:

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked --force
```

To uninstall:

```bash
cargo uninstall safeai-model-finder
```

## Start

```bash
safeai-model-finder
```

The tool prints a per-launch session token, opens your default browser at
the local URL it served (`http://127.0.0.1:<port>/?token=…`), prints your
detected hardware, and lists any models already installed. Press Ctrl+C to
stop.

If another local program is already using the normal local port, SafeAI
Model Finder automatically picks another free loopback port and prints
a clear line such as:

```
Port 8787 is already in use; using local port 34419 instead.
```

You don't need to do anything — the tool just opens the browser at the
new URL. All traffic remains on `127.0.0.1`; nothing is exposed to the
network.

## Privacy

- The local HTTP server binds only to `127.0.0.1`. It is reachable from
  the browser on the same machine only.
- The server rejects unexpected `Host` headers and requires the session
  token (delivered to the browser URL on launch) on every mutating
  endpoint.
- SafeAI Model Finder does **not** send your hardware information,
  benchmarks or model choices to any third party.
- It does not register an account, does not phone home, and has no
  telemetry.
- Network activity is limited to:
  1. Cargo fetching the project's Rust dependencies at install time.
  2. The local Ollama service on loopback during normal operation.
  3. The Ollama model download you explicitly confirm in the interface.

## SafeAI compatibility

SafeAI Model Finder manages models through the user's local Ollama
installation. Any model it downloads is stored in the same Ollama model
directory that any other local Ollama-aware application uses, including
the SafeAI application where configured to point at the same Ollama
instance. SafeAI Model Finder does not modify the SafeAI application, its
files, or its configuration; it only shares the Ollama model store.

## Acknowledgements

SafeAI Model Finder is built upon
[llmfit](https://github.com/AlexsJones/llmfit) by Alex Jones and
contributors, pinned at upstream tag v1.1.8. The upstream MIT licence is
preserved at the root of this repository as `LICENSE`.

We thank the llmfit project for its hardware detection, model fitting
core and benchmark data that power this product. SafeAI Model Finder is
an independent fork from the upstream; the upstream authors have not
reviewed or endorsed this fork.

## Licence

This product is released under the MIT licence — see `LICENSE` at the
root of this repository. The licence inherits from the upstream llmfit
project.
