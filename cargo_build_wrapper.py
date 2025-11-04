#!/usr/bin/env python
# pyright: strict

import argparse
import os
import shlex
import shutil
import subprocess
import sys
from contextlib import contextmanager
from pathlib import Path
from typing import Generator, Optional, cast


@contextmanager
def cwd(new: Path) -> Generator[None, None, None]:
    previous = Path.cwd()
    try:
        print("+", "cd", new, file=sys.stderr)
        os.chdir(new)
        yield
    finally:
        print("+", "cd", previous, file=sys.stderr)
        os.chdir(previous)


# Parse arguments
arg_parser = argparse.ArgumentParser()
arg_parser.add_argument("--cmd", default="cargo build")
arg_parser.add_argument("--out-dynamic", type=Path)
arg_parser.add_argument("--out-static", type=Path)
arg_parser.add_argument("--release", action="store_true")
arg_parser.add_argument("--target")
arg_parser.add_argument("project_root", type=Path)
args = arg_parser.parse_args()

# Unpack arguments
initial_directory = Path.cwd()
cmd = shlex.split(cast(str, args.cmd))
out_dynamic = cast(Path, args.out_dynamic) if args.out_dynamic else None
out_static = cast(Path, args.out_static) if args.out_static else None
project_root = cast(Path, args.project_root)
release = cast(bool, args.release)
target = cast(Optional[str], args.target)

# Validate the cmd argument
if not cmd:
    raise ValueError("--cmd argument can't be empty")
cmd_path = shutil.which(cmd[0])
cmd_args = cmd[1:]
if cmd_path is None:
    raise RuntimeError(f"{cmd[0]!r} executable not found. Is Rust installed?")

# Run the command
with cwd(project_root):
    args = [cmd_path, *cmd_args, "--lib"]
    if release:
        args.append("--release")
    if target:
        args.extend(("--target", target))

    print("+", cmd[0], *args[1:], file=sys.stderr)
    subprocess.run(args, check=True)

# Determine the output directory
if target:
    target_dir = project_root / "target" / target / ("release" if release else "debug")
else:
    target_dir = project_root / "target" / ("release" if release else "debug")

# Copy out the dynamic library
if out_dynamic:
    src_dynamic = target_dir / out_dynamic.name
    if not src_dynamic.exists() and out_dynamic.name.startswith("lib"):
        # Windows workaround, instead of "libroutx.dll" Cargo produces "routx.dll"
        src_dynamic = target_dir / out_dynamic.name[3:]
    print("+", "cp", src_dynamic, out_dynamic, file=sys.stderr)
    shutil.copy2(src_dynamic, out_dynamic)

# Copy out the static library
if out_static:
    src_static = target_dir / out_static.name
    if not src_static.exists() and out_static.name.startswith("lib"):
        # Windows workaround, instead of "libroutx.lib" Cargo produces "routx.lib"
        src_static = target_dir / out_static.name[3:]
    print("+", "cp", src_static, out_static, file=sys.stderr)
    shutil.copy2(src_static, out_static)
