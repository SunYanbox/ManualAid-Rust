# ManualAid

[中文](README_ZH_CN.md) / English

![Rust](https://img.shields.io/badge/rust-1.97.0+-blue.svg)
![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)

A **local-only, human-in-the-loop** copypaste assistant for LLM workflows.

> **Version**: 0.1.0 | **Rust**: >=1.97.0

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
