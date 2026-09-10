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

## [0.10.1] - 2026-08-30

### Changed

- The intent output rule is moved out of `<rules>` and rendered as a standalone `<system-reminder>` before the rules block; the rule content is replaced with the new `# Intent`/`# ToolCall`/`# Clarify`/`# Answer` format, and the duplicated intent explanation in `capabilities` is removed.

## [0.10.0] - 2026-08-27

### Added

- `manualaid-cli` gains a `copy` subcommand to copy 10 kinds of prompt snippets without entering the TUI; `system-prompt` and `context` support the `--context-files` override switch (values `all`/`none`/`First`, default `First`), `--lang` takes effect for the subcommand, and a clipboard write failure returns a non-zero exit code.

## [0.9.0] - 2026-08-26

### Added

- The system prompt `path-rules` adds an @-prefixed file reading rule.
- The copy-prompt second-level menu adds "copy context", reusing the multi-context file selection prompt.
- `manualaid-ws::prompt` adds a public `render_context_reminder` function shared by system prompt construction and the copy-context menu.
- The copy-prompt second-level menu adds "copy compression session prompt", copying the fixed compression session prompt template to the clipboard.
- When a tool result exceeds `max_result_chars` and triggers truncation, the full untruncated output is written to `<workspace_root>/.ManualAid/temp/<sha256>.md`, and a localized notice is appended to the truncated text informing of the staging path and the starting line number of each tool's output.

### Changed

- The `<directory_listing>` block gains a localized snapshot note at the beginning, reminding the LLM that the directory structure is a startup snapshot and will not be updated later; a missing newline before the closing tag is fixed.
- The line-ending escaping description of the JSON code-block tool-call template is refined: distinguishing the escaping of LF and CRLF.
- The workspace context files are moved out of `<dynamic-context>` and output as a standalone `<system-reminder>` block after `</system_prompt>`; the `render_context_files` output becomes a per-file `Instructions from` section with localized file tags.
- The system prompt `path-rules` removes the path-source reference to `<context_files>`.

### Fixed

- Added the directory-listing and Windows forward-slash rules missing from the Chinese system prompt `path-rules`, aligning it with the English version.

## [0.8.0] - 2026-08-25

### Added

- The main menu input supports running Shell commands directly with a `!` prefix, reusing round result display and history.
- The settings menu adds a built-in changelog viewer, supporting viewing by version and viewing all at once.
- `i18n` adds public `changelog_all`, `changelog_versions` and `changelog_version` interfaces, and wraps `set_locale`.
- Menu items gain stable unique keys (slug) for callers and tests to select menu items.

### Changed

- Tool output pagination is changed to three lines per page.
- Approval preview and Diff output pagination is changed to 20 lines for the first page and 10 lines afterwards.

## [0.7.0] - 2026-08-21

### Added

- Added `menu.rs` and `command.rs`, unifying numeric menu and inline command handling.
- Tool results wrap the tool name in square brackets and show a summary of important parameters.
- Read output appends a range/line-count marker at the end.
- Unclosed tool calls are kept as failed results instead of being silently dropped.
- The default parser order is changed to json-codeblock, invoke, xml.
- The Windows platform notes add a CoreUtils hint.
- The config menu adds a copy-prompt submenu, supporting copying the intent rule, tool format, enabled tool list, line-ending handling rule, plan mode rule, execution-mode switching rule and task planning rule.

### Changed

- Tool toggles are split into a third-level submenu.
- The `skill` module is split into `config_io`, `frontmatter`, `scan` and other submodules.
- A large amount of implementation and tests are split by module directory to improve maintainability.
- Updated prompts and tool descriptions.

## [0.6.0] - 2026-08-18

### Added

- Added an invoke wire-format parser supporting `<invoke name="tool"><parameter name="param">value</parameter></invoke>`.
- `ToolCallFormat` gains an `Invoke` variant, and the `/format` cycle order and configuration support `invoke`.

### Fixed

- Fixed the XML parser swallowing subsequent closing tags when a bare `<` in a parameter value is followed by whitespace or an invalid name-start character.

## [0.5.0] - 2026-08-18

### Added

- Added the `debug whitelist` command to view the audited command whitelist's defaults, project/global configuration, merged effective list and blacklist conflicts in layers.
- Added inline shortcut commands: `/help`, `/history`, `/summary`, `/clear`, `/mode`, with aliases such as `/h`, `/H`, `/s`, `/cls`, `/m`.
- After the main loop first renders the menu it shows a help hint, and shows the current approval mode before the prompt.
- The startup message includes the version number.
- The JSON parser accepts the `func_calls` fence used by the system prompt.

### Changed

- Greatly expanded the built-in default Shell whitelist, adding ls, cat, grep, the git series, the gh series, the cargo series and platform commands.
- Aligned the Agent prompt with the tool descriptions and updated the Chinese and English locale texts.
- The main loop adds `/mode`, `/m` to quickly switch the approval mode.
- The config menu's approval mode label logic is moved to utils.

## [0.4.1] - 2026-08-16

### Added

- The Read tool adds `show_line_numbers` and `show_line_endings` diagnostic parameters.
- Added the `AGENTS.md` development guide.
- Added the `docs/comment-style.md`, `docs/commit-conventions.md` and `docs/issue-pr-guide.md` documents.
- Added the `scripts/ci.cmd`, `scripts/ci.ps1` and `scripts/ci.sh` CI check scripts.

### Changed

- When the Edit `old_string` is not found, a newline difference hint or candidate strings with a similarity of at least 90% are attached.
- Updated the Chinese and English descriptions of the Read/Edit tools, removing the default 2000-line note and adding diagnostic parameter descriptions.

## [0.4.0] - 2026-08-15

### Added

- Added the `debug` subcommand group, migrating the former `mask`, `restore` and `skill` subcommands into its subcommands.
- Added `debug plan_edit`: pre-checks whether the Edit `old_string` matches via the real validation path, reporting the occurrence count and search text without modifying files.
- Added `debug shell`: previews, confirms and executes a Shell command, showing stdout, stderr, exit code and elapsed time.
- `debug plan_edit` and `debug shell` support the `@file path` syntax to read content arguments from a file.
- Added the `docs/zh-cn/工具系统/关于XML解析器的逻辑.md` document.

### Changed

- `plan_edit` and `EditPlan` are made public so debug commands can reuse the real execution validation path.
- The `plan_edit` `new_string` defaults to an empty string when missing.
- The XML parser enforces a strict CDATA closing rule: `]]>` and the closing tag must be adjacent with no whitespace allowed; unclosed parameter tags emit a soft warning.
- Non-adjacent CDATA is treated as literal text.

## [0.3.1] - 2026-08-15

### Added

- The Edit success result adds line-count changes and diff output; with `replace_all` a file-level diff is generated, otherwise a parameter-level diff.

### Changed

- The XML parser trims leading whitespace before CDATA wrapping, and non-CDATA parameters only trim leading/trailing newlines and allow empty strings.

### Fixed

- The Edit `old_string` is attached when it is not found.

## [0.3.0] - 2026-08-14

### Added

- Added `EnabledToolSet`, snapshotting the currently enabled tools and their parameter names, so the parser recognizes only tools and parameters in the set.
- `FormatRegistry` adds a `set_enabled_tools` cache, reusing `Arc<EnabledToolSet>` when the set is unchanged.
- Added the `ClipboardProvider` trait, providing `read`/`write` abstractions.
- Added `RealClipboard` and `MockClipboard`, supporting injected read/write errors.
- Added the intent rule copy command and the `prompt.system.intent-output-rule` template.
- Added Token estimation display for generating the system prompt and executing a round.

### Changed

- `ToolCallFormatParser::try_parse` now takes `&EnabledToolSet`.
- `FormatRegistry::parse` returns `ParseOutcome`, containing the call list and soft warnings.
- Rewrote the XML scanner, adding tool filtering and unclosed-tag soft warnings.
- The JSON codeblock parser also supports tool filtering.
- `read_clipboard`/`write_clipboard` now delegate to `RealClipboard`, remaining backward compatible.
- Tests now isolate the real system clipboard via `MockClipboard`.
- Split some tests of `handlers.rs` into `tests/commands/handlers.rs`.
- Added related keys to the i18n texts.

## [0.2.0] - 2026-08-12

### Added

- Added `RoundStats`, recording each round's parsing, approval and execution time and estimated total Tokens.
- `SessionLog::push` now takes `RoundStats` and adds a `MAX_ROUNDS` limit, dropping the oldest record beyond 200 rounds.
- Added `MemoryUsage` and `SessionLog::memory_usage()`, estimating the bytes of calls, results and metadata of in-memory session records.
- Added `SessionLog::rounds()` returning all rounds.
- `ToolResult` adds `execution_duration_ms` and `estimated_tokens` fields.
- Added the tool history list `/history`, newest first, showing each round's tools, status, elapsed time, Tokens and session totals.
- Added a detailed preview when copying round results: showing the round header, tool details, elapsed time and Tokens, with at most 10 lines of content previewed.
- The config menu adds "view in-memory session usage", showing total usage and the bytes of parsing calls, results and metadata.
- Added `<platform-notes>` platform notes when building prompts on Windows.
- The git information block gains a localized snapshot note at the beginning.
- Added CI path filtering to reduce unnecessary runs.
- Added Codecov configuration, fixing the coverage threshold at 80%.
- Added an automated release workflow, building and attaching binaries for each platform.
- Added Windows and Linux install/uninstall scripts `scripts/setup-cli.*`, `scripts/uninstall-cli.*`.
- Added `docs/zh-cn/提示词设计.md`.

### Changed

- `Executor::execute` measures tool execution time and writes it into the result.
- `execute_round_with_approval` returns `RoundStats`, and `estimate_round_tokens` is added to estimate a whole round's Token consumption.
- Added a trailing newline to the format description code block.
- Updated the i18n prompts and tool selection rule texts.
- Updated the README badges and installation instructions.

### Fixed

- On Windows with cmd, commands are passed verbatim via `raw_arg`, avoiding the conflict between standard argument escaping and cmd `/C` quote parsing.
- Commands with quoted paths and quoted arguments are additionally wrapped in quotes; other platforms keep standard argument escaping.
- `Executor::pre_check` adds a Read tool path pre-check, so directories and unreadable files directly return a failure result before entering the approval queue.

## [0.1.0] - 2026-08-09

### Added

- Initialized the Rust workspace, containing the four crates `i18n`, `manualaid-core`, `manualaid-ws` and `manualaid-cli`.
- Added `.github/workflows/ci.yml`, `deny.toml`, `.gitignore`.
- Added Chinese and English `CONTRIBUTING`, `README` and third-party specification licenses.
- Added a translation wrapper library based on `rust_i18n`, providing `t_str`.
- Added five kinds of Chinese and English locale files: `audit`, `cli`, `common`, `prompts`, `tools`.
- Added the unified error type `error.rs`.
- Added user standard directory lookup `user_dir.rs`.
- Added the clipboard abstraction `clipboard.rs`.
- Added Shell execution, timeout abort and encoding detection `shell.rs`.
- Added file-lock I/O and standard directory initialization `file_io.rs`, `manualaid_dir.rs`.
- Added skill discovery, deduplication and enable management `skill.rs`.
- Added reversible privacy masking `privacy/`, containing configuration, filters and registry.
- Added timing primitives `timer.rs`.
- Added the tool layer `tools/`: Read, Edit, Write, Shell, Skill and the unified `ToolResult`.
- Added the parsing layer `parser/`: XML, JSON codeblock, registry and format parsers.
- Added the audit layer `audit/`: path boundary checks, Shell whitelist/blacklist and content checks.
- Added the execution pipeline `executor.rs`: routing, validation, mask restoration, audit, execution, post-processing.
- Added workspace path normalization and boundary checks `workspace.rs`.
- Added async file read/write `async_fs.rs`.
- Added workspace config loading and persistence `config.rs`.
- Added context file selection `context.rs`.
- Added system prompt construction `prompt.rs`.
- Added session log recording `session.rs`.
- Added command-line argument parsing and entry `cli.rs`, `main.rs`, `lib.rs`.
- Added console capture output `console.rs`, terminal styling `style.rs`, pager `pager.rs`.
- Added directory tree rendering `dir_tree.rs`, environment path helpers `env.rs`.
- Added subcommands: `init`, `dir`, `mask`, `restore`, `skill`.
- Added the interactive Agent Copy-Paste Loop: main loop, numeric menu, config menu, approval queue, system prompt generation, paste submission, result copy, session history, inline shortcut commands, approval preview and diff display.
- Added `docs/zh-cn/` Chinese design documents.
- Added a large number of module and integration tests.
