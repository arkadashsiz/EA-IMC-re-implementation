# EA-IMC-re-implementation

**Course:** Low Power Digital System Design (Spring 2026)  
**Paper:** Zhang, Y.-W. (2023). DVFS-based energy-aware scheduling of imprecise mixed-criticality real-time tasks. *Journal of Systems Architecture*, 102849.  

---

## Overview

This project implements the **EA-IMC** (Energy-Aware Imprecise Mixed-Criticality) scheduling algorithm from the above paper. The algorithm uses DVFS (Dynamic Voltage and Frequency Scaling) to reduce energy consumption in mixed‑criticality real‑time systems, while providing **degraded service** to low‑criticality (LO) tasks when the system switches to high‑criticality (HI) mode – unlike conventional models that simply discard LO tasks.

---

## Features
//TODO
---

## IMC Task Model in a Nutshell

Each periodic task \(\tau_i\) is defined by:

| Parameter | Meaning |
|-----------|---------|
| \(T_i\) | Period (implicit deadline, \(D_i = T_i\)) |
| \(L_i \in \{LO, HI\}\) | Criticality level |
| \(C_i(LO)\) | WCET in LO mode |
| \(C_i(HI)\) | WCET in HI mode |

**Behaviour:**
- Start in **LO mode** at speed \(S_{LO}\) (energy‑efficient).
- If any HI task executes for \(C_i(LO)/S_{LO}\) time without finishing → switch to **HI mode**.
- In HI mode: HI tasks run at full speed \(S_{max}=1\) (up to \(C_i(HI)\)). LO tasks are **degraded**: each job is limited to \(C_i(HI)\) execution (suspended if exceeded).

**IMC Correctness:**
- LO mode: all jobs finish within \(C_i(LO)/S_{LO}\) and meet deadlines.
- HI mode: total execution of any task ≤ \(C_i(HI)/S\) and all deadlines met.

---

## Getting Started
//TODO
### Prerequisites
//TODO
### Compilation & Execution
//TODO
