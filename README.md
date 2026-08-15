# ManualAid

[中文](README_ZH_CN.md) / English

![Rust](https://img.shields.io/badge/rust-1.97.0+-blue.svg)
![GitHub top language](https://img.shields.io/github/languages/top/SunYanbox/ManualAid-Rust)
![GitHub License](https://img.shields.io/github/license/SunYanbox/ManualAid-Rust)
![Codecov](https://img.shields.io/codecov/c/github/SunYanbox/ManualAid-Rust)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/SunYanbox/ManualAid-Rust/.github%2Fworkflows%2Fci.yml)

A **local-only, human-in-the-loop** copypaste assistant for LLM workflows.

> **Version**: 0.4.0

## Features

- **Interactive Agent Loop** — Run `manualaid-cli` with no arguments to start an interactive session. The system builds a context-aware prompt (workspace layout, git status, enabled tools, loaded skills), you paste it into an LLM chat, then paste the LLM's tool-call response back. ManualAid parses, audits, executes the tools, and returns the results for the next turn.
- **Tool System** — Built-in tools: `Read`, `Edit`, `Write`, `Shell`, and `Skill`. Read operations are immediate; edit/write operations require explicit user approval by default (`manual` mode), with an `accept-edit` mode available for workspace-internal changes.
- **Skill System** — Skills are defined as `SKILL.md` files with YAML frontmatter and Markdown body, discovered from project and global agent directories (`.claude/`, `.ManualAid/`, etc.). Only skill names and descriptions are loaded into the system prompt; full instructions are injected only when the agent calls the Skill tool.
- **Session Logging** — Every tool call round is recorded in memory; you can copy the latest `i`-th result with built-in commands.

For detailed design and usage, see the [documentation](./doc/zh-cn/README.md).

## Installation

### Windows

**Option 1: Use the automated setup script (recommended)**

To bypass PowerShell execution policy restrictions, use one of the following methods:

- Pipe the script content to PowerShell (local clone):
  ```powershell
  Get-Content .\scripts\setup-cli.ps1 -Raw | iex
  ```

- Remote one-liner:
  ```powershell
  irm https://raw.githubusercontent.com/SunYanbox/ManualAid-Rust/main/scripts/setup-cli.ps1 | iex
  ```

To uninstall, replace `setup-cli.ps1` with `uninstall-cli.ps1` using the same method.

**Option 2: Manual installation**

1. Build locally via `cargo build --release` or download the executable from [Github Release](https://github.com/SunYanbox/ManualAid-Rust/releases)
2. Copy it to `%LOCALAPPDATA%\Programs\ManualAid\` (user-level installation) or to `C:\Program Files\ManualAid\` (system-wide installation)
3. Add the above path to the environment variable (user-level Path for user installation, system-level Path for system-wide installation)
4. Restart the terminal, then run with `manualaid-cli.exe` (in PowerShell you can usually type `manu` + Tab for auto-completion)

### Linux

**Option 1: Use the automated setup script (recommended)**

- Run the script directly (requires local clone):
  ```bash
  bash ./scripts/setup-cli.sh
  ```

- Remote one-liner:
  ```bash
  bash <(curl -fsSL https://raw.githubusercontent.com/SunYanbox/ManualAid-Rust/main/scripts/setup-cli.sh)
  ```

To uninstall, replace `setup-cli.sh` with `uninstall-cli.sh` using the same method.

**Option 2: Manual installation**

1. Build locally via `cargo build --release` or download the executable from [Github Release](https://github.com/SunYanbox/ManualAid-Rust/releases)
2. Copy it to `/usr/local/bin`; if built locally, you can also run `sudo install -m 755 target/release/manualaid-cli /usr/local/bin/` from the project root
3. Run with `manualaid-cli` (usually type `manu` + Tab for auto-completion)

### Constraints

- Before submitting any `*.rs` file, **must** pass all of the following checks:
  - `cargo fmt -- --check` — Check code style
  - `cargo clippy -- -D warnings` — Catch common errors and lint violations
  - `cargo check` — Verify compilation
  - `cargo llvm-cov` — Run code coverage analysis; in the output, the **TOTAL** line must have all three coverage metrics (Function, Line, Region) **≥80%**, unless the relevant files are difficult to improve coverage for various reasons.
- Please follow the [Rust Official Style Guide](https://doc.rust-lang.org/stable/style-guide/index.html) and the [Conventional Commits specification](https://www.conventionalcommits.org/en/v1.0.0/).

## ⚖️ Legal Disclaimer

ManualAid is a **local-only, human-in-the-loop assistant**. It is designed to
facilitate manual copypaste workflows and **does not support automated
interaction** with any LLM platform.

**Users are solely responsible for:**

1. Complying with the Terms of Service (ToS) of the LLM platforms they use.
2. Ensuring their usage does not violate rate limits, automation bans, or other
   policies.

**The author(s) of ManualAid explicitly disclaim any liability for:**

- Misuse of this tool to automate requests, bypass paywalls, or abuse LLM
  services.
- Any account suspensions, legal actions, or damages resulting from such misuse.

If you fork this project, you must retain this disclaimer and ensure your
modifications do not promote or enable automated abuse.
