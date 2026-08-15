//! CLI for reproducing the experiments of:
//! Zhang, Y.-W. "DVFS-based energy-aware scheduling of imprecise
//! mixed-criticality real-time tasks." JSA 137 (2023) 102849.
//!
//! Subcommands:
//!   example   - Table 1 worked example (Sec. 5.3): prints x, S_LO and a
//!               simulated schedule trace (LO-mode-only and switch scenarios).
//!   fms       - Table 2 avionics (FMS) use-case + x-sweep, reproducing Fig. 5.
//!   sweep     - Extensive experiments of Sec. 6.2 (Figs. 6-8): sweeps
//!               U_LO_LO, U_HI_LO, or U_LO_HI and writes a CSV of normalized
//!               energy consumption for EA-IMC vs. "IMC without DVFS".

use clap::{Parser, Subcommand, ValueEnum};
use ea_imc_core::algorithm::{EaImcScheduler, Overrun, SimHorizon, Utilizations};
use ea_imc_core::energy::PowerModel;
use ea_imc_core::task::{Criticality, Task, TaskId, TaskSet};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ea-imc-cli", about = "EA-IMC scheduling algorithm simulator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Table 1 worked example from Sec. 4.1 / 5.3.
    Example {
        #[arg(long, default_value = "data")]
        out_dir: PathBuf,
    },
    /// Table 2 FMS avionics use-case + deadline-scaling-factor sweep (Fig. 5).
    Fms {
        #[arg(long, default_value = "data/fig5_fms_sweep.csv")]
        out: PathBuf,
        #[arg(long, default_value_t = 40)]
        steps: usize,
    },
    /// Extensive utilization sweeps of Sec. 6.2 (Figs. 6-8).
    Sweep {
        #[arg(value_enum)]
        which: SweepTarget,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 30)]
        steps: usize,
        #[arg(long, default_value_t = 1000)]
        repeats: usize,
        #[arg(long, default_value_t = 7)]
        n_hi: usize,
        #[arg(long, default_value_t = 4)]
        n_lo: usize,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum SweepTarget {
    /// Fig. 6: impact of U^{LO}_{LO}(Gamma).
    ULoLo,
    /// Fig. 7: impact of U^{HI}_{LO}(Gamma), with U_LO_LO = 1.2 * U_HI_LO.
    UHiLo,
    /// Fig. 8: impact of U^{LO}_{HI}(Gamma), with U_LO_LO fixed at 0.4.
    ULoHi,
}

fn table1_taskset() -> TaskSet {
    // Table 1: tau1 (LO, T=8, C(LO)=2, C(HI)=1), tau2 (HI, T=12, C(LO)=2,
    // C(HI)=3), tau3 (LO, T=16, C(LO)=4, C(HI)=2).
    TaskSet::new(vec![
        Task::new(TaskId(1), "tau1", Criticality::LO, 8.0, 2.0, 1.0).unwrap(),
        Task::new(TaskId(2), "tau2", Criticality::HI, 12.0, 2.0, 3.0).unwrap(),
        Task::new(TaskId(3), "tau3", Criticality::LO, 16.0, 4.0, 2.0).unwrap(),
    ])
    .unwrap()
}

fn fms_taskset() -> TaskSet {
    // Table 2, the flight-management-system (FMS) subset.
    let rows: &[(usize, &str, Criticality, f64, f64, f64)] = &[
        (1, "tau1", Criticality::HI, 5000.0, 15.0, 21.0),
        (2, "tau2", Criticality::HI, 200.0, 18.0, 25.0),
        (3, "tau3", Criticality::HI, 1000.0, 16.0, 22.0),
        (4, "tau4", Criticality::HI, 1600.0, 20.0, 28.0),
        (5, "tau5", Criticality::HI, 100.0, 18.0, 26.0),
        (6, "tau6", Criticality::HI, 1000.0, 17.0, 24.0),
        (7, "tau7", Criticality::HI, 1000.0, 15.0, 21.0),
        (8, "tau8", Criticality::LO, 1000.0, 100.0, 50.0),
        (9, "tau9", Criticality::LO, 1000.0, 80.0, 40.0),
        (10, "tau10", Criticality::LO, 1000.0, 140.0, 70.0),
        (11, "tau11", Criticality::LO, 1000.0, 100.0, 50.0),
    ];
    let tasks = rows
        .iter()
        .map(|&(id, name, crit, t, clo, chi)| Task::new(TaskId(id), name, crit, t, clo, chi).unwrap())
        .collect();
    TaskSet::new(tasks).unwrap()
}

fn write_schedule_csv(path: &PathBuf, schedule: &ea_imc_core::schedule::Schedule) {
    let mut wtr = csv::Writer::from_path(path).expect("open csv");
    wtr.write_record(["time", "task", "event", "criticality", "mode", "speed"])
        .unwrap();
    for e in &schedule.events {
        wtr.write_record([
            format!("{:.6}", e.time),
            e.task_name.clone(),
            format!("{:?}", e.event_type),
            format!("{:?}", e.criticality),
            format!("{:?}", e.mode),
            format!("{:.6}", e.speed.value()),
        ])
        .unwrap();
    }
    wtr.flush().unwrap();
}

fn run_example(out_dir: PathBuf) {
    std::fs::create_dir_all(&out_dir).expect("create out_dir");
    let ts = table1_taskset();
    let u = Utilizations::of(&ts);
    println!("Table 1 task set utilizations:");
    println!(
        "  U_LO_LO = {:.4}  U_HI_LO = {:.4}  U_LO_HI = {:.4}  U_HI_HI = {:.4}",
        u.u_lo_lo, u.u_hi_lo, u.u_lo_hi, u.u_hi_hi
    );

    let sched = EaImcScheduler::default();
    let (x_lo, x_up) = sched.feasible_x_range(&ts).expect("feasible");
    println!("Feasible x range (Lemma 3): [{:.4}, {:.4}]", x_lo, x_up);

    let cfg = sched.config_for_x(&ts, 0.5).expect("schedulable at x=0.5");
    println!("At x = 0.5: S_LO = {:.4} (paper reports 0.83)", cfg.s_lo);

    let opt = sched.optimal_config(&ts).expect("optimal config");
    println!(
        "EA-IMC optimal: x = {:.4}, S_LO = {:.4}",
        opt.x, opt.s_lo
    );

    let pm = PowerModel::paper_default();
    let ne_ea_imc = pm.normalized_energy_lo_mode(&ts, opt.s_lo);
    let ne_no_dvfs = pm.normalized_energy_lo_mode(&ts, 1.0);
    println!(
        "Normalized LO-mode energy (Eq. 2): EA-IMC = {:.4}, IMC w/o DVFS = {:.4} ({:.2}% reduction)",
        ne_ea_imc,
        ne_no_dvfs,
        100.0 * (1.0 - ne_ea_imc / ne_no_dvfs)
    );

    // Simulate the no-overrun (LO-mode-only, ~Fig. 1/3) scenario.
    let sched_lo = sched
        .simulate(&ts, &cfg, Overrun::None, SimHorizon::OneHyperperiod)
        .expect("simulate");
    println!(
        "\n[LO-mode-only trace] hyperperiod = {}, {} events, schedulable = {}",
        sched_lo.hyperperiod,
        sched_lo.events.len(),
        sched_lo.schedulable
    );
    for e in &sched_lo.events {
        println!(
            "  t={:>6.2}  {:<12} {:<10} mode={} speed={:.2}",
            e.time, e.task_name, format!("{:?}", e.event_type), e.mode, e.speed.value()
        );
    }

    // Simulate a mode-switch scenario: tau2's 2nd job (job_index=1) overruns.
    let sched_hi = sched
        .simulate(
            &ts,
            &cfg,
            Overrun::At { task_id: TaskId(2), job_index: 1 },
            SimHorizon::OneHyperperiod,
        )
        .expect("simulate");
    println!(
        "\n[Mode-switch trace, tau2 overruns] switch_time = {:?}, schedulable = {}",
        sched_hi.mode_switch_time, sched_hi.schedulable
    );
    for e in &sched_hi.events {
        println!(
            "  t={:>6.2}  {:<12} {:<10} mode={} speed={:.2}",
            e.time, e.task_name, format!("{:?}", e.event_type), e.mode, e.speed.value()
        );
    }

    write_schedule_csv(&out_dir.join("example_schedule_lo.csv"), &sched_lo);
    write_schedule_csv(&out_dir.join("example_schedule_hi.csv"), &sched_hi);

    let mut wtr = csv::Writer::from_path(out_dir.join("example_summary.csv")).expect("open csv");
    wtr.write_record([
        "u_lo_lo", "u_hi_lo", "u_lo_hi", "u_hi_hi", "x_at_0_5_s_lo", "x_opt", "s_lo_opt",
        "ne_ea_imc", "ne_no_dvfs", "pct_reduction", "switch_time",
    ])
    .unwrap();
    wtr.write_record([
        format!("{:.6}", u.u_lo_lo),
        format!("{:.6}", u.u_hi_lo),
        format!("{:.6}", u.u_lo_hi),
        format!("{:.6}", u.u_hi_hi),
        format!("{:.6}", cfg.s_lo),
        format!("{:.6}", opt.x),
        format!("{:.6}", opt.s_lo),
        format!("{:.6}", ne_ea_imc),
        format!("{:.6}", ne_no_dvfs),
        format!("{:.4}", 100.0 * (1.0 - ne_ea_imc / ne_no_dvfs)),
        format!("{:.4}", sched_hi.mode_switch_time.unwrap_or(f64::NAN)),
    ])
    .unwrap();
    wtr.flush().unwrap();

    println!("\nWrote schedule + summary CSVs to {}", out_dir.display());
}

fn run_fms(out: PathBuf, steps: usize) {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let ts = fms_taskset();
    let u = Utilizations::of(&ts);
    println!(
        "FMS utilizations: U_HI_HI={:.2} U_LO_HI={:.2} U_LO_LO={:.2} U_HI_LO={:.2}",
        u.u_hi_hi, u.u_lo_hi, u.u_lo_lo, u.u_hi_lo
    );

    let pm = PowerModel::paper_default();
    let ne_no_dvfs = pm.normalized_energy_lo_mode(&ts, 1.0);
    println!("Normalized energy, IMC without DVFS: {:.4}", ne_no_dvfs);

    let sched = EaImcScheduler::default();
    let (x_lo, x_up) = sched.feasible_x_range(&ts).expect("feasible");
    println!("Feasible x range: [{:.4}, {:.4}]", x_lo, x_up);

    let mut wtr = csv::Writer::from_path(&out).expect("open csv");
    wtr.write_record(["x", "s_lo", "ne_ea_imc", "ne_no_dvfs", "pct_reduction"])
        .unwrap();

    let mut reductions = Vec::new();
    for i in 0..=steps {
        let x = x_lo + (x_up - x_lo) * (i as f64) / (steps as f64);
        if let Ok(cfg) = sched.config_for_x(&ts, x) {
            let ne = pm.normalized_energy_lo_mode(&ts, cfg.s_lo);
            let pct = 100.0 * (1.0 - ne / ne_no_dvfs);
            reductions.push(pct);
            wtr.write_record([
                format!("{:.6}", x),
                format!("{:.6}", cfg.s_lo),
                format!("{:.6}", ne),
                format!("{:.6}", ne_no_dvfs),
                format!("{:.4}", pct),
            ])
            .unwrap();
        }
    }
    wtr.flush().unwrap();

    let avg = reductions.iter().sum::<f64>() / reductions.len() as f64;
    println!("Wrote {} rows to {}", reductions.len(), out.display());
    println!(
        "Average energy reduction over sweep: {:.2}% (paper reports 24.55%)",
        avg
    );
}

fn run_sweep(which: SweepTarget, out: PathBuf, steps: usize, repeats: usize, n_hi: usize, n_lo: usize) {
    use ea_imc_core::generator::{GeneratorConfig, TaskSetGenerator};
    use rand::{Rng, SeedableRng};

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let pm = PowerModel::paper_default();
    let sched = EaImcScheduler::default();

    let mut wtr = csv::Writer::from_path(&out).expect("open csv");
    wtr.write_record(["swept_value", "ne_ea_imc", "ne_no_dvfs", "pct_reduction"])
        .unwrap();

    let (lo, hi, label) = match which {
        SweepTarget::ULoLo => (0.05, 0.45, "U_LO_LO"),
        SweepTarget::UHiLo => (0.05, 0.40, "U_HI_LO"),
        SweepTarget::ULoHi => (0.05, 0.45, "U_LO_HI"),
    };
    println!("Sweeping {label} from {lo} to {hi} over {} points, {repeats} repeats each", steps + 1);

    let mut base_rng = rand::rngs::StdRng::seed_from_u64(42);

    for i in 0..=steps {
        let v = lo + (hi - lo) * (i as f64) / (steps as f64);

        let mut ne_ea_sum = 0.0;
        let mut ne_base_sum = 0.0;
        let mut n_ok = 0usize;

        for _ in 0..repeats {
            let seed: u64 = base_rng.gen();
            let cfg = match which {
                SweepTarget::ULoLo => {
                    GeneratorConfig::section_6_2_defaults(n_hi, n_lo, v, seed)
                }
                SweepTarget::UHiLo => {
                    // U_LO_LO = 1.2 * U_HI_LO (Sec. 6.2.2), U_LO_HI = 0.6*U_HI_HI.
                    let u_lo_lo = 1.2 * v;
                    let mut c = GeneratorConfig::section_6_2_defaults(n_hi, n_lo, u_lo_lo, seed);
                    c.u_hi_lo = v;
                    c
                }
                SweepTarget::ULoHi => {
                    // U_LO_LO fixed at 0.4 (Sec. 6.2.3), U_LO_HI swept directly.
                    let mut c = GeneratorConfig::section_6_2_defaults(n_hi, n_lo, 0.4, seed);
                    c.u_lo_hi = v;
                    c
                }
            };

            let mut gen = TaskSetGenerator::new(seed);
            let ts = match gen.generate(&cfg) {
                Ok(ts) => ts,
                Err(_) => continue,
            };

            let opt = match sched.optimal_config(&ts) {
                Ok(o) => o,
                Err(_) => continue,
            };

            let ne_ea = pm.normalized_energy_lo_mode(&ts, opt.s_lo);
            let ne_base = pm.normalized_energy_lo_mode(&ts, 1.0);
            ne_ea_sum += ne_ea;
            ne_base_sum += ne_base;
            n_ok += 1;
        }

        if n_ok == 0 {
            continue;
        }
        let ne_ea_avg = ne_ea_sum / n_ok as f64;
        let ne_base_avg = ne_base_sum / n_ok as f64;
        let pct = 100.0 * (1.0 - ne_ea_avg / ne_base_avg);

        wtr.write_record([
            format!("{:.6}", v),
            format!("{:.6}", ne_ea_avg),
            format!("{:.6}", ne_base_avg),
            format!("{:.4}", pct),
        ])
        .unwrap();
    }
    wtr.flush().unwrap();
    println!("Wrote {}", out.display());
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Example { out_dir } => run_example(out_dir),
        Commands::Fms { out, steps } => run_fms(out, steps),
        Commands::Sweep { which, out, steps, repeats, n_hi, n_lo } => {
            let out = out.unwrap_or_else(|| {
                let name = match which {
                    SweepTarget::ULoLo => "fig6_sweep_u_lo_lo.csv",
                    SweepTarget::UHiLo => "fig7_sweep_u_hi_lo.csv",
                    SweepTarget::ULoHi => "fig8_sweep_u_lo_hi.csv",
                };
                PathBuf::from("data").join(name)
            });
            run_sweep(which, out, steps, repeats, n_hi, n_lo)
        }
    }
}
