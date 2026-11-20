# Confirmatory Evidence Placement Experiment — Run 44717320963 Audit

Date: 2026-11-20  
Workflow run: https://github.com/Nikkeiren/Chronos-Resonant-Quantum-Inspired-State-Lattice-CR-QISL-/actions/runs/44717320963  
Workflow commit: `ed08371e816cd8eb6a70fc5de53379acca18bf7d`  
Protocol: `docs/CONFIRMATORY_EVIDENCE_PLACEMENT_PROTOCOL.md`

## Executive result

The 100-seed replication reproduces a large positive BFS × evidence-placement interaction on **final local stable fraction**. The seed-block bootstrap estimate is **+0.4813** with a 95% percentile interval **[+0.4189, +0.5435]**. The pattern is substantial in the simulator, but it is not evidence of global consensus: the BFS independent-balanced cell reached strict final agreement in only **4/100** Full CR-QISL runs; all five other cells reached it in **0/100**.

This is a replication of an exploratory simulator pattern, not evidence of real-world consensus safety, Byzantine tolerance, finality, or quantum advantage. BFS and random partitions have markedly different cut-ratio distributions, so the experiment does not isolate topology from cut structure.

## Execution and artifact integrity

- All six matrix jobs completed successfully: BFS-ALIGNED, BFS-REVERSED, BFS-INDEPENDENT-BALANCED, RANDOM-ALIGNED, RANDOM-REVERSED, RANDOM-INDEPENDENT-BALANCED.
- Each job passed Rust unit tests, configuration validation, the 100-seed batch, and artifact upload.
- Six artifacts were retrieved and inspected.
- Each artifact contains 400 rows: 100 seeds × 4 variants. All rows have `status=ok`.
- Total: **2,400/2,400 successful simulations**.
- Each condition covers seeds 1001–1100 and contains 100 rows for each variant.
- The run used the exact workflow commit recorded above.
- Per-seed structural diagnostic columns are present and identical across the four variants within each seed.

Artifact IDs:
- BFS-ALIGNED: 14620196429
- BFS-REVERSED: 14619823329
- BFS-INDEPENDENT-BALANCED: 14620640501
- RANDOM-ALIGNED: 14619858273
- RANDOM-REVERSED: 14620665530
- RANDOM-INDEPENDENT-BALANCED: 14620016863

## Primary outcome — Full CR-QISL

Values below are medians across 100 seeds unless stated otherwise.

| Network partition | Evidence assignment | Final stable fraction | Strict final agreement | Final split | Post-heal divergence |
|---|---|---:|---:|---:|---:|
| BFS | Aligned | 0.050 | 0/100 | 0.44 mean | 0.1517 |
| BFS | Reversed | 0.030 | 0/100 | 0.40 mean | 0.1463 |
| BFS | Independent-balanced | 0.660 | 4/100 | 0.00 mean | 0.0000 |
| Random | Aligned | 0.110 | 0/100 | 0.03 mean | 0.0136 |
| Random | Reversed | 0.080 | 0/100 | 0.02 mean | 0.0152 |
| Random | Independent-balanced | 0.040 | 0/100 | 0.00 mean | 0.0000 |

Important distinction: final stable fraction is a local node-state measure. A median stable fraction of 0.66 does not mean 66% of runs reached global agreement, nor does it mean the stable nodes all selected the same candidate. Strict final agreement is a separate endpoint and occurred in only four BFS independent-balanced runs.

## Primary factorial contrast

Pre-specified contrast from the protocol:

`(BFS independent − mean(BFS aligned, BFS reversed)) − (Random independent − mean(Random aligned, Random reversed))`

Seed-block bootstrap resampled the 100 matched seed blocks, preserving all six condition cells within each resample; 20,000 bootstrap resamples; percentile 95% interval.

| Outcome | Interaction estimate | 95% bootstrap interval |
|---|---:|---:|
| Final stable fraction (primary) | +0.48130 | [+0.41885, +0.54345] |
| Strict final agreement rate | +0.04000 | [+0.01000, +0.08000] |
| Final split | −0.39500 | [−0.46500, −0.32500] |
| Post-heal divergence | −0.12157 | [−0.13203, −0.11070] |
| Final amplitude spread | −0.07453 | [−0.07956, −0.06954] |
| Post-heal mean amplitude spread | −0.08045 | [−0.08451, −0.07638] |
| Conflict entries | −310.985 | [−328.280, −293.330] |
| Messages sent | −17,348.075 | [−18,377.271, −16,317.154] |

Only final stable fraction is the pre-specified primary inferential outcome. Other rows are secondary descriptive contrasts and should not be treated as independent replications; several metrics are correlated. Positive values mean the first part of the difference-in-differences is larger; negative values mean lower values under the defined contrast.

## Structural diagnostics

| Condition | Median evidence overlap | Evidence overlap range | Median cut ratio | Cut ratio range | Median graph edges | Median cross-network edges |
|---|---:|---:|---:|---:|---:|---:|
| BFS aligned | 1.00 | 1.00–1.00 | 0.2000 | 0.0967–0.2833 | 300 | 60 |
| BFS reversed | 0.00 | 0.00–0.00 | 0.2000 | 0.0967–0.2833 | 300 | 60 |
| BFS independent-balanced | 0.50 | 0.40–0.62 | 0.2000 | 0.0967–0.2833 | 300 | 60 |
| Random aligned | 1.00 | 1.00–1.00 | 0.5067 | 0.4267–0.5700 | 300 | 152 |
| Random reversed | 0.00 | 0.00–0.00 | 0.5067 | 0.4267–0.5700 | 300 | 152 |
| Random independent-balanced | 0.50 | 0.38–0.62 | 0.5067 | 0.4267–0.5700 | 300 | 152 |

Each network/evidence group had 50 nodes. Structural diagnostics were consistent across variants within each seed. The median cut ratio was approximately 0.20 for BFS and 0.507 for random partitions. This is a major design limitation: topology mode and cut structure are not independently controlled. The observed interaction could depend on cut structure or other correlated graph properties, not solely on BFS versus random topology.

## Censoring and convergence

For Full CR-QISL:
- Agreement-time measurements were right-censored in 100/100 runs in five cells and 97/100 in BFS independent-balanced.
- Quorum-agreement time was censored in 100/100 runs in five cells and 93/100 in BFS independent-balanced.
- Amplitude-convergence time was censored in 100/100 BFS aligned and BFS reversed runs, 47/100 BFS independent-balanced runs, 30/100 random aligned runs, 39/100 random reversed runs, and 0/100 random independent-balanced runs.

The small number of uncensored agreement-time observations does not establish durable final agreement. In particular, only four BFS independent-balanced runs had strict final agreement at the horizon. Censored values must not be described as successful convergence.

## Interpretation

1. **The local-stability interaction replicated** on a new seed range (1001–1100): independent-balanced evidence assignment was associated with a much higher final stable fraction in the BFS partition condition than in the random partition condition.
2. **Global agreement did not replicate as a meaningful general outcome.** Only 4% of BFS independent-balanced Full CR-QISL runs reached strict final agreement; no other condition did.
3. **The structural confound remains unresolved.** BFS cut ratios are much lower than random cut ratios in this design. A follow-up must match or explicitly manipulate cut ratio independently of topology and evidence assignment before attributing the interaction to topology.
4. This remains an exploratory classical simulation. No claim of real-world consensus, fault tolerance, security, safety, finality, or quantum advantage is supported.

## Recommended next experiment

Design a topology/cut-structure controlled study: construct or select graphs so BFS and random partition conditions have overlapping or matched cut-ratio distributions, while keeping evidence assignment independently randomized. Pre-register the primary contrast and continue to report strict final agreement separately from local stable fraction. Include enough seeds to assess heterogeneity and report all structural diagnostics.

## Reproducibility

Raw `runs.csv`, descriptive summaries, paired-test summaries, resolved configs, and trace/event files are retained in the six GitHub Actions artifacts listed above. This audit is based on the raw run tables and structural diagnostics, with the primary factorial interaction recomputed by seed-block bootstrap.
