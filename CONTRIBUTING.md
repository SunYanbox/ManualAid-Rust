# Contribution Guidelines

Thank you for your interest in ManualAid! These guidelines are intended to help you contribute smoothly while maintaining consistency and quality across the codebase.

## Development Environment and Tooling

The project uses the following tools consistently:

- **Rust ecosystem**: Use `cargo` for dependency management, building, testing, and documentation generation.
- **Node.js ecosystem** (if frontend parts are involved): Use `pnpm` for dependency management; please refrain from using npm or yarn.

## Development Workflow

The core development workflow is as follows; for detailed rules see `docs/issue-pr-guide.md`:

1. **Check out a new branch**: First `git fetch` to pull the latest `origin/main`, then check out a branch from the latest `main`, using `<type>/<desc>` or `<type>/<issue>-<desc>` naming (e.g., `fix/24-...`, `ci/...`).
2. **Pre-commit checks (gate for every commit)**: Before staging the files to commit, run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo check`. There is no need to additionally run `cargo fmt -- --check`.
3. **Full checks are handled by PR CI**: Cross-platform full checks (formatting, lint, docs, tests, coverage) are run automatically by PR CI (`.github/workflows/ci.yml`); you **do not** need to run the full `./scripts/ci.*` locally for every commit.
4. **Before creating a PR**: Update the CHANGELOG (rules in `docs/issue-pr-guide.md`); resolve basic test and coverage issues as much as possible (see "Testing Requirements"). When only viewing coverage info, prefer reading the cache `coverage_with_lines.txt`; do not re-run the full coverage test when the code is unchanged.
5. **After creating a PR**: Continue optimizing tests and coverage based on the PR CI results.
6. **Write the PR strictly per the template**: The PR body **must** be written per the `.github/pull_request_template.md` template, with a matching label.

## Code Style and Quality

We follow the official Rust style guide and rely on automated tools for checks:

- Before submitting any `*.rs` file, **must run before staging** `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo check` (no need to additionally run `cargo fmt -- --check`).
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

- Commit messages should follow the [Conventional Commits specification](https://www.conventionalcommits.org/en/v1.0.0/) (e.g., `feat:`, `fix:`, `docs:`). For detailed guidelines, see `docs/commit-conventions.md`.
- **The PR body must be written per the `.github/pull_request_template.md` template**, and a matching label should be added; see `docs/issue-pr-guide.md` for template notes.
- Update the CHANGELOG before creating a PR (rules in `docs/issue-pr-guide.md`); docs-only changes are exempt. In the PR, please ensure that all checks (formatting, lint, compilation, documentation, tests) have passed.

> **Tip**: Cross-platform full checks (format, lint, compile, docs, tests, and coverage) are run automatically by PR CI (`.github/workflows/ci.yml`). Before delivery you may run `./scripts/ci.*` (choose the appropriate script for your platform) for a full check; for commits, only `cargo fmt` + `cargo clippy -- -D warnings` + `cargo check` are required before staging.

## Comment Style

Comments should explain **“why”** rather than **“what”**. Feel free to use comments to clarify complex logic, mark to-do items (`TODO`/`FIXME`), or note side effects and external dependencies. At the same time:

- Avoid comments that merely repeat the code itself.
- Avoid outdated, misleading, or “diary-style” comments (e.g., author names, dates).
- Do not keep large commented-out blocks of obsolete code; rely on version control history instead.
- **Do not use decorative separator lines** (e.g., `----`, `====`, or Unicode lines).

For documentation comments (`//!` and `///`), we suggest providing concise bilingual (Chinese/English) descriptions; detailed explanations can use the `# Description` / `# 描述` structure.
