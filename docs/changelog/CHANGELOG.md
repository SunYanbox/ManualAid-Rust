# Changelog

## [Unreleased]

### Added

- `!` command results are now posted as `[USER_ACTION kind="exec" command="…"] … [END USER_ACTION]`, clearly marking that this is an operation directly initiated by the user rather than an automatic tool call.
- The system prompt adds a USER_ACTION explanation, so the external model recognizes the meaning of user-driven operation blocks and avoids re-executing commands the user has already initiated.
- The main menu input now supports interactive completion: typing `/` suggests built-in commands and available skills, and typing `@` suggests project files and folders; `/SKILL` and `@path` take effect as the user actively loading a skill and referencing a path respectively, producing corresponding USER_ACTION result blocks.
- The main menu input completion editor restores history recall and cursor movement: with no completion panel, up/down arrows switch history entries and left/right arrows move the cursor within the line, with Ctrl+arrows moving by word; identical input keeps only the newest entry, and the cursor stays aligned for multi-byte characters such as Chinese.
- During tool execution a progress line is shown: `(seconds) tool1, tool2, ...`, each tool colored by state (not started grey, running yellow, success green, failure red); when execution ends the line is kept with a newline before the original result summary and timing are output; no timer is shown during the approval phase.

### Changed

- The skill tool description changed from "Do not invoke a skill that is already running" to "Do not invoke a skill that is already loaded", to avoid mistaking "loaded" for "executing" and reduce repeated invocation of the same skill.

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
