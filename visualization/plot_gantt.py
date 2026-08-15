#!/usr/bin/env python3
"""Renders Gantt-chart style visualizations of the Table 1 worked-example
schedules (Sec. 5.3), analogous to Figs. 1-4 of the paper: one trace that
stays in LO mode for the whole hyperperiod, and one where task tau2
overruns and triggers the switch to HI mode.

Reads ../data/example_schedule_{lo,hi}.csv, produced by
`cargo run -p ea-imc-cli -- example --out-dir data`.
"""
from __future__ import annotations

import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches

from common import DATA_DIR, CRITICALITY_COLORS, set_paper_style, savefig

# Segment pairs that bound a job's execution interval on the timeline.
START_EVENTS = {"JobStart", "JobResume"}
END_EVENTS = {"JobComplete", "JobSuspend"}


def build_intervals(df: pd.DataFrame) -> pd.DataFrame:
    """Turn a flat event log into a list of (task, start, end, criticality,
    mode, speed) execution intervals by pairing each Start/Resume with the
    next Complete/Suspend/Start/Resume for that task."""
    intervals = []
    open_seg: dict[str, dict] = {}

    for _, row in df.sort_values("time").iterrows():
        task = row["task"]
        ev = row["event"]

        if ev in START_EVENTS:
            # If a segment was already open for this task (preempted then
            # resumed without an explicit end event in between), close it
            # at this time first.
            if task in open_seg:
                seg = open_seg.pop(task)
                if row["time"] > seg["start"]:
                    intervals.append({**seg, "end": row["time"]})
            open_seg[task] = {
                "task": task,
                "start": row["time"],
                "criticality": row["criticality"],
                "mode": row["mode"],
                "speed": row["speed"],
            }
        elif ev in END_EVENTS:
            if task in open_seg:
                seg = open_seg.pop(task)
                intervals.append({**seg, "end": row["time"]})

    return pd.DataFrame(intervals)


def plot_trace(csv_name: str, title: str, out_name: str, switch_time: float | None = None) -> None:
    df = pd.read_csv(DATA_DIR / csv_name)
    intervals = build_intervals(df)

    tasks = sorted(df["task"].unique(), reverse=True)
    y_pos = {t: i for i, t in enumerate(tasks)}

    fig, ax = plt.subplots(figsize=(11, 0.9 * len(tasks) + 1.5))

    for _, seg in intervals.iterrows():
        y = y_pos[seg["task"]]
        color = CRITICALITY_COLORS.get(seg["criticality"], "#888888")
        alpha = 0.55 if seg["mode"] == "HI" else 0.9
        ax.barh(
            y, seg["end"] - seg["start"], left=seg["start"], height=0.55,
            color=color, alpha=alpha, edgecolor="black", linewidth=0.6,
        )
        mid = (seg["start"] + seg["end"]) / 2
        ax.text(mid, y, f"S={seg['speed']:.2f}", ha="center", va="center",
                fontsize=7, color="white" if alpha > 0.6 else "black")

    # Release-time arrows (job releases, matching the paper's up-arrows).
    releases = df[df["event"] == "JobRelease"]
    for _, r in releases.iterrows():
        y = y_pos[r["task"]]
        ax.annotate("", xy=(r["time"], y + 0.32), xytext=(r["time"], y - 0.05),
                    arrowprops=dict(arrowstyle="-|>", color="black", lw=0.8))

    if switch_time is not None:
        ax.axvline(switch_time, color="#d62728", linestyle="--", linewidth=1.5)
        ax.text(switch_time, len(tasks) - 0.4, f"  mode switch\n  t={switch_time:g}",
                color="#d62728", fontsize=8, va="top")

    ax.set_yticks(list(y_pos.values()))
    ax.set_yticklabels(list(y_pos.keys()))
    ax.set_xlabel("Time")
    ax.set_title(title)
    ax.set_xlim(left=-0.5)

    handles = [
        mpatches.Patch(color=CRITICALITY_COLORS["HI"], label="HI-criticality task"),
        mpatches.Patch(color=CRITICALITY_COLORS["LO"], label="LO-criticality task"),
        mpatches.Patch(color="#888888", alpha=0.55, label="executing in HI mode"),
    ]
    ax.legend(handles=handles, loc="upper center", bbox_to_anchor=(0.5, -0.18), ncol=3)

    savefig(fig, out_name)


def main() -> None:
    set_paper_style()

    summary_path = DATA_DIR / "example_summary.csv"
    switch_time = None
    if summary_path.exists():
        s = pd.read_csv(summary_path).iloc[0]
        if pd.notna(s.get("switch_time")):
            switch_time = float(s["switch_time"])

    plot_trace(
        "example_schedule_lo.csv",
        "EA-IMC schedule, Table 1 task set — no overrun (LO mode throughout)\n(analogous to Fig. 1/3 of the paper)",
        "gantt_lo_mode.png",
    )
    plot_trace(
        "example_schedule_hi.csv",
        "EA-IMC schedule, Table 1 task set — tau2 overruns, mode switch to HI\n(analogous to Fig. 2/4 of the paper)",
        "gantt_hi_mode.png",
        switch_time=switch_time,
    )


if __name__ == "__main__":
    main()
