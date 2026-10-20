#!/usr/bin/env python3
"""Read and write Debian binary packages (`.deb`) without the `dpkg` tools.

The Linux packaging leg has to produce a *real* Debian package on the build
machine, and the build machine is not necessarily a Debian host: the release
runner and the developer workstation both differ. This module implements the
binary package format so the packaging leg owns exactly one writer:

    ar archive
      debian-binary     the format version ("2.0")
      control.tar.gz    the package metadata (control, md5sums, maintainer scripts)
      data.tar.gz       the installed file tree

The writer is deterministic (fixed timestamps, fixed ownership, gzip without a
file name or modification time), so the same input tree always produces the
same bytes and therefore the same SHA-256 identity.

Usage:
    python3 deb_archive.py build --output PKG --control FILE --tree DIR
                                 [--script NAME=PATH]...
    python3 deb_archive.py info --package PKG
    python3 deb_archive.py extract --package PKG --destination DIR

Exit codes: 0 success, 1 a malformed package or tree, 2 a usage error.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import sys
import tarfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator, Sequence

AR_MAGIC = b"!<arch>\n"
AR_HEADER_SIZE = 60
DEBIAN_BINARY = b"2.0\n"
CONTROL_MEMBER = "control.tar.gz"
DATA_MEMBER = "data.tar.gz"
CONTROL_SCRIPTS = ("preinst", "postinst", "prerm", "postrm")
FILE_MODE = 0o644
EXEC_MODE = 0o755
DIR_MODE = 0o755
# The archive timestamps are pinned so two builds of the same tree agree.
FIXED_TIMESTAMP = 0


@dataclass(frozen=True)
class ArMember:
    """One member of an `ar` archive."""

    name: str
    data: bytes


@dataclass(frozen=True)
class DebPackage:
    """The parsed content of a Debian binary package."""

    members: tuple[ArMember, ...]
    control: dict[str, str]
    data_entries: tuple[str, ...]
    scripts: tuple[str, ...]

    def member(self, name: str) -> bytes:
        for member in self.members:
            if member.name == name:
                return member.data
        raise KeyError(name)


def _gzip_bytes(payload: bytes) -> bytes:
    """Compress without a file name or a modification time (reproducible)."""
    buffer = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=buffer, mtime=FIXED_TIMESTAMP) as handle:
        handle.write(payload)
    return buffer.getvalue()


def read_ar(path: Path) -> list[ArMember]:
    """Parse an `ar` archive into its members."""
    raw = path.read_bytes()
    if not raw.startswith(AR_MAGIC):
        raise ValueError(f"{path}: not an ar archive")
    members: list[ArMember] = []
    offset = len(AR_MAGIC)
    while offset < len(raw):
        if offset + AR_HEADER_SIZE > len(raw):
            raise ValueError(f"{path}: truncated ar header at offset {offset}")
        header = raw[offset:offset + AR_HEADER_SIZE]
        if header[58:60] != b"`\n":
            raise ValueError(f"{path}: malformed ar header at offset {offset}")
        name = header[0:16].decode("ascii", "replace").strip().rstrip("/")
        size = int(header[48:58].decode("ascii").strip() or "0")
        offset += AR_HEADER_SIZE
        payload = raw[offset:offset + size]
        if len(payload) != size:
            raise ValueError(f"{path}: truncated member {name}")
        offset += size
        if offset % 2:
            offset += 1
        members.append(ArMember(name=name, data=payload))
    return members


def write_ar(path: Path, members: Sequence[ArMember]) -> None:
    """Write an `ar` archive with the member layout Debian expects."""
    chunks: list[bytes] = [AR_MAGIC]
    for member in members:
        fields = (
            f"{member.name:<16}",
            f"{FIXED_TIMESTAMP:<12}",
            f"{0:<6}",
            f"{0:<6}",
            f"{0o100644:o}".ljust(8),
            f"{len(member.data):<10}",
            "`\n",
        )
        header = "".join(fields).encode("ascii")
        if len(header) != AR_HEADER_SIZE:
            raise ValueError("the ar header layout changed")
        chunks.append(header)
        chunks.append(member.data)
        if len(member.data) % 2:
            chunks.append(b"\n")
    path.write_bytes(b"".join(chunks))


def parse_control(text: str) -> dict[str, str]:
    """Parse the RFC 822 style control file into its fields."""
    fields: dict[str, str] = {}
    name: str | None = None
    for line in text.splitlines():
        if not line.strip():
            continue
        if line[0] in " \t":
            if name is None:
                raise ValueError("the control file starts with a continuation line")
            fields[name] += "\n" + line.strip()
            continue
        key, separator, value = line.partition(":")
        if not separator:
            raise ValueError(f"malformed control line: {line!r}")
        name = key.strip()
        fields[name] = value.strip()
    return fields


def _tree_entries(tree: Path) -> Iterator[tuple[str, Path]]:
    """Yield the tree entries in a deterministic order (parents first)."""
    for root, directories, files in os.walk(tree):
        directories.sort()
        files.sort()
        root_path = Path(root)
        relative_root = root_path.relative_to(tree)
        yield "." if relative_root == Path(".") else f"./{relative_root.as_posix()}", root_path
        for name in files:
            path = root_path / name
            relative = path.relative_to(tree).as_posix()
            yield f"./{relative}", path


def _tar_info(archive: tarfile.TarFile, path: Path, name: str) -> tarfile.TarInfo:
    info = archive.gettarinfo(str(path), arcname=name)
    info.mtime = FIXED_TIMESTAMP
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "root"
    if info.isdir():
        info.mode = DIR_MODE
    elif info.issym():
        info.mode = 0o777
    else:
        info.mode = EXEC_MODE if path.stat().st_mode & 0o111 else FILE_MODE
    return info


def build_data_archive(tree: Path) -> bytes:
    """Pack the installed file tree into the `data.tar.gz` member."""
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.GNU_FORMAT) as archive:
        for name, path in _tree_entries(tree):
            info = _tar_info(archive, path, name)
            if info.isdir():
                archive.addfile(info)
            elif info.issym():
                archive.addfile(info)
            else:
                with path.open("rb") as handle:
                    archive.addfile(info, handle)
    return _gzip_bytes(buffer.getvalue())


def build_md5sums(tree: Path) -> bytes:
    """Build the `md5sums` file: one digest per installed regular file."""
    lines: list[str] = []
    for root, directories, files in os.walk(tree):
        directories.sort()
        files.sort()
        for name in files:
            path = Path(root) / name
            if path.is_symlink() or not path.is_file():
                continue
            digest = hashlib.md5(path.read_bytes()).hexdigest()
            lines.append(f"{digest}  {path.relative_to(tree).as_posix()}")
    return ("\n".join(lines) + "\n" if lines else "").encode("utf-8")


def write_package(
    output: Path,
    control_text: str,
    scripts: dict[str, bytes],
    tree: Path,
    md5sums: bytes | None = None,
) -> None:
    """Write a complete Debian binary package."""
    if not tree.is_dir():
        raise ValueError(f"{tree}: the package tree is not a directory")
    control_payload = control_text.encode("utf-8")
    control_members = {"./control": control_payload}
    if md5sums is not None:
        control_members["./md5sums"] = md5sums
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.GNU_FORMAT) as archive:
        for name, payload in sorted(control_members.items()):
            info = tarfile.TarInfo(name)
            info.size = len(payload)
            info.mode = FILE_MODE
            info.mtime = FIXED_TIMESTAMP
            info.uid = 0
            info.gid = 0
            info.uname = "root"
            info.gname = "root"
            archive.addfile(info, io.BytesIO(payload))
        for name in sorted(scripts):
            payload = scripts[name]
            info = tarfile.TarInfo(f"./{name}")
            info.size = len(payload)
            info.mode = EXEC_MODE
            info.mtime = FIXED_TIMESTAMP
            info.uid = 0
            info.gid = 0
            info.uname = "root"
            info.gname = "root"
            archive.addfile(info, io.BytesIO(payload))
    output.parent.mkdir(parents=True, exist_ok=True)
    write_ar(output, (
        ArMember("debian-binary", DEBIAN_BINARY),
        ArMember(CONTROL_MEMBER, _gzip_bytes(buffer.getvalue())),
        ArMember(DATA_MEMBER, build_data_archive(tree)),
    ))


def _tar_members(payload: bytes) -> tuple[tarfile.TarFile, io.BytesIO]:
    buffer = io.BytesIO(gzip.decompress(payload))
    return tarfile.open(fileobj=buffer, mode="r:"), buffer


def read_package(path: Path) -> DebPackage:
    """Parse a Debian binary package into its control fields and file tree."""
    members = read_ar(path)
    names = [member.name for member in members]
    if names[:1] != ["debian-binary"]:
        raise ValueError(f"{path}: the first member must be debian-binary")
    if members[0].data != DEBIAN_BINARY:
        raise ValueError(f"{path}: unsupported binary package format")
    if CONTROL_MEMBER not in names or DATA_MEMBER not in names:
        raise ValueError(f"{path}: the control or data member is missing")
    control_archive, _ = _tar_members(members[names.index(CONTROL_MEMBER)].data)
    control: dict[str, str] = {}
    scripts: list[str] = []
    for info in control_archive.getmembers():
        name = info.name.lstrip("./")
        if name == "control":
            handle = control_archive.extractfile(info)
            control = parse_control(handle.read().decode("utf-8") if handle else "")
        elif name in CONTROL_SCRIPTS:
            scripts.append(name)
    data_archive, _ = _tar_members(members[names.index(DATA_MEMBER)].data)
    entries = tuple(
        info.name.lstrip("./")
        for info in data_archive.getmembers()
        if not info.isdir()
    )
    return DebPackage(
        members=tuple(members),
        control=control,
        data_entries=entries,
        scripts=tuple(sorted(scripts)),
    )


def extract_data(path: Path, destination: Path) -> DebPackage:
    """Extract the installed file tree of a package into `destination`."""
    package = read_package(path)
    members = read_ar(path)
    data = next(member.data for member in members if member.name == DATA_MEMBER)
    destination.mkdir(parents=True, exist_ok=True)
    data_archive, _ = _tar_members(data)
    for info in data_archive.getmembers():
        relative = info.name.lstrip("./")
        if not relative:
            continue
        target = destination / relative
        if info.isdir():
            target.mkdir(parents=True, exist_ok=True)
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        if info.issym():
            if target.exists() or target.is_symlink():
                target.unlink()
            os.symlink(info.linkname, target)
            continue
        handle = data_archive.extractfile(info)
        if handle is None:
            continue
        target.write_bytes(handle.read())
        os.chmod(target, info.mode)
    return package


def _script_arguments(pairs: Iterable[str]) -> dict[str, bytes]:
    scripts: dict[str, bytes] = {}
    for pair in pairs:
        name, separator, path = pair.partition("=")
        if not separator:
            raise ValueError(f"malformed --script argument: {pair!r}")
        if name not in CONTROL_SCRIPTS:
            raise ValueError(f"unknown maintainer script: {name}")
        scripts[name] = Path(path).read_bytes()
    return scripts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    build = subparsers.add_parser("build", help="write a Debian binary package")
    build.add_argument("--output", required=True, type=Path)
    build.add_argument("--control", required=True, type=Path)
    build.add_argument("--tree", required=True, type=Path)
    build.add_argument("--script", action="append", default=[], metavar="NAME=PATH")

    info = subparsers.add_parser("info", help="print the package metadata as JSON")
    info.add_argument("--package", required=True, type=Path)

    extract = subparsers.add_parser("extract", help="extract the installed file tree")
    extract.add_argument("--package", required=True, type=Path)
    extract.add_argument("--destination", required=True, type=Path)

    arguments = parser.parse_args()
    try:
        if arguments.command == "build":
            control_text = arguments.control.read_text(encoding="utf-8")
            scripts = _script_arguments(arguments.script)
            write_package(
                arguments.output,
                control_text,
                scripts,
                arguments.tree,
                md5sums=build_md5sums(arguments.tree),
            )
            print(f"deb-archive: wrote {arguments.output}")
            return 0
        if arguments.command == "info":
            package = read_package(arguments.package)
            print(json.dumps({
                "control": package.control,
                "scripts": list(package.scripts),
                "data_entries": list(package.data_entries),
            }, ensure_ascii=False, indent=2, sort_keys=True))
            return 0
        package = extract_data(arguments.package, arguments.destination)
        print(f"deb-archive: extracted {len(package.data_entries)} files to {arguments.destination}")
        return 0
    except (OSError, ValueError, tarfile.TarError) as error:
        print(f"deb-archive: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
