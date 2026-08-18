# Troubleshoot SafeAI Model Finder — with an AI agent

> A short, copy-paste prompt for a capable coding/computer agent with
> terminal access (ChatGPT Codex, Claude Code, Cline, Cursor agent,
> OpenCode, or similar). Use this when an installation went wrong, or
> when the tool is launching but something downstream is failing.

The agent's job in this prompt is to **diagnose**, **summarise**, and
**suggest a fix**. It must **not** silently delete Ollama models, blow
away a working Ollama install, or trigger a multi-GiB re-download
unless the user explicitly asks.

## Step 0 — Gather facts first

Before suggesting any fix, capture:

```bash
uname -a                                    # OS + arch
command -v ollama; ollama --version         # Ollama state
ollama list                                  # Models already on disk
pgrep -af 'ollama' || true                   # Is the daemon running?
command -v cargo; cargo --version            # Rust toolchain state
command -v safeai-model-finder               # Binary present?
echo "$PATH" | tr ':' '\n' | grep -F "$HOME/.cargo/bin" || echo "(no ~/.cargo/bin on PATH)"
```

If SafeAI Model Finder was just launched, also report the last
**20–30 lines of its terminal output** (including the URL it served).

## Symptom → check → fix

### 1. `cargo: command not found` (right after rustup)

This is a known shell refresh hazard. **Do not** install a second
Cargo. Try:

```bash
source "$HOME/.cargo/env"
cargo --version
```

If still missing, open a new terminal. If neither works, check
`$HOME/.cargo/bin` exists and is a directory with `cargo` inside it.

### 2. `rustup` installer asked for a default toolchain

The safe default is `stable`. Press Enter. Custom toolchain selection
is not recommended for this project.

### 3. `Ollama not running`

Verify install:

```bash
command -v ollama && ollama --version
ollama list
```

If Ollama is installed but not running, start it:

- Linux (systemd / user service):

  ```bash
  systemctl --user start ollama
  # or, foreground:
  ollama serve
  ```

- macOS (Homebrew or LaunchAgent):

  ```bash
  brew services start ollama
  # or, foreground:
  ollama serve
  ```

- Windows: Start the **Ollama** service from Services (or run
  `ollama serve` in a separate elevated shell).

If Ollama is **not** installed, return to the platform-specific
INSTALL prompt (`INSTALL-LINUX.md` / `INSTALL-MACOS.md` /
`INSTALL-WINDOWS.md`).

### 4. Download returns `Connection refused`

Same root cause as #3: Ollama daemon is not reachable on loopback.
Fix Ollama first; the download will then work.

### 5. `Port 8787 is already in use; using local port …`

This is **not** an error. SafeAI Model Finder detected another
service on its default port and picked a free loopback port. Open
the URL it actually printed (the chosen port) in the browser. Do not
try to free port 8787 unless the user wants to use that exact port for
another reason.

### 6. Readiness test timeout after a download

**This case must NOT auto-redownload.** A readiness timeout has been
observed even when the model is fully installed and usable.

Steps to confirm the model is actually there:

1. List models in Ollama:

   ```bash
   ollama list
   ```

   The expected model name should appear.

2. Check that the model responds directly through Ollama:

   ```bash
   ollama run <model-name> "Say 'ok' and nothing else."
   ```

   If Ollama can run it, the model is installed and works.

3. If Ollama can run the model, the timeout was a SafeAI Model Finder
   readiness-test artefact, **not** a missing model. **Do NOT
   redownload.** Report this to the user and let them choose between:

   - accepting the model as installed;
   - explicitly choosing **Remove** in SafeAI Model Finder if they no
     longer want it;
   - restarting `safeai-model-finder` and trying the readiness test
     again.

4. If Ollama **cannot** run the model (broken download, partial
   architecture mismatch, etc.), then it is safe to delete and
   re-download. Ask the user before re-downloading.

The goal is to **avoid duplicate multi-GB downloads** triggered by
spurious timeouts.

### 7. GPU not detected (or video drivers missing)

Run:

```bash
safeai-model-finder            # and capture the hardware-detection output
```

If GPU is reported as `unknown` or missing:

- Verify the proprietary driver is installed for the GPU family
  (NVIDIA / AMD). On Linux, the open `nouveau` and `amdgpu` drivers
  are detected but are often slower and may be reported with limited
  detail.
- For NVIDIA on Linux, the CUDA driver and `nvidia-smi` matter for
  GPU-accelerated inference in Ollama. **Ollama itself** must be
  installed using the **same** official installer as Step 1 of the
  platform INSTALL prompt; that installer is what sets up GPU
  detection inside Ollama.

If Ollama can detect the GPU but SafeAI Model Finder cannot, report
both outputs to the user so the gap can be diagnosed.

### 8. Install command / build failure

Common causes:

- **Network blocked** → Cargo could not fetch dependencies. Verify
  network access to `crates.io`.
- **`rustc` is older than 1.95** → `cargo install …` may degrade. Run
  `rustup update` or `rustup default stable` and retry.
- **Missing C linker** (Linux / macOS) → install Xcode Command Line
  Tools (`xcode-select --install`) on macOS, or a stock `gcc` on
  Linux.
- **Missing MSVC tools** (Windows) → install Visual Studio Build
  Tools with the "Desktop development with C++" workload.

Capture the full error message before retrying. Do not retry blindly.

## Report

Summarise, in plain language:

- Which symptom was investigated.
- The exact commands run, in order.
- The current state of: Ollama, Rust/Cargo, SafeAI Model Finder
  binary, models on disk, current GPU detection.
- Whether any **destructive** action (reinstall, model deletion,
  download) was taken; if so, why and with whose confirmation.
- Suggested next step.

The agent must finish by **asking the user** to confirm before any
non-read-only action — even when the action seems obvious.

## Stop conditions

Stop and ask the user before doing any of:

- Removing or resetting an existing Ollama installation.
- Deleting any model from `ollama list`.
- Triggering a re-download of a model that is already installed.
- Replacing a system-level package silently.
- Touching firewall rules or running unsafe operations.
