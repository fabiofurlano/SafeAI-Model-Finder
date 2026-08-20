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

## Quick start

You need two things: **Ollama** (already installed and running) and
**SafeAI Model Finder** itself.

1. **Make sure Ollama is installed and running.**
   Download it from <https://ollama.com/download> for your platform. SafeAI
   Model Finder manages models through your local Ollama install — if
   Ollama is missing or stopped, the tool will show "Ollama not running"
   and downloads will fail with `Connection refused`. We do not bundle
   or replace it; whatever models you have today stay where they are.

2. **Install SafeAI Model Finder from the terminal** (one line):

   ```bash
   cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked
   ```

3. **Start it**:

   ```bash
   safeai-model-finder
   ```

   A browser tab opens at `http://127.0.0.1:8787/` (or another free
   loopback port if 8787 is busy), the interface detects your hardware,
   shows the models already in your Ollama, and lets you find, browse,
   and download new ones.

If terminal setup is not your thing, jump to
[Install with an AI agent](#install-with-an-ai-agent); it gives a
ready-to-copy installation prompt for ChatGPT Codex, Claude Code,
Cline, Cursor agent, OpenCode, or any other capable coding/computer
agent with terminal access.

If something doesn't work, see [Troubleshooting](#troubleshooting).

## Requirements

SafeAI Model Finder is installed and started from a terminal. You need:

**To install the tool (one-time)**

- The **Rust** toolchain, installed via **rustup** from
  <https://rustup.rs>. `rustup` installs `rustc`, `cargo`, **and**
  `rustup` together — Cargo is not a separate prerequisite. After
  rustup finishes, open a **new terminal** or run
  `source "$HOME/.cargo/env"` so `cargo --version` works in the
  current shell. (This is the most-missed step on a fresh machine.)
- **Rust 1.95 or newer** (`rustc --version`).
- The install path below has been **proven end-to-end on Linux and
  on Windows** (a real fresh Windows machine). The macOS prompt in
  [`docs/agent-install/`](docs/agent-install/) remains a best-effort
  draft pending independent validation.
- A standard C toolchain — Rust needs a linker to compile native
  dependencies:
  - **Linux:** most distributions ship `gcc` / `cc` out of the box.
  - **macOS:** install Xcode Command Line Tools
    (`xcode-select --install`).
  - **Windows:** if Rust's installer reports missing Visual C++
    prerequisites, choose its option
    **"Quick install via the Visual Studio Community installer"** —
    this is the tested path. It is **not VS Code**; it installs
    Microsoft's compiler/linker plus the Windows SDK that Rust
    needs. This one-time prerequisite step can take noticeably
    longer than installing Model Finder itself. When the Microsoft
    installer finishes, return to rustup and continue the Rust
    installation; a restart is only needed if Rust cannot continue
    or the tools don't work afterwards.
- Network access during installation so Cargo can download the
  published Rust dependencies. No source code or system data is sent
  out.

**To start SafeAI Model Finder**

- The installed binary: `safeai-model-finder` (lands in Cargo's bin
  directory — `~/.cargo/bin` on Linux/macOS,
  `%USERPROFILE%\.cargo\bin` on Windows — after the install step
  below).
- A web browser on the same machine (Chrome, Firefox, Edge, Safari —
  any modern browser). The browser is the interface only; no data
  leaves the loopback connection.

**To manage and run local models**

- An installed and running **Ollama** on the same machine. Download
  from <https://ollama.com/download>. Network access is needed when
  *you* initiate a model download through Ollama; SafeAI Model
  Finder itself does not initiate any download without your explicit
  confirmation.

## Install from source

This is the path most users will take — installing SafeAI Model Finder
straight from the public GitHub repository into Cargo's bin directory
(`~/.cargo/bin` on Linux/macOS, `%USERPROFILE%\.cargo\bin` on Windows).

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked
```

The `--locked` flag pins every dependency to the exact versions
recorded in the in-repo `Cargo.lock` for a reproducible, deterministic
install. The install places a single binary named `safeai-model-finder`
in Cargo's bin directory (`safeai-model-finder.exe` on Windows).

**On a freshly rustup-installed shell**, first run:

```bash
source "$HOME/.cargo/env"
```

or simply open a new terminal window, then re-run the `cargo install`
command.

To uninstall:

```bash
cargo uninstall safeai-model-finder
```

---

## Install with an AI agent

If setting up a terminal tool from scratch is not your thing, you can
hand the installation to a capable coding/computer agent with terminal
access (ChatGPT Codex, Claude Code, Cline, Cursor agent, OpenCode, or
similar) by copying one of the prompts below and pasting it into the
agent. The prompts tell the agent exactly what to check, what to
install, what **not** to touch (your existing Ollama models, your
existing Ollama install, your existing toolchains), and how to verify
that everything works at the end.

**Platform support status** (what has actually been verified
end-to-end on this project):

- **Linux** — *proven*. The Linux install path has been validated
  end-to-end on a fresh machine (Vast.ai KDE VM, RTX 3060) plus the
  maintainer's Linux development host.
- **macOS** — *not yet validated*. The SafeAI Model Finder source
  has `cfg(target_os = "macos")` blocks and the underlying
  hardware-detection library runs on macOS, but the public
  `cargo install --git …` install path has not been proven
  end-to-end on a Mac yet. The macOS prompt below is provided as
  an **informational draft** so a capable agent can attempt it
  with you; treat its output as best-effort until a Mac owner
  runs it on a clean machine and reports back.
- **Windows** — *proven*. The public install path
  (`cargo install --git … --locked`) has been validated end-to-end
  on a real fresh Windows machine (Vagon): Rust/rustup installed
  from zero including the Visual C++ prerequisites, Model Finder
  compiled and installed, the browser UI launched, Ollama was
  detected, and a small model (SmolLM2 135M) was downloaded and
  marked ready through the UI.

If you only have macOS available today and are not comfortable
running an unproven install recipe through an AI agent, the
safest path is to use a Linux machine (or VM) instead.

Pick the prompt for your operating system:

- [Linux](docs/agent-install/INSTALL-LINUX.md) — proven end-to-end
  on a fresh Linux install (Vast.ai KDE VM, RTX 3060).
- [macOS](docs/agent-install/INSTALL-MACOS.md) — _informational
  draft; not yet validated on a Mac by this project._
- [Windows](docs/agent-install/INSTALL-WINDOWS.md) — proven
  end-to-end on a real fresh Windows machine (Vagon).

If something is misbehaving and you would rather not troubleshoot by
hand, copy the troubleshooting prompt instead:

- [Troubleshooting](docs/agent-install/TROUBLESHOOT.md)

The agent must always ask you before deleting models, reinstalling
Ollama, or replacing an existing toolchain.

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
- `--force` overwrites the existing installed `safeai-model-finder`
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

## Troubleshooting

If something doesn't work, walk through this short list in order. Each
entry tells you what to check, what almost always fixes it, and where to
get help.

**`cargo: command not found` after installing rustup.**
This is the most common mistake on a fresh machine. `rustup` puts
Cargo in Cargo's bin directory, but the current shell may not have
that directory in `PATH` yet. Open a new terminal, or in the current
shell run:
```bash
source "$HOME/.cargo/env"
cargo --version
```

**On Windows: `rustc`/`cargo` not recognized after installing Rust.**
Open a **new** Command Prompt / PowerShell and verify with
`rustc --version` and `cargo --version`. If they still fail, re-run
`rustup-init.exe`. Restart Windows only if Rust genuinely cannot
continue or the tools remain unavailable — the Microsoft installer
may *recommend* a restart, but in the tested clean install Rust
completed without one.

**On Windows: rustup asks about missing Visual C++ prerequisites.**
Choose option 1, "Quick install via the Visual Studio Community
installer" — the tested path. It is not VS Code; it installs the
Microsoft compiler/linker and Windows SDK Rust needs, and it can take
noticeably longer than installing Model Finder itself.

**`cargo install` prints compilation warnings.**
Warnings are not installation failures. The install succeeded only if
Cargo ends with lines like `Finished release profile` and
`Installed package safeai-model-finder`.

**SafeAI Model Finder shows "Ollama not running".**
Install Ollama from <https://ollama.com/download> if it isn't there, or
start the service if it's installed but stopped. For the local/private
SafeAI workflow, leave Ollama's "Expose to the network" option OFF —
it is not needed. Once Ollama is responding on the loopback, refresh
or restart Model Finder and it will be detected.

**Model can be listed in Ollama but Model Finder shows a readiness-test
timeout.**
Verify the model is actually usable first:
```bash
ollama list
ollama run <model-name> "hello"
```
If both succeed, the model is healthy and the timeout was a Model
Finder probe miss — no need to re-download. Don't redownload unless
the model is genuinely missing from `ollama list`.

**Browser shows `Connection refused` when starting Model Finder.**
Make sure Ollama is running and reachable on `127.0.0.1:11434`. The
`Connection refused` error in Model Finder normally means Ollama isn't
there yet.

**GPU isn't detected on Linux.**
Check that the official Ollama installer (which sets up the bundled
GPU runtime) ran successfully. If you installed Ollama from a
distribution package, GPU support may be missing.

**`cargo install` failed partway through.**
Re-run the same command. `cargo install` is resumable: only the
unbuilt crates will be compiled next time.

If you would rather not troubleshoot by hand, copy the
[Troubleshooting prompt](docs/agent-install/TROUBLESHOOT.md) into a
coding/computer agent and let it drive the diagnosis.

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
