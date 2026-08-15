#!/usr/bin/env python3
"""Runs every plotting script and writes all charts to ./output.

Usage:
    python plot_all.py               # assumes ../data already exists
    python plot_all.py --regenerate  # regenerate ../data via the Rust CLI first
"""
from __future__ import annotations

import argparse
import sys

from common import DATA_DIR


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--regenerate", action="store_true",
                         help="regenerate ../data by invoking the Rust CLI first")
    args = parser.parse_args()

    if args.regenerate or not DATA_DIR.exists():
        import generate_data
        print("Generating data via the Rust CLI ...")
        sys.argv = [sys.argv[0]]  # use generate_data's own defaults
        generate_data.main()

    required = [
        "example_schedule_lo.csv", "example_schedule_hi.csv", "example_summary.csv",
        "fig5_fms_sweep.csv", "fig6_sweep_u_lo_lo.csv",
        "fig7_sweep_u_hi_lo.csv", "fig8_sweep_u_lo_hi.csv",
    ]
    missing = [f for f in required if not (DATA_DIR / f).exists()]
    if missing:
        print(f"error: missing data files: {missing}\n"
              f"Run `python generate_data.py` (or `python plot_all.py --regenerate`) first.",
              file=sys.stderr)
        sys.exit(1)

    print("\n=== Gantt charts (Table 1 example, Sec. 5.3) ===")
    import plot_gantt
    plot_gantt.main()

    print("\n=== Fig. 5: deadline scaling factor sweep (FMS use-case) ===")
    import plot_fig5_fms
    plot_fig5_fms.main()

    print("\n=== Figs. 6-8: extensive utilization sweeps ===")
    import plot_figs_6_7_8
    plot_figs_6_7_8.main()

    print("\nAll charts written to visualization/output/")


if __name__ == "__main__":
    main()
