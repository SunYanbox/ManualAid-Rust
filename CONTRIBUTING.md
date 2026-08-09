# Contribution Guidelines

Thank you for your interest in ManualAid! These guidelines are intended to help you contribute smoothly while maintaining consistency and quality across the codebase.

## Development Environment and Tooling

The project uses the following tools consistently:

- **Rust ecosystem**: Use `cargo` for dependency management, building, testing, and documentation generation.
- **Node.js ecosystem** (if frontend parts are involved): Use `pnpm` for dependency management; please refrain from using npm or yarn.

## Code Style and Quality

We follow the official Rust style guide and rely on automated tools for checks:

- Before submitting, it is recommended to run `cargo fmt -- --check` to verify formatting, or simply use `cargo fmt` to auto-format.
- Use `cargo clippy -- -D warnings` to catch common errors and lint violations.
- Use `cargo check` to verify that the code compiles.
- Ensure that documentation builds successfully: `cargo doc --no-deps`.

## Testing Requirements

### Test File Organization

Tests for public APIs should be placed in the corresponding integration test files under the `tests` directory, rather than mixing large amounts of test implementation with source code in the same file. You can use `metron --per-file crates` to view the proportions of code, comments, and tests per file, which helps evaluate test distribution.

### Coverage Targets

We encourage maintaining good test coverage. Use `cargo llvm-cov --show-missing-lines` to check coverage of the `src` directory:

- Test coverage for individual files is suggested to be **no less than 85%**, and for core modules **no less than 95%**.
- Overall coverage (Function, Line, Region) is suggested to be **no less than 80%**.

Some files are temporarily excluded from coverage requirements due to external dependencies or special reasons; these are noted in code comments (e.g., `user_dir.rs`, `clipboard.rs`, `init.rs`).

### stdout Test Handling

For tests that print to standard output, please redirect the output to an internal string variable before making assertions, to avoid polluting the terminal output. Also ensure that the approach works correctly in both sandboxed environments and real terminals.

## Dependency Management

When adding a new crate or feature, use `cargo add xxx` **without specifying a version number**, so that the latest compatible version is always used.

## Commits and Pull Requests

- Commit messages should follow the [Conventional Commits specification](https://www.conventionalcommits.org/en/v1.0.0/) (e.g., `feat:`, `fix:`, `docs:`).
- PR descriptions should be objective, describing what changed and its impact, without inferring the intent behind the code.
- In the PR, please ensure that all checks (formatting, lint, compilation, documentation, tests) have passed.

## Comment Style

Comments should explain **“why”** rather than **“what”**. Feel free to use comments to clarify complex logic, mark to-do items (`TODO`/`FIXME`), or note side effects and external dependencies. At the same time:

- Avoid comments that merely repeat the code itself.
- Avoid outdated, misleading, or “diary-style” comments (e.g., author names, dates).
- Do not keep large commented-out blocks of obsolete code; rely on version control history instead.
- **Do not use decorative separator lines** (e.g., `----`, `====`, or Unicode lines).

For documentation comments (`//!` and `///`), we suggest providing concise bilingual (Chinese/English) descriptions; detailed explanations can use the `# Description` / `# 描述` structure.
