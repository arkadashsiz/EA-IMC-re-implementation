#!/usr/bin/env python3
"""Regenerates all CSVs under ../data by invoking the Rust CLI.

Equivalent to running, from the project root:

    cargo run -q -p ea-imc-cli -- example --out-dir data
    cargo run -q -p ea-imc-cli -- fms --steps 40
    cargo run -q -p ea-imc-cli -- sweep u-lo-lo --steps 30 --repeats 500
    cargo run -q -p ea-imc-cli -- sweep u-hi-lo --steps 30 --repeats 500
    cargo run -q -p ea-imc-cli -- sweep u-lo-hi --steps 30 --repeats 500

Usage:
    python generate_data.py [--repeats 500] [--steps 30] [--release]
"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys

from common import PROJ_DIR, DATA_DIR


def run(cmd: list[str]) -> None:
    print("$ " + " ".join(cmd))
    subprocess.run(cmd, cwd=PROJ_DIR, check=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repeats", type=int, default=500, help="repeats per sweep point")
    parser.add_argument("--steps", type=int, default=30, help="points per sweep")
    parser.add_argument("--release", action="store_true", help="build/run in release mode")
    args = parser.parse_args()

    if shutil.which("cargo") is None:
        print("error: `cargo` not found on PATH. Install Rust (e.g. `apt install cargo rustc`) first.",
              file=sys.stderr)
        sys.exit(1)

    base = ["cargo", "run", "-q", "-p", "ea-imc-cli"]
    if args.release:
        base.append("--release")
    base_args = base + ["--"]

    DATA_DIR.mkdir(exist_ok=True)

    run(base_args + ["example", "--out-dir", "data"])
    run(base_args + ["fms", "--steps", "40"])
    for target in ("u-lo-lo", "u-hi-lo", "u-lo-hi"):
        run(base_args + ["sweep", target, "--steps", str(args.steps), "--repeats", str(args.repeats)])

    print(f"\nAll data written to {DATA_DIR}")


if __name__ == "__main__":
    main()
