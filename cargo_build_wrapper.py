#!/usr/bin/env python
# pyright: strict

import argparse
import os
import sys
import subprocess
import shutil
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
arg_parser.add_argument("--out-dynamic", type=Path)
arg_parser.add_argument("--out-static", type=Path)
arg_parser.add_argument("--release", action="store_true")
arg_parser.add_argument("--target")
arg_parser.add_argument("project_root", type=Path)
args = arg_parser.parse_args()

# Unpack arguments
initial_directory = Path.cwd()
out_dynamic = cast(Path, args.out_dynamic) if args.out_dynamic else None
out_static = cast(Path, args.out_static) if args.out_static else None
project_root = cast(Path, args.project_root)
release = cast(bool, args.release)
target = cast(Optional[str], args.target)

# Run cargo build
with cwd(project_root):
    cargo_cmd = shutil.which("cargo")
    if cargo_cmd is None:
        raise RuntimeError("'cargo' executable not found in PATH. Is Rust installed?")

    args = [cargo_cmd, "build", "--lib"]
    if release:
        args.append("--release")
    if target:
        args.extend(("--target", target))

    print("+", "cargo", *args[1:], file=sys.stderr)
    subprocess.run(args, check=True)

# Determine the output directory
target_dir = project_root / "target" / ("release" if release else "debug")

# Copy out the dynamic library
if out_dynamic:
    src_dynamic = target_dir / out_dynamic.name
    print("+", "cp", src_dynamic, out_dynamic, file=sys.stderr)
    shutil.copy2(src_dynamic, out_dynamic)

# Copy out the static library
if out_static:
    src_static = target_dir / out_static.name
    print("+", "cp", src_static, out_static, file=sys.stderr)
    shutil.copy2(src_static, out_static)
