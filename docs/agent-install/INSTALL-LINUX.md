# Install SafeAI Model Finder on Linux — with an AI agent

> A ready-to-copy prompt for a capable coding/computer agent with terminal
> access (ChatGPT Codex, Claude Code, Cline, Cursor agent, OpenCode, or
> similar). The agent should walk the user through this checklist, do the
> work, and report exactly what it changed.
>
> This file is for **Linux** (any modern desktop distribution: Ubuntu,
> Debian, Fedora, Arch, …). If the agent detects a different OS, ask it to
> switch to `INSTALL-MACOS.md` or `INSTALL-WINDOWS.md`.

## Goal

Get **SafeAI Model Finder** installed, launching, and detecting the local
**Ollama** install — without harming any pre-existing Ollama models,
configurations, or downloaded models on this machine.

## Source of truth

- Public repository: `https://github.com/fabiofurlano/SafeAI-Model-Finder`
- Already-proven fresh-machine evidence: a clean Linux install on a Vast.ai
  RTX 3060 KDE VM. The same workflow is reproducible from this file.

## Step 0 — Inspect before changing anything

Before touching anything, the agent must:

1. Identify the OS and architecture (`uname -a`, `cat /etc/os-release`).
   Confirm it is Linux.
2. Check whether Ollama is already installed and running:

   ```bash
   command -v ollama && ollama --version
   systemctl --user is-active ollama 2>/dev/null || pgrep -af 'ollama' || true
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

5. Check whether the current shell session already has `~/.cargo/bin` on
   `PATH`:

   ```bash
   echo "$PATH" | tr ':' '\n' | grep -F "$HOME/.cargo/bin" || echo "(not on PATH)"
   ```

Report each finding back to the user before doing anything.

## Step 1 — Ollama (prerequisite)

**Ollama is required.** SafeAI Model Finder manages models through the
user's local Ollama installation. Without Ollama, the tool launches but
shows "Ollama not running" and downloads fail with `Connection refused`.

- If Ollama is **already installed and running**: do **not** reinstall it.
  Preserve every existing model shown by `ollama list`.
- If Ollama is **not installed**: install it through the official,
  currently-supported method. As of 2026, that is:

  ```bash
  curl -fsSL https://ollama.com/install.sh | sh
  ```

  Explain to the user what this script does before running it. It may
  require `sudo` and creates a `systemd` service; that is expected on
  modern Linux. Do **not** use third-party or unreviewed installers.
- After install, **verify Ollama is healthy**:

  ```bash
  ollama --version
  ollama list
  ollama serve --help   # confirms the daemon is reachable
  ```

  If the systemd service is enabled, it will auto-start at login. If
  not, start it (`systemctl --user start ollama` or `ollama serve &`).

- If Ollama is installed but **not running**, start it. Do **not**
  reinstall.

## Step 2 — Rust toolchain (via rustup)

If Rust / Cargo is already installed (Step 0), skip ahead to Step 3.

If `rustup` is missing, install it via the official installer:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

This installs `rustc`, `cargo`, and `rustup` together. **Cargo is bundled
with rustup** — there is no separate "install Cargo" step.

## Step 3 — PATH refresh (the most-missed Linux gotcha)

Right after `rustup` finishes, the **current shell may not yet see
`cargo`**. This is normal. Two acceptable fixes; teach the user both:

1. **Open a new terminal**, OR
2. In the current shell:

   ```bash
   source "$HOME/.cargo/env"
   ```

After the fix, verify:

```bash
cargo --version
```

If `cargo --version` still returns `command not found`, do **not**
install a second Cargo toolchain. Repeat the `source` line, check
`$PATH`, and warn the user to use a fresh shell for future steps.

## Step 4 — Install SafeAI Model Finder

The canonical, already-proven terminal-install command is:

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked
```

This fetches the source from the public GitHub repository, builds it,
and places the binary at `~/.cargo/bin/safeai-model-finder`. The
`--locked` flag pins every dependency to the exact versions in the
`Cargo.lock` for a reproducible, deterministic install.

If the binary already exists (the user is updating), use:

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked --force
```

`--force` overwrites the existing binary. **There is no need to
uninstall first.** Existing models, settings, and the Ollama
environment are untouched.

The install needs network access (Cargo fetches Rust dependencies). No
user data is sent out.

## Step 5 — Launch and verify

```bash
safeai-model-finder
```

Expected output, in order:

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

- **`Ollama not running`** → go back to Step 1 and start Ollama. Do
  not skip this.
- **`Connection refused` on download** → same. Ollama is not
  reachable.
- **`Port 8787 is already in use; using local port <something>
  instead`** → totally fine. The tool picked a free loopback port.
  The browser URL will show the chosen port. Validate the UI works
  at that URL, not at 8787.

## Step 6 — Verify, but do not auto-download

A model download is **not** part of installation verification. If the
tool asks to download a model during verification, the agent must:

1. **Stop** before clicking anything.
2. **Ask the user** whether they want to proceed.
3. If yes, prefer a **very small** model for the test
   (e.g. `smollm2:135m` or similar < 200 MB) so verification does not
   consume tens of GB of network/disk.

After the user confirms, follow through; report the chosen model
name and size back to the user.

## Step 7 — Report

Print back to the user:

- OS and architecture actually detected.
- Whether Ollama was already installed / newly installed / left
  untouched; version;
  model count from `ollama list`.
- Whether `cargo --version` succeeded after a fresh rustup install
  without reopening the shell; remind the user to open a new terminal
  going forward.
- Exact command used to install / update safeai-model-finder.
- Final URL printed by `safeai-model-finder` and port that opened.
- Which model (if any) was downloaded during verification, and the
  approximate size.
- Any deviation from the steps above, with reason.

## Stop conditions

Stop and ask the user before doing any of:

- Removing or resetting an existing Ollama installation.
- Deleting any model from `ollama list`.
- Replacing a system-level package silently.
- Touching system firewall rules or running unsafe operations.
- Installing a Rust/Cargo toolchain from anywhere other than
  <https://rustup.rs>.
