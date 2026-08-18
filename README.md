# SafeAI Model Finder

<p align="center">
  <a href="https://ai-insider.site/"><img src="assets/safeai-model-finder-logo.png" alt="SafeAI Model Finder logo" width="160" /></a>
</p>

<p align="center"><a href="https://ai-insider.site/">ai-insider.site</a></p>

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

To uninstall:

```bash
cargo uninstall safeai-model-finder
```

---

## Update

You do **not** need to uninstall SafeAI Model Finder before updating.
The same `cargo install --git … --locked --force` invocation that
installs the binary the first time also replaces the currently
installed command with the latest version published in the public
GitHub repository:

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked --force
```

- `--git …` fetches the latest public `main` commit.
- `--locked` pins every dependency to the exact versions recorded
  in the in-repo `Cargo.lock` for a reproducible, deterministic build.
- `--force` overwrites the existing `~/.cargo/bin/safeai-model-finder`
  binary without you having to run `cargo uninstall` first.

Your settings, your existing models, your downloaded models, and your
Ollama environment are untouched. Only the binary itself is replaced.
After the command finishes, just run `safeai-model-finder` again.

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

## SafeAI Desktop and SafeAI Office

SafeAI Model Finder, **SafeAI Desktop**, and **SafeAI Office** can all
share one local Ollama environment. When the three are configured to
talk to the same Ollama instance on the same machine, any model that
SafeAI Model Finder downloads for you is placed in Ollama's normal
model directory and is therefore visible and usable from SafeAI Desktop
and SafeAI Office as well — without any extra step, import, or
synchronisation.

- SafeAI Model Finder **only** writes to your local Ollama model
  directory. It does **not** read, write, or modify SafeAI Desktop,
  SafeAI Office, or any of their files, settings, or configuration.
- No background syncing. No cloud relay. No import wizard. The
  "sharing" is the fact that all three apps point at the same local
  Ollama instance.
- If you only run SafeAI Model Finder and never start SafeAI Desktop or
  SafeAI Office, nothing changes for you. The compatibility is purely
  additive: the same downloaded model is discoverable by those apps
  when you also use them, without any extra action from SafeAI Model
  Finder.
- Removing a model from SafeAI Model Finder removes it from Ollama's
  model directory, so it disappears from SafeAI Desktop and SafeAI
  Office too. This is normal Ollama behaviour — not a SafeAI Model
  Finder action against those apps.

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
