# Independent Evidence Experiment — All-Seed Diagnostic Export Audit

Run: [44714090270](https://github.com/Nikkeiren/Chronos-Resonant-Quantum-Inspired-State-Lattice-CR-QISL-/actions/runs/44714090270)  
Tested commit: `059d9d1cb6327f10af3fac281454502fff256628`  
Date: 2026-11-20  
Status: six jobs successful; six artifacts inspected; 720 run rows validated.

## 1. Execution and implementation verification

All six matrix jobs completed successfully:

- BFS-ALIGNED
- BFS-REVERSED
- BFS-INDEPENDENT-BALANCED
- RANDOM-ALIGNED
- RANDOM-REVERSED
- RANDOM-INDEPENDENT-BALANCED

Each artifact contains 120 rows: four variants × 30 seeds. All 120 rows in each artifact have `status=ok`; no row has a populated error. Total: 720 successful simulations.

The artifacts were produced from commit `059d9d1cb6327f10af3fac281454502fff256628`, which includes the new diagnostic-export implementation. This is the run that verifies those columns; the earlier run `44712016218` used the prior commit and did not.

All expected columns are present in `runs.csv`:

- network/evidence group sizes
- `evidence_overlap_rate`
- contingency counts `group_n00`, `group_n01`, `group_n10`, `group_n11`
- `graph_edge_count`
- `cross_network_edge_count`
- `network_cut_ratio`

For every condition, the structural diagnostic values are identical across the four simulation variants for each seed. This is expected: the variants share the same seed-level graph and assignments.

## 2. All-seed structural diagnostics

The following summaries use all 30 seeds per condition, not only the four trace-export seeds.

| Condition | Median evidence overlap | Overlap range | Median graph edges | Median cross-group edges | Median cut ratio | Cut-ratio range |
|---|---:|---:|---:|---:|---:|---:|
| BFS aligned | 1.00 | 1.00–1.00 | 300 | 57.5 | 0.1917 | 0.1333–0.2567 |
| BFS reversed | 0.00 | 0.00–0.00 | 300 | 57.5 | 0.1917 | 0.1333–0.2567 |
| BFS independent-balanced | 0.48 | 0.42–0.60 | 300 | 57.5 | 0.1917 | 0.1333–0.2567 |
| Random aligned | 1.00 | 1.00–1.00 | 300 | 152 | 0.5067 | 0.4500–0.5567 |
| Random reversed | 0.00 | 0.00–0.00 | 300 | 152 | 0.5067 | 0.4500–0.5567 |
| Random independent-balanced | 0.48 | 0.42–0.58 | 300 | 152 | 0.5067 | 0.4500–0.5567 |

Every seed has network groups of 50/50 and evidence groups of 50/50. Every graph has 300 edges.

The all-seed export confirms that the BFS partition has substantially fewer cross-group edges than the random partition. The independent-balanced evidence assignments have intermediate overlap with network groups, near the expected 50% overlap for balanced independent labels. Aligned and reversed assignments are exact by construction.

## 3. Full CR-QISL outcomes

Descriptive medians across 30 seeds:

| Condition | Final stable fraction | Strict final agreement runs | Post-heal divergence | Final amplitude spread | Post-heal mean spread | Conflict entries | Messages sent |
|---|---:|---:|---:|---:|---:|---:|---:|
| BFS aligned | 0.045 | 0/30 | 0.1560 | 0.1139 | 0.1421 | 381.5 | 52,308.5 |
| BFS reversed | 0.020 | 0/30 | 0.1361 | 0.1116 | 0.1451 | 410.5 | 52,970 |
| BFS independent-balanced | 0.660 | 2/30 | 0.0000 | 0.0167 | 0.0311 | 0 | 26,114.5 |
| Random aligned | 0.140 | 0/30 | 0.0157 | 0.0145 | 0.0361 | 43 | 34,221.5 |
| Random reversed | 0.100 | 0/30 | 0.0179 | 0.0187 | 0.0339 | 44.5 | 33,481 |
| Random independent-balanced | 0.075 | 0/30 | 0.0000 | 0.0054 | 0.0117 | 0 | 26,042 |

Censoring remains important:

- Agreement and quorum-agreement times are censored in 30/30 runs for five conditions and 28/30 for BFS independent-balanced.
- Amplitude-convergence time is censored in 30/30 BFS aligned, BFS reversed, and BFS independent-balanced runs; 14/30 random aligned; 15/30 random reversed; and 0/30 random independent-balanced runs.

## 4. Interpretation

The diagnostic-export implementation is verified. The new all-seed columns support a much firmer description of the design than the earlier trace-only audit.

The striking BFS independent-balanced condition again has a median local stable fraction of 0.66, but strict final agreement occurs in only 2/30 runs. A high local stable fraction is not equivalent to global agreement or consensus. In the random topology, independent-balanced assignment does not reproduce the same stable-fraction increase.

The topology conditions also differ materially in cut ratio: BFS median 0.1917 versus random median 0.5067. Therefore, the apparent topology × evidence-placement interaction remains a hypothesis to investigate, not a universal causal conclusion. These are exploratory simulator results, not evidence of quantum advantage, real-world consensus safety, Byzantine tolerance, or finality.

## 5. Recommended next step

Proceed with the planned confirmatory design rather than adding more interpretations to this same batch:

1. Pre-register the topology × evidence-placement contrasts and primary outcomes.
2. Increase the number of independent seeds.
3. Add another topology or vary graph parameters while controlling cut ratio where possible.
4. Keep local stability, strict final agreement, split, divergence, amplitude spread, censoring, and communication cost as separate outcomes.
5. Report the evidence-group contingency and cut ratio alongside every condition.

