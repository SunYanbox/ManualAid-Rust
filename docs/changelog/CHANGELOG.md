# Changelog

## [Unreleased]

## [0.14.0] - 2026-09-12

### Added

- New `/compress-fence` shortcut (also `copy compressed-fence`): copies a hand-off template wrapping an empty `<compacted-summary>` fence, ready to paste at the start of a new chat.

### Changed

- The conversation compression prompt is rewritten into a shorter, tighter checkpoint format: the eight-section structure stays, while the old principles and tips are folded into one compact rules list.
- The system prompt is clearer: contradictions and undefined references are removed, the rules about touching files through the terminal are tightened, and a reply with many tool calls can now put them in one JSON array instead of many code blocks.

### Fixed

- The tool-call template shown in the prompt no longer omits the commas between parameters, and its trailing note is now a comment, so the example is valid JSONC

## [0.13.2] - 2026-09-10

### Changed

- The Linux download is roughly a third smaller: release builds now move the executable's symbol table into a separate `.debug` file, and both platforms publish their symbol file next to the executable so crashes remain diagnosable

## [0.13.1] - 2026-09-10

### Fixed

- The input suggestion panel no longer leaves stale lines behind or redraws the prompt repeatedly when candidates are long or numerous

## [0.13.0] - 2026-09-10

### Added

- New `/compress`, `/plan` (`/p`), `/build` (`/b`) and `/skills` shortcuts: copy the conversation compression prompt, the plan mode prompt, the switch execution mode prompt and the enabled skills list straight from the prompt; `/tools` now copies the full enabled tools list template.
- The copy-prompt menu adds a "copy enabled skills list" entry and upgrades "copy currently enabled tools list" to the full tools list template (parameters and call templates included); copied tools and skills lists are uniformly wrapped in a `<system-reminder>` block.
- `!` command results are now posted as `[USER_ACTION kind="exec" command="…"] … [END USER_ACTION]`, clearly marking that this is an operation directly initiated by the user rather than an automatic tool call.
- The system prompt adds a USER_ACTION explanation, so the external model recognizes the meaning of user-driven operation blocks and avoids re-executing commands the user has already initiated.
- The main menu input now supports interactive completion: typing `/` suggests built-in commands and available skills, and typing `@` suggests project files and folders; `/SKILL` and `@path` take effect as the user actively loading a skill and referencing a path respectively, producing corresponding USER_ACTION result blocks.
- The main menu input completion editor restores history recall and cursor movement: with no completion panel, up/down arrows switch history entries and left/right arrows move the cursor within the line, with Ctrl+arrows moving by word; identical input keeps only the newest entry, and the cursor stays aligned for multi-byte characters such as Chinese.
- During tool execution a progress line is shown: `(seconds) tool1, tool2, ...`, each tool colored by state (not started grey, running yellow, success green, failure red); when execution ends the line is kept with a newline before the original result summary and timing are output; no timer is shown during the approval phase.

### Changed

- The skill tool description changed from "Do not invoke a skill that is already running" to "Do not invoke a skill that is already loaded", to avoid mistaking "loaded" for "executing" and reduce repeated invocation of the same skill.
- The changelog viewer now follows the UI language: the English interface shows English content, while Chinese and other languages show Chinese content.

## [0.12.0] - 2026-09-09

### Added

- The session compression summary now filters profanity, personal attacks and other emotional expressions, keeping only neutral technical information, to avoid aggressive content affecting the performance of subsequent sessions.
- On interactive loop startup the terminal window title is set to `[ManualAid] <project folder name>`, so sessions of multiple projects are recognizable at a glance in the window and tab bar.

### Changed

- Skill naming and resolution refactor: every skill has a stable full unique name (`<scope>-<directory>-<name>`, e.g. `project-.agents-pdf`); the name the agent sees in the available skills list and the Skill tool stays short (the skill name) when there is no conflict and automatically carries a prefix for the same name, while the management UI always shows the full unique name.
- Skill duplicate detection is more tolerant: identical skills differing only in line-ending style (LF/CRLF) and leading/trailing whitespace are treated as duplicates.

## [0.11.0] - 2026-09-07

### Added

- On edit failure only the most critical diagnostic is given: for a newline mismatch it directly hints how to adjust, and for similar content it gives only reference text, no longer emitting irrelevant hints such as "please re-read the file" at the same time.

### Fixed

- Removed the "use forward slashes for paths on Windows" suggestion from the system prompt, to avoid that guidance causing path parsing failures.

## [0.10.3] - 2026-09-05

### Added

- The Skill tool execution result gains an absolute path field for the skill folder, helping the Agent correctly resolve the relative paths of scripts and reference files inside the skill.
- On edit failure the newline-difference hint now directly tells the user to switch `old_string` and `new_string` to the newline used by the matched snippet in the file and retry.
- When reading a file the footer now automatically indicates the file's line-ending style (LF/CRLF/mixed), without manually enabling line-ending diagnostics.

### Fixed

- Fixed an issue where the edit tool could incorrectly report that the matched snippet used CRLF in files with mixed line endings.

## [0.10.2] - 2026-09-04

### Fixed

- Simplified the default denial prompt to a plain denial statement, removing redundant descriptions such as the command content, whitelist state and approval requirement.

## [0.10.1] - 2026-08-30

### Changed

- The intent output prompt is replaced with a system-level rule and adopts the new intent block format.

## [0.10.0] - 2026-08-27

### Added

- Added the `manualaid-cli copy` command, copying 10 kinds of prompts directly without entering the interactive menu; when copying the system prompt or context files, `--context-files` can be used to choose the load scope.

## [0.9.0] - 2026-08-26

### Added

- The system prompt `path-rules` adds an @-prefixed file reading rule.
- The copy-prompt menu adds "copy context", supporting selection of multiple workspace context files.
- The copy-prompt menu adds "copy compression session prompt", which when pasted into a session lets the model compress the current session.
- When a tool result exceeds the character limit and is truncated, the full output is staged to a file under `.ManualAid/temp/`, and the truncation notice informs of the file location and the line number of each tool's output.

### Changed

- The `<directory_listing>` block gains a snapshot note at the beginning, indicating that the directory structure is a startup snapshot and will not be updated later.
- The line-ending escaping description of the JSON code-block (json-codeblock) tool-call template is refined: distinguishing the escaping of LF and CRLF.
- Workspace context files are now shown in a standalone prompt block at the end of the system prompt, with localized file tags.
- The system prompt `path-rules` removes the path-source reference to `<context_files>`.

### Fixed

- Added the directory-listing and Windows forward-slash rules missing from the Chinese system prompt `path-rules`, aligning it with the English version.

## [0.8.0] - 2026-08-25

### Added

- The main menu supports running Shell commands directly with a `!` prefix.
- The settings menu adds a built-in changelog viewer.

### Changed

- Tool output pagination is changed to three lines per page.
- Approval preview and Diff output pagination is changed to 20 lines for the first page and 10 lines afterwards.

## [0.7.0] - 2026-08-21

### Added

- Tool results show a summary of important parameters.
- Read output appends a range/line-count marker at the end.
- Unclosed tool calls are kept as failed results.
- The config menu adds a copy-prompt submenu, supporting copying the intent rule, tool format, enabled tool list, line-ending handling rule, plan mode rule, execution-mode switching rule and task planning rule.

### Changed

- The default parser order is changed to json-codeblock, invoke, xml.
- Unified menu and inline command handling.

## [0.6.0] - 2026-08-18

### Added

- Support for the invoke tool-call format.

### Fixed

- Fixed a bare `<` in an XML parameter value swallowing subsequent tags.

## [0.5.0] - 2026-08-18

### Added

- Added the `debug whitelist` command to view the Shell whitelist.
- Added inline shortcut commands: `/help`, `/history`, `/summary`, `/clear`, `/mode`, with aliases such as `/h`, `/H`, `/s`, `/cls`, `/m`.

### Changed

- Expanded the default Shell whitelist.
- Updated prompts and tool descriptions.

## [0.4.1] - 2026-08-16

### Added

- The Read tool supports showing line numbers and line-ending markers.

### Changed

- When the Edit `old_string` is not found, clearer newline difference or similar text hints are given.

## [0.4.0] - 2026-08-15

### Added

- Added `debug plan_edit`: pre-checks the Edit match.
- Added `debug shell`: previews, confirms and executes a Shell command.
- The former `mask`, `restore` and `skill` commands are migrated into `debug` subcommands.

### Changed

- The XML parser is stricter about CDATA closing, and unclosed parameters emit a soft warning.

## [0.3.1] - 2026-08-15

### Changed

- The Edit success result now shows line-count changes and a diff.
- The XML parser optimizes CDATA whitespace handling and empty-string parameters.

## [0.3.0] - 2026-08-14

### Added

- Support for filtering parsing by enabled tools, with parsing soft warnings shown.
- Copy intent rule shortcut command.
- Token estimation display when generating the system prompt and executing a round.

### Fixed

- Clipboard read/write changed to an injectable Provider abstraction, so tests no longer pollute the system clipboard.

## [0.2.0] - 2026-08-12

### Added

- Tool history list, showing each round's tools, elapsed time and Token statistics.
- Detailed preview when copying round results.
- The config menu adds "view in-memory session usage".
- The Windows platform prompt adds platform notes.

### Fixed

- Fixed the cmd Shell command quote parsing conflict on Windows.
- Fixed directory reads not being intercepted before approval.

## [0.1.0] - 2026-08-09

### Added

- Initial release of the ManualAid Rust rewrite.
- Support for starting the interactive Agent Loop with no arguments.
- Built-in Read, Edit, Write, Shell, Skill tools.
- Support for skill discovery and loading.
- Support for session round logs and result copying.
- Command-line subcommands such as init, dir, mask, restore, skill.
- Chinese and English UI switching.
- Permission approval, privacy masking and Shell whitelist.
