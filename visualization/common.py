"""Shared configuration and helpers for the EA-IMC visualization scripts.

All scripts read CSVs produced by the Rust CLI (`ea-imc-cli`) from
``../data`` (relative to this file) and write PNG charts to
``./output``. Run ``python generate_data.py`` first if the ``data``
folder doesn't exist yet or you want to regenerate it.
"""
from __future__ import annotations

import pathlib
import matplotlib.pyplot as plt

VIS_DIR = pathlib.Path(__file__).resolve().parent
PROJ_DIR = VIS_DIR.parent
DATA_DIR = PROJ_DIR / "data"
OUTPUT_DIR = VIS_DIR / "output"

# Paper-style palette: EA-IMC in a saturated blue, the IMC-without-DVFS
# baseline in a muted red, matching the look of Figs. 5-8 in the paper.
COLOR_EA_IMC = "#1f6fb2"
COLOR_BASELINE = "#c0392b"
COLOR_ACCENT = "#2e8b57"

CRITICALITY_COLORS = {"HI": "#c0392b", "LO": "#1f6fb2"}
MODE_HATCH = {"LO": None, "HI": "//"}


def ensure_dirs() -> None:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)


def set_paper_style() -> None:
    plt.rcParams.update(
        {
            "figure.dpi": 130,
            "savefig.dpi": 160,
            "font.size": 11,
            "axes.grid": True,
            "grid.alpha": 0.3,
            "axes.spines.top": False,
            "axes.spines.right": False,
            "legend.frameon": False,
        }
    )


def savefig(fig, name: str) -> pathlib.Path:
    ensure_dirs()
    path = OUTPUT_DIR / name
    fig.savefig(path, bbox_inches="tight")
    print(f"  wrote {path.relative_to(PROJ_DIR)}")
    return path
