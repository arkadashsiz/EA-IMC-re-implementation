#!/usr/bin/env python3
"""Reproduces Figs. 6-8 of the paper: impact of U^{LO}_{LO}(Gamma),
U^{HI}_{LO}(Gamma), and U^{LO}_{HI}(Gamma) on normalized energy consumption
(Sec. 6.2, extensive experiments over randomly generated task sets).
"""
from __future__ import annotations

import pandas as pd
import matplotlib.pyplot as plt

from common import DATA_DIR, COLOR_EA_IMC, COLOR_BASELINE, set_paper_style, savefig

FIGURES = [
    {
        "csv": "fig6_sweep_u_lo_lo.csv",
        "xlabel": r"$U^{LO}_{LO}(\Gamma)$",
        "title": "Impact of $U^{LO}_{LO}(\\Gamma)$ on energy consumption",
        "out": "fig6_impact_u_lo_lo.png",
    },
    {
        "csv": "fig7_sweep_u_hi_lo.csv",
        "xlabel": r"$U^{HI}_{LO}(\Gamma)$",
        "title": "Impact of $U^{HI}_{LO}(\\Gamma)$ on energy consumption",
        "out": "fig7_impact_u_hi_lo.png",
    },
    {
        "csv": "fig8_sweep_u_lo_hi.csv",
        "xlabel": r"$U^{LO}_{HI}(\Gamma)$",
        "title": "Impact of $U^{LO}_{HI}(\\Gamma)$ on energy consumption",
        "out": "fig8_impact_u_lo_hi.png",
    },
]


def plot_one(spec: dict) -> float:
    df = pd.read_csv(DATA_DIR / spec["csv"])

    fig, ax = plt.subplots(figsize=(6.5, 4.5))
    ax.plot(df["swept_value"], df["ne_no_dvfs"], "s-", color=COLOR_BASELINE,
            label="IMC without DVFS", markersize=4, linewidth=1.8)
    ax.plot(df["swept_value"], df["ne_ea_imc"], "o-", color=COLOR_EA_IMC,
            label="EA-IMC", markersize=4, linewidth=1.8)

    ax.set_xlabel(spec["xlabel"])
    ax.set_ylabel("Normalized Energy Consumption")
    ax.set_title(spec["title"])
    ax.set_ylim(bottom=0)
    ax.legend(loc="upper left")

    avg = df["pct_reduction"].mean()
    ax.text(
        0.98, 0.04,
        f"avg. reduction: {avg:.2f}%",
        transform=ax.transAxes, fontsize=9, va="bottom", ha="right",
        bbox=dict(boxstyle="round", facecolor="white", edgecolor="0.7", alpha=0.9),
    )

    savefig(fig, spec["out"])
    return avg


def plot_combined_savings() -> None:
    """A single chart overlaying the % energy-saving trend of all three
    sweeps, for a quick side-by-side comparison."""
    fig, ax = plt.subplots(figsize=(7, 4.5))
    colors = ["#1f6fb2", "#2e8b57", "#e08214"]
    for spec, color in zip(FIGURES, colors):
        df = pd.read_csv(DATA_DIR / spec["csv"])
        # Normalize x-axis to [0, 1] fraction of its own sweep range so the
        # three curves (different utilization ranges) can share one plot.
        x = df["swept_value"]
        x_norm = (x - x.min()) / (x.max() - x.min())
        ax.plot(x_norm, df["pct_reduction"], "o-", color=color, markersize=3,
                linewidth=1.8, label=spec["xlabel"])

    ax.set_xlabel("Swept utilization (normalized to its own [min, max] range)")
    ax.set_ylabel("Energy reduction of EA-IMC vs.\nIMC without DVFS (%)")
    ax.set_title("EA-IMC energy savings across the three utilization sweeps\n(Figs. 6-8)")
    ax.legend(loc="upper right")
    savefig(fig, "fig6_7_8_combined_savings.png")


def main() -> None:
    set_paper_style()
    for spec in FIGURES:
        avg = plot_one(spec)
        print(f"  {spec['csv']}: average EA-IMC energy reduction = {avg:.2f}%")
    plot_combined_savings()


if __name__ == "__main__":
    main()
