# Install SafeAI Model Finder on Windows — with an AI agent

> **Project status: NOT YET VALIDATED on Windows.** This file is an
> **informational draft**. The Linux install path
> ([`INSTALL-LINUX.md`](INSTALL-LINUX.md)) is the only install path
> proven end-to-end by this project. The Windows path described
> below has not been independently recorded as a clean-machine
> success by a Windows owner. The agent and the user must understand
> that going through this prompt on Windows is best-effort, not a
> supported path, until a Windows maintainer does that clean-machine
> proof and reports back.
>
> If you only have Windows available, the **safest option** is to
> use a Linux VM (Ubuntu / Fedora under WSL2 or VirtualBox) instead
> and follow [`INSTALL-LINUX.md`](INSTALL-LINUX.md).

> A ready-to-copy prompt for a capable coding/computer agent with
> terminal access (ChatGPT Codex, Claude Code, Cline, Cursor agent,
> OpenCode, or similar). The agent should walk the user through this
> checklist, do the work, and report exactly what it changed. The
> agent must **start by telling the user that this prompt is an
> unvalidated draft on this platform** and that the user can stop at
> any time.
>
> This file is for **Windows 10 / 11**. If the agent detects a
> different OS, ask it to switch to `INSTALL-LINUX.md` or
> `INSTALL-MACOS.md`.

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
   (`[System.Environment]::OSVersion`, `Get-CimInstance Win32_Processor`,
   `Architecture`).
   Confirm it is Windows 10 or 11.
2. Check whether Ollama is already installed and reachable:

   ```powershell
   Get-Command ollama -ErrorAction SilentlyContinue | Format-List Source, Version
   & ollama list
   ```

3. Check whether Rust / Cargo is already installed:

   ```powershell
   Get-Command cargo, rustup -ErrorAction SilentlyContinue
   ```

4. Check whether `safeai-model-finder` is already installed:

   ```powershell
   Get-Command safeai-model-finder -ErrorAction SilentlyContinue
   ```

5. Verify that the MSVC build tools are present (Windows needs C and C++
   tooling for Rust to compile native dependencies):

   ```powershell
   Get-Command cl.exe -ErrorAction SilentlyContinue
   ```

   If `cl.exe` is missing, the recommended fix is to install the
   **Microsoft Visual C++ Build Tools** via Visual Studio's
   "Desktop development with C++" workload.

Report each finding back to the user before doing anything.

## Step 1 — Ollama (prerequisite)

**Ollama is required.** SafeAI Model Finder manages models through the
user's local Ollama installation. Without Ollama, the tool launches but
shows "Ollama not running" and downloads fail with `Connection refused`.

- If Ollama is **already installed and running**: do **not** reinstall it.
  Preserve every existing model shown by `ollama list`.
- If Ollama is **not installed**: download and run the official
  `OllamaSetup.exe` from <https://ollama.com/download>. **Explain** to
  the user what the installer does before running it; it registers a
  background Windows service that starts Ollama at login.
- After install, verify Ollama is healthy:

  ```powershell
  & ollama --version
  & ollama list
  ```

- If Ollama is installed but **not running**, start the Ollama service
  (Settings → Services, or `Start-Service ollama` from an elevated
  PowerShell). Do **not** reinstall.

## Step 2 — Rust toolchain (via rustup)

If Cargo is already on `PATH` (Step 0), skip ahead to Step 3.

Otherwise, install rustup for Windows from
<https://rustup.rs> — the standard installer
(`rustup-init.exe`) installs `rustc`, `cargo`, and `rustup` together.
**Cargo is bundled with rustup.**

During rustup installation, the agent must confirm the host triple is
either `x86_64-pc-windows-msvc` or `aarch64-pc-windows-msvc` (the
Visual Studio C++ Build Tools must already be present for the chosen
triple).

## Step 3 — PATH refresh

rustup-init.exe normally registers `~/.cargo/bin` on the user `PATH`
for **new** PowerShell / Command Prompt sessions. In an already-open
shell after install:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

Verify:

```powershell
cargo --version
```

If this returns `cargo: command not found`, do **not** install a second
Cargo toolchain. Open a new shell, or re-run the `Path` assignment
above.

## Step 4 — Install SafeAI Model Finder

The canonical, already-proven customer-install command is:

```powershell
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked
```

This fetches the source from the public GitHub repository, builds it,
and places the binary at `%USERPROFILE%\.cargo\bin\safeai-model-finder.exe`.

If the binary already exists (the user is updating), use:

```powershell
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked --force
```

`--force` overwrites the existing binary. **There is no need to
uninstall first.** Existing models, settings, and the Ollama
environment are untouched.

## Step 5 — Launch and verify

```powershell
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

- **`Ollama not running`** → go back to Step 1 and start Ollama.
- **`Connection refused` on download** → same. Ollama is not
  reachable.
- **`Port 8787 is already in use; using local port <something>
  instead`** → fine. The browser URL will show the chosen port.

## Step 6 — Verify, but do not auto-download

A model download is **not** part of installation verification. If the
tool asks to download a model during verification, the agent must:

1. **Stop** before clicking anything.
2. **Ask the user** whether they want to proceed.
3. If yes, prefer a **very small** model for the test
   (e.g. `smollm2:135m` or similar < 200 MB) so verification does not
   consume tens of GB of network/disk.

## Step 7 — Report

Print back to the user:

- OS and architecture actually detected.
- Whether Ollama was already installed / newly installed / left
  untouched; version; model count from `ollama list`.
- Whether `cargo --version` succeeded after a fresh rustup install;
  remind the user to open a new PowerShell session going forward.
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
