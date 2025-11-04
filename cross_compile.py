#!/usr/bin/env python
# pyright: strict

import os
import shlex
import sys
from argparse import ArgumentParser
from typing import cast

TARGET_TO_CMD = {
    "aarch64-apple-darwin": "cargo zigbuild --target @TARGET@",
    "aarch64-pc-windows-msvc": "cargo xwin build --target @TARGET@",
    "aarch64-unknown-linux-gnu": "cargo zigbuild --target @TARGET@",
    "aarch64-unknown-linux-musl": "cargo zigbuild --target @TARGET@",
    "x86_64-apple-darwin": "cargo zigbuild --target @TARGET@",
    "x86_64-pc-windows-msvc": "cargo xwin build --target @TARGET@",
    "x86_64-unknown-linux-gnu": "cargo zigbuild --target @TARGET@",
    "x86_64-unknown-linux-musl": "cargo zigbuild --target @TARGET@",
}

# Parse arguments
arg_parser = ArgumentParser()
arg_parser.add_argument("--release", action="store_true")
arg_parser.add_argument("target", choices=TARGET_TO_CMD)
args = arg_parser.parse_args()

# Unpack arguments
target = cast(str, args.target)
cmd = TARGET_TO_CMD[target]
release = cast(bool, args.release)

# Prepare the command to execute
args = list[str]()
for arg in shlex.split(cmd):
    if arg == "@TARGET@":
        args.append(target)
    else:
        args.append(arg)
if release:
    args.append("--release")
print("+", *args, file=sys.stderr)
os.execvp(args[0], args)
