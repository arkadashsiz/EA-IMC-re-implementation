#!/usr/bin/env python3
"""Reproduces Fig. 5 of the paper: impact of the deadline scaling factor x
on normalized energy consumption for the FMS avionics use-case (Table 2).
"""
from __future__ import annotations

import pandas as pd
import matplotlib.pyplot as plt

from common import DATA_DIR, COLOR_EA_IMC, COLOR_BASELINE, set_paper_style, savefig


def main() -> None:
    set_paper_style()
    path = DATA_DIR / "fig5_fms_sweep.csv"
    df = pd.read_csv(path)

    fig, ax = plt.subplots(figsize=(6.5, 4.5))
    ax.plot(df["x"], df["ne_no_dvfs"], "s-", color=COLOR_BASELINE, label="IMC without DVFS",
            markersize=4, linewidth=1.8)
    ax.plot(df["x"], df["ne_ea_imc"], "o-", color=COLOR_EA_IMC, label="EA-IMC",
            markersize=4, linewidth=1.8)

    ax.set_xlabel("Deadline scaling factor $x$")
    ax.set_ylabel("Normalized Energy Consumption")
    ax.set_title("Impact of the deadline scaling factor $x$ on energy consumption\n(FMS avionics use-case, Table 2)")
    ax.set_ylim(bottom=0)
    ax.legend(loc="upper right")

    avg_reduction = df["pct_reduction"].mean()
    ax.text(
        0.02, 0.04,
        f"avg. reduction over sweep: {avg_reduction:.2f}%\n(paper reports 24.55%)",
        transform=ax.transAxes, fontsize=9, va="bottom",
        bbox=dict(boxstyle="round", facecolor="white", edgecolor="0.7", alpha=0.9),
    )

    savefig(fig, "fig5_deadline_scaling_factor.png")

    fig2, ax2 = plt.subplots(figsize=(6.5, 3.8))
    ax2.plot(df["x"], df["pct_reduction"], "o-", color="#6a3d9a", markersize=4, linewidth=1.8)
    ax2.set_xlabel("Deadline scaling factor $x$")
    ax2.set_ylabel("Energy reduction of EA-IMC vs.\nIMC without DVFS (%)")
    ax2.set_title("EA-IMC energy savings vs. $x$ (FMS use-case)")
    savefig(fig2, "fig5b_energy_savings_pct.png")


if __name__ == "__main__":
    main()
