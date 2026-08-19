# Install SafeAI Model Finder on macOS — with an AI agent

> **Project status: NOT YET VALIDATED on macOS.** This file is an
> **informational draft**. The Linux install path
> ([`INSTALL-LINUX.md`](INSTALL-LINUX.md)) is the only install path
> proven end-to-end by this project. The macOS path described below
> has not been independently recorded as a clean-machine success by
> a Mac owner. The agent and the user must understand that going
> through this prompt on macOS is best-effort, not a supported path,
> until a Mac maintainer does that clean-machine proof and reports
> back.
>
> If you only have macOS available, the **safest option** is to use a
> Linux VM (Ubuntu / Fedora) instead and follow
> [`INSTALL-LINUX.md`](INSTALL-LINUX.md).

> A ready-to-copy prompt for a capable coding/computer agent with
> terminal access (ChatGPT Codex, Claude Code, Cline, Cursor agent,
> OpenCode, or similar). The agent should walk the user through this
> checklist, do the work, and report exactly what it changed. The
> agent must **start by telling the user that this prompt is an
> unvalidated draft on this platform** and that the user can stop at
> any time.
>
> This file is for **macOS** (any modern Intel or Apple-silicon
> machine). If the agent detects a different OS, ask it to switch to
> `INSTALL-LINUX.md` or `INSTALL-WINDOWS.md`.

## Goal

Get **SafeAI Model Finder** installed, launching, and detecting the local
**Ollama** install — without harming any pre-existing Ollama models,
configurations, or downloaded models on this machine.

## Source of truth

- Public repository:
  `https://github.com/fabiofurlano/SafeAI-Model-Finder`

## Step 0 — Inspect before changing anything

Before touching anything, the agent must:

1. Identify the OS and architecture
   (`sw_vers`, `uname -m`). Confirm it is macOS on
   `x86_64` or `arm64` (Apple Silicon).
2. Check whether Ollama is already installed and running:

   ```bash
   command -v ollama && ollama --version
   pgrep -af 'ollama' || true
   ollama list
   ```

3. Check whether Rust / Cargo is already installed:

   ```bash
   command -v rustup; command -v cargo; command -v rustc
   ```

4. Check whether `safeai-model-finder` is already installed:

   ```bash
   command -v safeai-model-finder && safeai-model-finder --version 2>&1 || true
   ```

5. Check whether Xcode Command Line Tools are present
   (`xcode-select -p`).

Report each finding back to the user before doing anything.

## Step 1 — Ollama (prerequisite)

**Ollama is required.** SafeAI Model Finder manages models through the
user's local Ollama installation. Without Ollama, the tool launches but
shows "Ollama not running" and downloads fail with `Connection refused`.

- If Ollama is **already installed and running**: do **not** reinstall it.
  Preserve every existing model shown by `ollama list`.
- If Ollama is **not installed**: download the official
  `Ollama-darwin.zip` from <https://ollama.com/download>, or use Homebrew
  (`brew install ollama`). Explain to the user what the installer does
  before running it. Do **not** use third-party or unreviewed
  installers.
- After install, verify Ollama is healthy:

  ```bash
  ollama --version
  ollama list
  ```

  Ollama normally runs as a launch agent started by `launchctl`; if
  not running, start it (`brew services start ollama` or run
  `ollama serve &` for a foreground session).
- If Ollama is installed but **not running**, start it. Do **not**
  reinstall.

## Step 2 — Rust toolchain (via rustup)

If Cargo is already on `PATH` (Step 0), skip ahead to Step 3.

If `rustup` is missing, install it via the official installer:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

This installs `rustc`, `cargo`, and `rustup` together. **Cargo is bundled
with rustup** — there is no separate "install Cargo" step.

If `xcode-select -p` returns nothing, install the Command Line Tools
before proceeding:

```bash
xcode-select --install
```

Otherwise Rust cannot find a C linker.

## Step 3 — PATH refresh

Right after `rustup` finishes, the **current shell may not yet see
`cargo`**. This is normal. Two acceptable fixes; teach the user both:

1. **Open a new Terminal window**, OR
2. In the current shell:

   ```bash
   source "$HOME/.cargo/env"
   ```

Verify:

```bash
cargo --version
```

If `cargo --version` still returns `command not found`, do **not**
install a second Cargo toolchain. Repeat the `source` line, check
`$PATH`, and warn the user to use a fresh Terminal for future steps.

## Step 4 — Install SafeAI Model Finder

The canonical install command is:

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked
```

This fetches the source from the public GitHub repository, builds it,
and places the binary at `~/.cargo/bin/safeai-model-finder`.

If the binary already exists (the user is updating), use:

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked --force
```

`--force` overwrites the existing binary. **There is no need to
uninstall first.** Existing models, settings, and the Ollama
environment are untouched.

## Step 5 — Launch and verify

```bash
safeai-model-finder
```

Expected output:

```
Session token: …
Detecting hardware...
CPU: …
RAM: …
GPU: …
Loading model catalog...
Loaded N models

SafeAI Model Finder starting on http://127.0.0.1:<port>/
Browser opened
```

If the agent sees:

- **`Ollama not running`** → go back to Step 1 and start Ollama.
- **`Connection refused` on download** → same.
- **`Port 8787 is already in use; using local port <something>
  instead`** → fine. Use that port in the browser URL.

## Step 6 — Verify, but do not auto-download

A model download is **not** part of installation verification. If the
tool asks to download a model during verification, the agent must:

1. **Stop** before clicking anything.
2. **Ask the user** whether they want to proceed.
3. If yes, prefer a **very small** model for the test
   (e.g. `smollm2:135m` or similar < 200 MB).

## Step 7 — Report

Print back to the user:

- OS and architecture actually detected (Intel vs Apple Silicon).
- Whether Ollama was already installed / newly installed / left
  untouched; version; model count from `ollama list`.
- Whether `cargo --version` succeeded after a fresh rustup install.
- Exact command used to install / update safeai-model-finder.
- Final URL printed by `safeai-model-finder` and port that opened.
- Which model (if any) was downloaded during verification.
- Any deviation from the steps above, with reason.

## Stop conditions

Stop and ask the user before doing any of:

- Removing or resetting an existing Ollama installation.
- Deleting any model from `ollama list`.
- Replacing a system-level package silently.
- Touching firewall rules or running unsafe operations.
- Installing a Rust/Cargo toolchain from anywhere other than
  <https://rustup.rs>.
