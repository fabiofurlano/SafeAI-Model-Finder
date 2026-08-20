# Install SafeAI Model Finder on Windows — with an AI agent

> **Project status: PROVEN on Windows.** This install path has been
> validated end-to-end on a real fresh Windows machine (Vagon):
> Rust/rustup installed from zero including the Visual C++
> prerequisites, SafeAI Model Finder compiled and installed with the
> public `cargo install` command, the browser UI launched, Ollama was
> detected, and a small model (SmolLM2 135M) was downloaded and marked
> ready through the UI. The macOS path
> ([`INSTALL-MACOS.md`](INSTALL-MACOS.md)) remains an unvalidated
> draft.

> A ready-to-copy prompt for a capable coding/computer agent with
> terminal access (ChatGPT Codex, Claude Code, Cline, Cursor agent,
> OpenCode, or similar). The agent should walk the user through this
> checklist, do the work, and report exactly what it changed. The
> user can stop at any time.
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
  `OllamaSetup.exe` from <https://ollama.com/download>. **Explain**
  to the user what the installer does before running it; it registers a
  background Windows service that starts Ollama at login. For the
  local/private SafeAI workflow:
  - leave **"Expose Ollama to the network" OFF** — it is not needed;
  - an Ollama account / sign-in is **not** required;
  - cloud models are **not** required;
  - the default model location is fine.
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

Otherwise, download and run `rustup-init.exe` from
<https://rustup.rs> — the standard installer installs `rustc`,
`cargo`, and `rustup` together. **Cargo is bundled with rustup.**

**If Rust reports missing Visual C++ prerequisites** and presents:

```
1) Quick install via the Visual Studio Community installer
2) Manually install the prerequisites
3) Don't install the prerequisites
```

choose **option 1** — the simplest path, and the one actually tested
on a fresh Windows machine. Explain to the user explicitly:

- this is **NOT VS Code**;
- it installs Microsoft's compiler/linker plus the Windows SDK
  prerequisites that Rust needs;
- this first-time prerequisite step may take **noticeably longer
  than installing Model Finder itself**.

**If Windows recommends a restart** after the Microsoft
prerequisites: restart. Then run `rustup-init.exe` **again** if Rust
itself did not finish installing. Use the default Rust installation.

During rustup installation, the agent must confirm the host triple is
either `x86_64-pc-windows-msvc` or `aarch64-pc-windows-msvc`.

## Step 3 — New terminal and verification

Open a **new** Command Prompt / PowerShell (the old one will not see
the new tools). Verify:

```powershell
rustc --version
cargo --version
```

rustup normally registers `%USERPROFILE%\.cargo\bin` on the user
`PATH` for new sessions. Do **not** tell normal users to manipulate
`PATH` manually unless these commands still fail in a fresh
terminal.

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

- **`Ollama not running`** → go back to Step 1, install or start
  Ollama (network exposure stays OFF for the local workflow), then
  refresh or restart Model Finder.
- **`Connection refused` on download** → same. Ollama is not
  reachable.
- **`Port 8787 is already in use; using local port <something>
  instead`** → fine. The browser URL will show the chosen port.
- **Compilation warnings from `cargo install`** → not a failure.
  The install succeeded if Cargo ends with lines like
  `Finished release profile` and
  `Installed package safeai-model-finder`.

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

## Windows troubleshooting (real issues seen in the field)

**A. `rustc`/`cargo` not recognized immediately after the Visual C++
prerequisite install.**
Rust itself may not have finished installing — the Microsoft
prerequisites are a separate first-time step. If Windows recommended
a restart, restart, then run `rustup-init.exe` again. Open a **new**
Command Prompt / PowerShell and verify `rustc --version` /
`cargo --version`.

**B. rustup shows the Visual C++ prerequisite prompt.**
Option 1 ("Quick install via the Visual Studio Community installer")
is the tested simple path. It is not VS Code; it installs the
Microsoft compiler/linker and Windows SDK Rust needs.

**C. "Ollama not running".**
Install or start Ollama. Leave "Expose Ollama to the network" OFF
for the local/private workflow. Refresh or restart Model Finder
afterwards; it will then detect Ollama and the installed-model
count.

**D. `cargo install` prints compilation warnings.**
Warnings are not installation failure. Success is determined by
Cargo's final lines: `Finished release profile` and
`Installed package safeai-model-finder`.

## Stop conditions

Stop and ask the user before doing any of:

- Removing or resetting an existing Ollama installation.
- Deleting any model from `ollama list`.
- Replacing a system-level package silently.
- Touching system firewall rules or running unsafe operations.
- Installing a Rust/Cargo toolchain from anywhere other than
  <https://rustup.rs>.
