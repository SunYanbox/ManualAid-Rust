#!/usr/bin/env python3
"""Unit tests for the release-notes sync script's pure helpers.

Network-facing functions are intentionally not exercised here; the workflow and
manual runs cover those.

针对 release-notes 同步脚本纯函数的单元测试。

有意不覆盖网络相关函数，那部分由工作流与手工运行验证。
"""

import unittest

from update_release_notes import (
    MARKER,
    build_notes,
    merge_body,
    normalize_tag,
    parse_changelog,
    plan_targets,
)

CHANGELOG = """# ChangeLog

Intro line.

## [Unreleased]

## [0.13.2] - 2026-09-10

### Changed

- second release note

## [0.13.1] - 2026-09-10

### Fixed

- first release note
"""


class ParseChangelogTest(unittest.TestCase):
    def test_extracts_each_version_section(self):
        sections = parse_changelog(CHANGELOG)

        self.assertEqual(
            sorted(sections), ["0.13.1", "0.13.2", "Unreleased"]
        )
        self.assertIn("second release note", sections["0.13.2"])
        self.assertIn("first release note", sections["0.13.1"])

    def test_drops_version_heading(self):
        sections = parse_changelog(CHANGELOG)

        self.assertNotIn("## [0.13.2]", sections["0.13.2"])
        self.assertNotIn("2026-09-10", sections["0.13.2"])

    def test_unreleased_is_empty(self):
        self.assertEqual(parse_changelog(CHANGELOG)["Unreleased"], "")


class NormalizeTagTest(unittest.TestCase):
    def test_strips_leading_v(self):
        self.assertEqual(normalize_tag("v0.13.2"), "0.13.2")

    def test_keeps_bare_version(self):
        self.assertEqual(normalize_tag("0.13.2"), "0.13.2")


class BuildNotesTest(unittest.TestCase):
    def test_english_then_separator_then_chinese(self):
        notes = build_notes("english body", "chinese body")

        self.assertTrue(notes.startswith(MARKER))
        self.assertLess(notes.index("english body"), notes.index("---"))
        self.assertLess(notes.index("---"), notes.index("chinese body"))

    def test_chinese_only_skips_separator(self):
        notes = build_notes("", "chinese body")

        self.assertNotIn("---", notes)
        self.assertIn("chinese body", notes)


class MergeBodyTest(unittest.TestCase):
    def test_empty_body_returns_generated(self):
        generated = build_notes("english", "chinese")

        self.assertEqual(merge_body(None, generated), generated)

    def test_preserves_auto_generated_prefix(self):
        existing = "## What's Changed\n\n- a pull request\n"
        generated = build_notes("english", "chinese")

        merged = merge_body(existing, generated)

        self.assertTrue(merged.startswith("## What's Changed"))
        self.assertIn(MARKER, merged)
        self.assertIn("english", merged)

    def test_replaces_previous_managed_block(self):
        old = build_notes("old english", "old chinese")
        existing = f"## What's Changed\n\n- a pull request\n\n{old}"
        generated = build_notes("new english", "new chinese")

        merged = merge_body(existing, generated)

        self.assertIn("new english", merged)
        self.assertNotIn("old english", merged)
        self.assertEqual(merged.count(MARKER), 1)

    def test_appends_behind_separator_when_unmarked(self):
        generated = build_notes("english", "chinese")

        merged = merge_body("hand written notes", generated)

        self.assertTrue(merged.startswith("hand written notes"))
        self.assertIn("---", merged)
        self.assertIn(MARKER, merged)


class PlanTargetsTest(unittest.TestCase):
    @staticmethod
    def _release(tag, body=""):
        return {"tag_name": tag, "body": body}

    def test_backfills_when_previous_lacks_marker(self):
        releases = [
            self._release("v0.12.0"),
            self._release("v0.13.0"),
            self._release("v0.13.2"),
        ]

        targets, backfill = plan_targets(releases, "v0.13.2")

        self.assertTrue(backfill)
        self.assertEqual(
            [r["tag_name"] for r in targets],
            ["v0.12.0", "v0.13.0", "v0.13.2"],
        )

    def test_incremental_when_previous_has_marker(self):
        releases = [
            self._release("v0.13.0", f"notes\n{MARKER}"),
            self._release("v0.13.2"),
        ]

        targets, backfill = plan_targets(releases, "v0.13.2")

        self.assertFalse(backfill)
        self.assertEqual([r["tag_name"] for r in targets], ["v0.13.2"])

    def test_first_ever_release_backfills_itself(self):
        releases = [self._release("v0.13.2")]

        targets, backfill = plan_targets(releases, "v0.13.2")

        self.assertTrue(backfill)
        self.assertEqual([r["tag_name"] for r in targets], ["v0.13.2"])


if __name__ == "__main__":
    unittest.main()
