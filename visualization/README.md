# EA-IMC Visualization

Python scripts that turn the CSV output of `ea-imc-cli` into charts for
understanding the EA-IMC results, mirroring the figures in Zhang (2023):

| Script                  | Reproduces                          | Output                                                                 |
|--------------------------|--------------------------------------|-------------------------------------------------------------------------|
| `plot_gantt.py`          | Figs. 1-4 (Table 1 worked example)   | `gantt_lo_mode.png`, `gantt_hi_mode.png`                               |
| `plot_fig5_fms.py`       | Fig. 5 (FMS use-case, x sweep)       | `fig5_deadline_scaling_factor.png`, `fig5b_energy_savings_pct.png`     |
| `plot_figs_6_7_8.py`     | Figs. 6-8 (extensive experiments)    | `fig6_impact_u_lo_lo.png`, `fig7_impact_u_hi_lo.png`, `fig8_impact_u_lo_hi.png`, `fig6_7_8_combined_savings.png` |

## Quick start

From the `visualization/` directory:

```bash
pip install -r requirements.txt

# Generates ../data/*.csv (runs the Rust CLI) and every chart in one go:
python plot_all.py --regenerate

# If ../data already exists (e.g. you ran the CLI manually) you can skip
# regeneration:
python plot_all.py
```

Charts are written to `visualization/output/*.png`.

## Running steps individually

```bash
# 1. Generate data (equivalent to running the CLI commands by hand)
python generate_data.py --steps 30 --repeats 500

# 2. Plot whichever figure(s) you want
python plot_gantt.py
python plot_fig5_fms.py
python plot_figs_6_7_8.py
```

## Notes

- `generate_data.py` shells out to `cargo run -p ea-imc-cli`, so a Rust
  toolchain must be on `PATH` (see the top-level `README.md`). Increase
  `--repeats` for smoother sweep curves at the cost of longer runtime
  (defaults: 30 points x 500 repeats per figure, a few seconds total).
- `plot_gantt.py` reads the per-event schedule traces
  (`example_schedule_lo.csv` / `example_schedule_hi.csv`) and reconstructs
  execution intervals from `JobStart`/`JobResume`/`JobComplete`/`JobSuspend`
  events, then draws a horizontal bar per task with release-time arrows and
  the mode-switch instant marked — the same information as Figs. 1-4 of the
  paper, generated from the actual simulator rather than hand-drawn.
- `common.py` holds the shared color scheme and output-directory
  conventions used by all scripts; edit it to restyle every chart at once.
