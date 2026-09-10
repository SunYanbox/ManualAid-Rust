# ChangeLog (Developer Edition)

This file records the complete technical changes relative to the previous release, aimed at developers; for the user-facing concise edition, see `docs/changelog/CHANGELOG.md`.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and follows Semantic Versioning.

## [Unreleased]

### Added

- Execution results of user-driven operations such as `!` shell commands, `/SKILL` skill loading and `@path` file/folder references are now uniformly wrapped as `[USER_ACTION kind="exec|skill|path" …] … [END USER_ACTION]` and posted back to the external LLM: the header attributes take `command`/`name`/`path` per kind, with labels being the full user command, the skill's exposed name and the absolute path (directories get a trailing `\` marker); regular `[TOOL_RESULT]` rendering is byte-for-byte unchanged, and the truncation and full-output staging pipelines are reused uniformly for both wrapper kinds.
- `ToolResult` gains an optional `user_action` field (`serde(default, skip_serializing_if)`) and a chained `with_user_action(kind, label)` constructor; `UserActionKind::as_str/attr_name` centralizes the mapping between kind wire names and label attribute names for later `/SKILL` and `@path` integration; old session JSON loads as `None` by default, and the field is omitted when serializing with no value.
- The system prompt (both English and Chinese) adds a `user-action-note` after the system-reminder explanation, explaining the meaning of the USER_ACTION block to the external LLM and telling it not to repeat user operations.
- The main menu input gains interactive completion: typing `/` at line start shows built-in commands and enabled SKILLs (full unique names), typing `@` at line start or after a space shows project paths; ↑/↓ to select, Tab to fill, Enter to confirm, Esc to close suggestions; `/SKILL` directly loads a skill and `@path` reads a file or lists a folder (directories use tree rendering), each producing a `[USER_ACTION kind="skill"|"path"]` result; non-interactive and test builds automatically fall back to scripted input, with zero intrusion into existing test mechanisms.
- The completion editor gains a shared session input history `InputHistory`: a `Mutex`-protected entry list shared across editing rounds, where `push` drops earlier identical entries so the same input keeps only the newest one and blank lines are ignored; `mod.rs` creates one instance per session and passes it via `Arc` to `read_line_with_completion`, and both the interactive submit path and the non-interactive `read_line` fallback record input.
- The completion editor gains arrow-key history recall: with no suggestion panel, ↑ recalls an earlier history entry and ↓ a newer one; the first recall saves the current buffer as a draft, and moving past the newest entry restores the draft; with no history the up/down keys are no-ops.
- The completion editor cursor is upgraded to character-boundary byte indices: `Char` inserts at the cursor, `Backspace` deletes the character before the cursor, Left/Right move one character, Ctrl+Left/Right move by word; the crossterm key conversion maps plain arrow keys, Ctrl-modified arrow keys and the Emacs aliases C-b/C-f; cursor movement triggers candidate refresh because the active completion token now ends at the cursor.
- Completion rendering places the cursor at the prompt width plus the display width of the text before the cursor, keeping it aligned for CJK characters; after Tab fills a candidate the cursor stays after the inserted content instead of jumping to the end of the buffer.
- Added `loop_cli/progress.rs`: a single-line progress display `(Ns) TOOL1, TOOL2, ...` during tool execution, colored by state (not started=grey, running=yellow, success=green, failure=red), using `tokio::select!` to race the execution future against a 200ms ticker to keep refreshing the seconds; `ProgressLine` is entirely disabled under test builds, console capture or non-terminal stdout, and `suspend`/`finish` support erasing and redrawing during mid-way paged diffs and freezing with a newline when execution ends; rendering is extracted into a pure function for unit testing.

### Changed

- The wording "Do not invoke a skill that is already running" in the skill tool description is ambiguous and easily mistaken for a concurrent execution state; the English was changed to "Do not invoke a skill that is already loaded" and the Chinese from "不要调用已在运行的技能" to "不要调用已经加载过的技能", clarifying that it refers to skills already loaded/activated in the current conversation.
- The install scripts `scripts/setup-cli.ps1` and `scripts/setup-cli.sh` remove the y/N confirmation before upgrading: when an existing installation is detected it is upgraded in place at the original path, and only a fresh install asks for the install level; switching between system-level and user-level requires uninstalling first, and reinstalling triggers the path question again.

## [0.12.0] - 2026-09-09

### Added

- The session compression prompt (both English and Chinese) adds an "emotional sanitization" principle: summaries strip profanity, personal attacks and other insulting emotional wording, keeping only neutral technical information and summarizing user dissatisfaction in one neutral sentence; "preserve the decision chain" now quotes the user's technical original wording; the [禁区]/[硬约束] items of Critical Context require emotional wording to be stripped.
- Added the `terminal_title` module: on interactive loop startup it sets the terminal window title to `[ManualAid] <project folder name>` using `crossterm::terminal::SetTitle` (falling back to `[ManualAid]` when no usable folder name exists, with control characters in the folder name stripped to avoid breaking the OSC sequence); the title write goes through the `console` outlet and does not write to the real terminal when stdout is not a terminal or is under test capture.

### Changed

- Skill unique-name system refactor: `Skill::unique_name` is always the stable, provenance-traceable `<scope>-<agent_dir>-<name>` (e.g. `project-.agents-pdf`); `Skill` gains an `agent_dir` field assigned directly by the scanner at load time. Skills with the same name but different content are no longer renamed with a `.project-`/`.global-` prefix but each keeps a stable name; residual conflicts from the same frontmatter `name` declared within one scan root are resolved with a deterministic `-N` suffix, and suffix candidates avoid other skills' natural names (a real skill named `pdf-2` is not shadowed).
- Added exposed-name resolution: `exposed_name_map` and `resolve_skill` compute the shortest unique name over the *enabled* skill set (bare name when no conflict, `<agent_dir>-` prefix for same names, full stable name for the same directory across scopes); the `<available_skills>` prompt list and the Skill tool parameter now use exposed names, while `resolve_skill` can still resolve disabled skills' full stable names in order to report "disabled"; `get_skill` is tightened to match only full stable names.
- Skill duplicate detection is tolerant of whitespace and line endings: `description`/`body` have leading/trailing whitespace stripped and `\r\n` normalized to `\n` before comparison, without rewriting the stored text.
- The Skill tool "not found" message and CLI management UI (`debug skill`, loop CLI skill menu) are updated accordingly: the message lists exposed names and the management UI always shows the full stable unique name.

## [0.11.0] - 2026-09-07

### Added

- When `old_string` does not match, the edit tool now returns only targeted diagnostics: for a pure newline difference it directly gives the corresponding line-ending style and adjustment advice, and when a highly similar candidate exists it returns only the candidate text, no longer stacking the base "not found, please re-read the file" hint, to avoid interfering with model judgment.

### Fixed

- Removed the "use forward slashes for paths on Windows" suggestion from the system prompt, to avoid that guidance causing path parsing failures.

## [0.10.3] - 2026-09-05

### Added

- On successful Skill tool execution, the returned `invoke_skill` JSON header gains a `path` field whose value is the skill folder's absolute path (`/`-separated), for the Agent to resolve relative resource paths such as `scripts/`, `references/` inside the skill.
- The edit tool's line-ending mismatch warning gains an actionable hint: it directly instructs switching `old_string` and `new_string` to the CRLF/LF actually used by the matched snippet in the file and retrying.
- The read tool footer automatically reports the file's line-ending style (LF/CRLF/mixed), so the line-ending information needed for editing is available without enabling per-line diagnostics separately.

### Fixed

- Fixed a misjudgment of matched-snippet newlines by the edit tool's line-ending difference detection for files with mixed line endings.

## [0.10.2] - 2026-09-04

### Added

- Added the `.github/workflows/changelog-reminder.yml` workflow: when a PR is detected to change `crates/`, `Cargo.toml`, `Cargo.lock` or the `.github/` directory, it posts a reminder comment if `CHANGELOG*.md` was not updated; if the CHANGELOG is later updated, it automatically updates the existing comment to a passing state.

### Fixed

- Simplified the default denial prompts in both Chinese and English to a plain denial statement (`操作已被用户拒绝` / `Operation denied by user`), removing redundant descriptions such as the command content, whitelist state and approval requirement (command information is already provided via the result parameter summary).
