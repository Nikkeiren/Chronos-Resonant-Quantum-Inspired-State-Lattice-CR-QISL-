# Experiment 3 — Robustness audit (600 simulation runs)

Date: 2026-11-20
Workflow run: 44686892539
Commit: 6613eb8e726d1d8c52cd2b93010d001dcd69ef6c

## Scope and caveats
Five conditions, four variants, 30 paired seeds per variant-condition (120 runs per condition; 600 total). Conditions: packet loss 10%, packet loss 30%, latency maximum 10 ticks, partition fraction 50%, partition fraction 80%. These are simulator observations, not protocol guarantees, security claims, or real-world validation. The model remains a classical quantum-inspired simulator. Metrics are observational.

## Descriptive medians: Full CR-QISL
| Condition | Final amplitude spread | Post-heal mean spread | Post-heal divergence | Final stable fraction | Agreement time median |
|---|---:|---:|---:|---:|---:|
| Loss 10% | 0.0857 | 0.1420 | 0.0214 | 0.915 | 160 (censored) |
| Loss 30% | 0.1004 | 0.1512 | 0.0350 | 0.875 | 160 (censored) |
| Latency max 10 | 0.0826 | 0.1237 | 0.0394 | 0.860 | 160 (censored) |
| Partition 50% | 0.1139 | 0.1421 | 0.1560 | 0.045 | 160 (censored) |
| Partition 80% | 0.0754 | 0.1185 | 0.0059 | 1.000 | 94 |

## Full vs Phase Ablation
Paired difference is Phase Ablation minus Full; positive values for spread/divergence indicate lower Full values. Holm-adjusted p values are taken from the artifact CSV and are metric-family-wide, so interpret exploratorily.

| Condition | Final spread difference | p-Holm | Post-heal mean spread difference | p-Holm | Post-heal divergence difference | p-Holm |
|---|---:|---:|---:|---:|---:|---:|
| Loss 10% | 0.00662 | 0.0000442 | 0.01603 | 2.01e-7 | 0.01028 | 0.000503 |
| Loss 30% | 0.01127 | 2.01e-7 | 0.02032 | 2.01e-7 | 0.01818 | 0.000143 |
| Latency max 10 | 0.01804 | 2.01e-7 | 0.02588 | 2.01e-7 | 0.04295 | 2.01e-7 |
| Partition 50% | 0.00872 | 0.0000168 | 0.01575 | 2.01e-7 | 0.01133 | 1 (not significant) |
| Partition 80% | 0.00932 | 2.01e-7 | 0.02004 | 2.01e-7 | 0.00474 | 0.0000195 |

## Interpretation
- Full CR-QISL had lower final spread and lower post-heal mean spread than Phase Ablation in all five tested conditions (paired differences and Holm p-values shown above).
- Post-heal divergence comparison was not statistically distinguishable in the 50% partition condition; other four conditions showed lower Full values in these paired analyses.
- This does not establish causality, generalization, practical superiority, or safety. Weighted Gossip often had lower amplitude spread than Full, so Full is not a universal winner.
- Severe degradation appears in the 50% partition condition: the median final stable fraction for Full was 0.045. Conversely, the 80% partition artifact shows median final stable fraction 1.0 and agreement time 94. This non-monotonic and counterintuitive result is a priority for an independent configuration/partition-generator audit; do not explain it away or treat it as a reliable effect until verified.
- Agreement-time censoring remains high in several conditions, and amplitude convergence remained censored at the horizon in all Full variants in these robustness runs.

## Priority next action
Before any prototype implementation claim, audit the partition generation, partition labels/actual realized component sizes, scenario-to-artifact mapping, and the precise definition/export of final_stable_fraction and agreement time. Then produce a reproducible pooled report with all conditions and caveats. Do not launch another broad simulation grid until this issue is resolved.
