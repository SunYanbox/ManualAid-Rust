#!/usr/bin/env python3
"""Synchronize GitHub Release notes with the repository changelogs.

Reads the developer-facing ``CHANGELOG.md`` / ``CHANGELOG_ZH_CN.md`` and writes
the matching version section into each Release body, wrapped in a unique HTML
comment marker so the next run can tell whether that Release is already managed.

When the *previous* Release carries the marker the script only refreshes the
current Release; otherwise it backfills every Release from the oldest up to the
current one, so a first run (or a run after a broken pipeline) restores the
whole history.

读取面向开发者的 ``CHANGELOG.md`` / ``CHANGELOG_ZH_CN.md``，把对应版本段落写入
各 Release 正文，并用唯一的 HTML 注释标记包裹，便于下次运行判断该 Release
是否已由本流程接管。

若*上一个* Release 带有该标记，仅刷新当前 Release；否则从最早的 Release 一直
回填到当前 Release，使首次运行（或流程中断后的运行）能补齐全部历史。
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.request
from pathlib import Path

# Unique marker written into every managed Release body. It doubles as the
# idempotency token: its presence on the previous Release means the pipeline has
# already taken over and only the current Release needs refreshing.
# 写入每个受管 Release 正文的唯一标记，同时充当幂等凭据：出现在上一个 Release
# 上即表示流程已接管，只需刷新当前 Release。
MARKER = "<!-- manualaid-release-notes -->"

# Horizontal rule separating the English and Chinese halves.
# 分隔中英文两半的水平线。
SEPARATOR = "---"

API_ROOT = "https://api.github.com"

# ``## [x.y.z] - date`` / ``## [Unreleased]`` headings of a Keep a Changelog file.
# Keep a Changelog 文件的 ``## [x.y.z] - date`` / ``## [Unreleased]`` 标题。
_HEADING_RE = re.compile(
    r"^##[ \t]+\[(?P<version>[^\]]+)\].*$", re.MULTILINE
)


def parse_changelog(text: str) -> dict[str, str]:
    """Split a changelog into ``{version: section}`` preserving file order.

    The ``## [x.y.z] - date`` heading itself is dropped because the Release
    title already carries the version and repeating it only adds noise.

    把 changelog 拆成 ``{版本: 段落}``，保持文件顺序。

    丢弃 ``## [x.y.z] - date`` 标题行：Release 标题已含版本号，重复只会增加噪音。
    """
    matches = list(_HEADING_RE.finditer(text))
    sections: dict[str, str] = {}
    for index, match in enumerate(matches):
        version = match.group("version").strip()
        start = match.end()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        sections[version] = text[start:end].strip()
    return sections


def build_notes(english: str, chinese: str) -> str:
    """Compose the managed block: marker, English, separator, Chinese.

    组装受管区块：标记、英文、分隔线、中文。
    """
    parts = [MARKER, ""]
    if english:
        parts.append(english)
    if english and chinese:
        parts += ["", SEPARATOR, ""]
    if chinese:
        parts.append(chinese)
    return "\n".join(parts).rstrip() + "\n"


def merge_body(existing: str | None, generated: str) -> str:
    """Return a Release body where ``generated`` owns the managed block.

    Content before the marker (for example GitHub's auto-generated notes) is
    preserved; a marker-less body gets the block appended behind a separator.

    返回新的 Release 正文，由 ``generated`` 接管受管区块。

    标记之前的内容（如 GitHub 自动生成的说明）保持不变；不含标记的正文会在
    分隔线之后追加该区块。
    """
    existing = (existing or "").strip()
    if not existing:
        return generated
    index = existing.find(MARKER)
    if index != -1:
        return existing[:index].rstrip() + "\n\n" + generated
    if existing.endswith(SEPARATOR):
        return existing + "\n\n" + generated
    return existing + "\n\n" + SEPARATOR + "\n\n" + generated


def normalize_tag(tag: str) -> str:
    """Map a release tag to its changelog version, e.g. ``v0.13.2`` -> ``0.13.2``.

    把 Release 标签映射到 changelog 版本，例如 ``v0.13.2`` -> ``0.13.2``。
    """
    return tag[1:] if tag[:1] in ("v", "V") else tag


def plan_targets(
    releases: list[dict], current_tag: str
) -> tuple[list[dict], bool]:
    """Pick the releases to refresh and report whether this is a backfill.

    ``releases`` must be ordered oldest first. When the previous Release lacks
    the marker, every Release up to and including the current one is returned;
    otherwise only the current Release is.

    选出需要刷新的 Release，并返回是否为回填模式。

    ``releases`` 必须按从旧到新排序。若上一个 Release 缺少标记，则返回截至当前
    Release（含）的全部 Release；否则只返回当前 Release。
    """
    tags = [release["tag_name"] for release in releases]
    current = tags.index(current_tag)
    previous = releases[current - 1] if current > 0 else None
    backfill = previous is None or MARKER not in (previous.get("body") or "")
    if backfill:
        return releases[: current + 1], True
    return [releases[current]], False


def _request(url: str, token: str, *, method: str = "GET", payload=None):
    """Issue a GitHub REST call and decode the JSON response.

    发起 GitHub REST 调用并解析 JSON 响应。
    """
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    request = urllib.request.Request(url, data=data, method=method)
    request.add_header("Authorization", f"Bearer {token}")
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("X-GitHub-Api-Version", "2022-11-28")
    if data is not None:
        request.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(request) as response:
        body = response.read()
    return json.loads(body) if body else None


def list_releases(repo: str, token: str) -> list[dict]:
    """Fetch every Release, oldest first.

    获取全部 Release，按从旧到新排序。
    """
    releases: list[dict] = []
    page = 1
    while True:
        batch = _request(
            f"{API_ROOT}/repos/{repo}/releases?per_page=100&page={page}", token
        )
        if not batch:
            break
        releases.extend(batch)
        if len(batch) < 100:
            break
        page += 1
    releases.sort(key=lambda release: release["created_at"])
    return releases


def update_release(repo: str, token: str, release_id: int, body: str) -> None:
    """Overwrite a single Release body.

    覆盖单个 Release 正文。
    """
    _request(
        f"{API_ROOT}/repos/{repo}/releases/{release_id}",
        token,
        method="PATCH",
        payload={"body": body},
    )


def main(argv: list[str] | None = None) -> int:
    """Entry point: parse arguments, plan targets and sync their bodies.

    入口：解析参数、规划目标并同步正文。
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY"))
    parser.add_argument(
        "--token",
        default=os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN"),
    )
    parser.add_argument(
        "--tag",
        default=os.environ.get("RELEASE_TAG") or os.environ.get("GITHUB_REF_NAME"),
    )
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--changelog-zh", default="CHANGELOG_ZH_CN.md")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print the planned updates without calling the API",
    )
    args = parser.parse_args(argv)

    if not args.repo:
        parser.error("missing --repo (or GITHUB_REPOSITORY)")
    if not args.token:
        parser.error("missing --token (or GH_TOKEN/GITHUB_TOKEN)")
    if not args.tag:
        parser.error("missing --tag (or RELEASE_TAG/GITHUB_REF_NAME)")

    english = parse_changelog(Path(args.changelog).read_text(encoding="utf-8"))
    chinese = parse_changelog(Path(args.changelog_zh).read_text(encoding="utf-8"))

    releases = list_releases(args.repo, args.token)
    if args.tag not in [release["tag_name"] for release in releases]:
        print(f"release {args.tag} not found; nothing to do")
        return 0

    targets, backfill = plan_targets(releases, args.tag)
    print(f"mode={'backfill' if backfill else 'incremental'}")

    for release in targets:
        version = normalize_tag(release["tag_name"])
        notes = build_notes(english.get(version, ""), chinese.get(version, ""))
        if notes.strip() == MARKER:
            print(f"skip {release['tag_name']}: no changelog entry for {version}")
            continue
        body = merge_body(release.get("body"), notes)
        if body.strip() == (release.get("body") or "").strip():
            print(f"skip {release['tag_name']}: already up to date")
            continue
        if args.dry_run:
            print(f"[dry-run] update {release['tag_name']}")
            continue
        update_release(args.repo, args.token, release["id"], body)
        print(f"updated {release['tag_name']}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
