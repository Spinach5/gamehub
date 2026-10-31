#!/usr/bin/env python3
"""Render the GitHub Release body for gameHub from CHANGELOG.md and dist/."""

from __future__ import annotations

import argparse
import os
import re
from pathlib import Path

RELEASE_HEADING = re.compile(r"^## v(?P<version>[0-9][^\s]*)")
SUBSECTION_HEADING = re.compile(r"^### (?P<title>.+?)\s*$")
DATE_SUFFIX = re.compile(r"（[^）]*）\s*$")

ARCHES = ("x86_64", "aarch64")
HIGHLIGHTS_TITLE = "完成内容"
CJK = re.compile(r"[\u3000-\u303f\u4e00-\u9fff\uff00-\uffef]")
LIST_ITEM = re.compile(r"^\s*[-*]\s+")

PACKAGES = (
    ("AppImage（推荐，免安装）", ".AppImage"),
    ("Debian / Ubuntu（`.deb`）", ".deb"),
    ("Fedora / RHEL（`.rpm`）", ".rpm"),
    ("Arch Linux（`.pkg.tar.zst`）", ".pkg.tar.zst"),
    ("通用压缩包（`.tar.xz`）", ".tar.xz"),
)

ARCH_LABELS = {
    "x86_64": "x86_64（64 位）",
    "aarch64": "aarch64（ARM64）",
}


def asset_name(version: str, arch: str, suffix: str) -> str:
    if suffix == ".tar.xz":
        return f"gamehub_{version}_linux_{arch}.tar.xz"
    return f"gamehub_{version}_{arch}{suffix}"


def parse_changelog(text: str) -> tuple[list[str], dict[str, list[str]]]:
    versions: list[str] = []
    sections: dict[str, list[str]] = {}
    current: str | None = None
    for line in text.splitlines():
        match = RELEASE_HEADING.match(line)
        if match:
            current = match.group("version")
            versions.append(current)
            sections[current] = [line]
        elif line.startswith("## "):
            current = None
        elif current is not None:
            sections[current].append(line)
    return versions, sections


def extract_heading_title(heading: str, version: str) -> str:
    rest = heading[len(f"## v{version}") :].strip()
    rest = rest.lstrip("—–-").strip()
    return DATE_SUFFIX.sub("", rest).strip()


TIGHT_PUNCTUATION = "、，。；：！？）】》」』"


def join_wrapped(left: str, right: str) -> str:
    if left[-1:] in TIGHT_PUNCTUATION or right[:1] in TIGHT_PUNCTUATION:
        return left + right
    if CJK.match(left[-1:]) and CJK.match(right[:1]):
        return left + right
    return f"{left} {right}"


def unwrap(text: str) -> str:
    """Join hard-wrapped source lines so GitHub does not insert mid-sentence <br>s."""
    lines: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped:
            lines.append("")
        elif LIST_ITEM.match(line) or not lines or not lines[-1]:
            lines.append(stripped)
        else:
            lines[-1] = join_wrapped(lines[-1], stripped)
    return "\n".join(lines).strip()


def extract_highlights(lines: list[str]) -> str:
    blocks: dict[str, list[str]] = {}
    current: str | None = None
    for line in lines:
        match = SUBSECTION_HEADING.match(line)
        if match:
            current = match.group("title")
            blocks[current] = []
        elif current is not None:
            blocks[current].append(line)
    if HIGHLIGHTS_TITLE in blocks:
        text = "\n".join(blocks[HIGHLIGHTS_TITLE]).strip()
        if text:
            return unwrap(text)
    fallback = [line for line in lines if not SUBSECTION_HEADING.match(line)]
    return unwrap("\n".join(fallback))


def previous_version(versions: list[str], version: str) -> str | None:
    try:
        index = versions.index(version)
    except ValueError:
        return None
    return versions[index + 1] if index + 1 < len(versions) else None


def available_arches(dist: Path, version: str) -> list[str]:
    found = [
        arch
        for arch in ARCHES
        if any((dist / asset_name(version, arch, suffix)).is_file() for _, suffix in PACKAGES)
    ]
    return found or list(ARCHES)


def install_commands(version: str, arch: str, suffix: str) -> list[str]:
    name = asset_name(version, arch, suffix)
    if suffix == ".AppImage":
        return [f"chmod +x {name}", f"./{name}"]
    if suffix == ".deb":
        return [f"sudo apt install ./{name}"]
    if suffix == ".rpm":
        return [f"sudo dnf install ./{name}"]
    if suffix == ".pkg.tar.zst":
        return [f"sudo pacman -U ./{name}"]
    return [f"tar -xJf {name}", f"./gamehub_{version}_linux_{arch}/gamehub"]


def render_download_addresses(
    dist: Path, version: str, repo: str, tag: str, arches: list[str]
) -> str:
    blocks: list[str] = []
    for title, suffix in PACKAGES:
        links = []
        example = None
        for arch in arches:
            name = asset_name(version, arch, suffix)
            if (dist / name).is_file():
                url = f"https://github.com/{repo}/releases/download/{tag}/{name}"
                links.append(f"[{ARCH_LABELS[arch]}]({url})")
                if example is None:
                    example = arch
        if not links or example is None:
            continue
        block = [
            f"### {title}",
            "",
            "- " + " | ".join(links),
            "",
            "```bash",
            *install_commands(version, example, suffix),
            "```",
        ]
        blocks.append("\n".join(block))
    return "\n\n".join(blocks)


def render_body(
    *,
    version: str,
    tag: str,
    repo: str,
    commit: str,
    changelog: Path,
    dist: Path,
    run_url: str | None,
) -> tuple[str, str, bool]:
    versions, sections = parse_changelog(changelog.read_text(encoding="utf-8"))
    lines = sections.get(version, [])
    heading = lines[0] if lines else f"## v{version}"
    heading_title = extract_heading_title(heading, version)
    highlights = extract_highlights(lines[1:]) if lines else ""
    if not highlights:
        highlights = "_本版本没有可展示的更新摘要，请查看完整变更记录。_"

    arches = available_arches(dist, version)
    downloads = render_download_addresses(dist, version, repo, tag, arches)
    checksums = f"gamehub_{version}_SHA256SUMS"
    appimage = asset_name(version, arches[0], ".AppImage")
    short_commit = commit[:7]
    previous = previous_version(versions, version)

    parts = [
        "<p align=\"center\">",
        f"  <img src=\"https://raw.githubusercontent.com/{repo}/{tag}/packaging/gamehub.svg\" width=\"88\" alt=\"gameHub\" />",
        "</p>",
        "",
        f"<h1 align=\"center\">gameHub <code>v{version}</code></h1>",
        "",
        "<p align=\"center\">",
        "  <b>完全离线的 Linux 游戏库</b><br />",
        "  <sub>Native Rust launcher for Windows and Linux games · 不收集数据，不发起遥测</sub>",
        "</p>",
        "",
        "---",
        "",
        "## 更新内容",
        "",
        highlights,
        "",
        "## 下载地址",
        "",
        "> x86_64 适用于 Intel / AMD 64 位电脑，aarch64 适用于 ARM64 设备。"
        "不确定格式时，桌面用户优先选择 **AppImage**（免安装）；"
        "Debian / Ubuntu 选 `.deb`，Fedora / RHEL 选 `.rpm`，Arch 选 `.pkg.tar.zst`。"
        "下方命令以 x86_64 为例，ARM64 请将文件名中的 `x86_64` 替换为 `aarch64`。",
        "",
        downloads,
        "",
        "## 校验",
        "",
        f"下载后可使用附带的 `{checksums}` 校验文件完整性：",
        "",
        "```bash",
        f"sha256sum -c --ignore-missing {checksums}",
        "```",
        "",
        "所有发布产物同时生成 GitHub Artifact Attestation（SLSA 构建来源证明）：",
        "",
        "```bash",
        f"gh attestation verify {appimage} --repo {repo}",
        "```",
        "",
        "## 系统要求",
        "",
        "- x86_64 或 aarch64 Linux，glibc ≥ 2.35",
        "- X11 或 Wayland 会话（Wine / Proton 窗口定位推荐 XWayland）",
        "- Wine 或 Proton / GE-Proton（运行 Windows 游戏时需要）",
        "- `xdg-desktop-portal` 与可用的 CJK 字体",
        "- DXVK 需要可用的 Vulkan 驱动",
        "",
        "> 未安装 Wine / Proton 也可以先浏览和管理游戏库；启动 Windows 游戏前需在 **Runners** 页面配置。",
        "",
        "---",
    ]

    if previous:
        parts.append(f"**Full Changelog**: https://github.com/{repo}/compare/v{previous}...{tag}")
    footer = (
        f"构建自 [`{short_commit}`](https://github.com/{repo}/commit/{commit})"
        f" · [CHANGELOG](https://github.com/{repo}/blob/v{version}/CHANGELOG.md)"
    )
    if run_url:
        footer += f" · [工作流运行]({run_url})"
    parts.append(footer)
    parts.append("")
    parts.append("gameHub 完全离线运行，不收集任何使用数据。")

    title = f"gameHub v{version}"
    if heading_title:
        title += f" — {heading_title}"
    prerelease = "-" in version
    return "\n".join(parts), title, prerelease


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--changelog", default="CHANGELOG.md", type=Path)
    parser.add_argument("--dist", default="dist", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()

    run_url = None
    if os.environ.get("GITHUB_RUN_ID") and os.environ.get("GITHUB_SERVER_URL"):
        run_url = (
            f"{os.environ['GITHUB_SERVER_URL']}/{args.repository}"
            f"/actions/runs/{os.environ['GITHUB_RUN_ID']}"
        )

    body, title, prerelease = render_body(
        version=args.version,
        tag=args.tag,
        repo=args.repository,
        commit=args.commit,
        changelog=args.changelog,
        dist=args.dist,
        run_url=run_url,
    )
    args.output.write_text(body + "\n", encoding="utf-8")
    if args.github_output:
        with args.github_output.open("a", encoding="utf-8") as output:
            output.write(f"title={title}\n")
            output.write(f"prerelease={'true' if prerelease else 'false'}\n")

    print(f"Wrote {args.output} ({len(body)} bytes)")
    print(f"Title: {title}")
    print(f"Prerelease: {prerelease}")


if __name__ == "__main__":
    main()
