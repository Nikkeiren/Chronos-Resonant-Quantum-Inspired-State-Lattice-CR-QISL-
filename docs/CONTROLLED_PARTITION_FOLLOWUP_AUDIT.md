# Controlled Partition Follow-up — Audit

Run: [44710701943](https://github.com/Nikkeiren/Chronos-Resonant-Quantum-Inspired-State-Lattice-CR-QISL-/actions/runs/44710701943)  
Commit: `0d51a08ad1408abf0629e73b8b982261582a579c`  
Date: 2026-11-20

## Execution integrity

All four matrix jobs completed successfully:

- P50-BFS
- P50-RANDOM
- P80-BFS
- P80-RANDOM

For every job, configuration validation, paired-seed batch execution, and artifact upload succeeded. Each artifact contains 120 runs (four variants × 30 seeds), for 480 simulations total. No failed runs were reported. The four artifacts were downloaded and their analysis reports, resolved configurations, paired tests, and selected traces were inspected.

The experiment uses 100 nodes, two candidates, a connected small-world graph (k=6, p=0.1), 20% packet loss, latency 1–5 ticks, partition ticks 60–139, evidence injection at tick 61, and a 300-tick horizon. The two evidence injections target group 0/candidate 0 and group 1/candidate 1. No adversarial nodes are enabled.

## Main finding: BFS and random do not create equally severe partitions

The graph is held constant by seed across the partition modes. In `src/sim.rs`, graph generation uses RNG stream 1 and partition assignment uses independent stream 2. `src/topology.rs::partition_groups` defines:

- `bfs`: take the first requested number of nodes in BFS order from node 0;
- `random`: shuffle all node IDs and take the requested number.

Thus the two modes preserve group sizes but create very different graph cuts.

For the four exported trace seeds (1, 10, 20, 30), each graph had 300 edges:

| Condition | Group sizes | Cross-group edges, mean | Cut ratio, mean |
|---|---:|---:|---:|
| P50-BFS | 50 / 50 | 61.25 / 300 | 0.204 |
| P50-RANDOM | 50 / 50 | 154.25 / 300 | 0.514 |
| P80-BFS | 80 / 20 | 47.25 / 300 | 0.158 |
| P80-RANDOM | 80 / 20 | 95.75 / 300 | 0.319 |

The BFS groups are much more spatially clustered and therefore have fewer cross-group links. The random groups leave substantially more cross-group links. During a partition, cross-group messages are dropped, so the random conditions are materially less isolating even though their nominal group fractions are identical.

This is the key interpretation constraint: **P50-BFS versus P50-RANDOM and P80-BFS versus P80-RANDOM are not merely alternate labelings of the same partition severity.** They vary the realized graph cut as well as which nodes receive each competing evidence injection.

## Full CR-QISL descriptive results

Medians over 30 seeds:

| Condition | Final amplitude spread | Post-heal mean spread | Post-heal divergence | Final stable fraction | Agreement time after heal |
|---|---:|---:|---:|---:|---:|
| P50-BFS | 0.1139 | 0.1421 | 0.1560 | 0.045 | 160 (30/30 censored) |
| P50-RANDOM | 0.0145 | 0.0361 | 0.0157 | 0.140 | 160 (30/30 censored) |
| P80-BFS | 0.0754 | 0.1185 | 0.0059 | 1.000 | 94 (10/30 censored) |
| P80-RANDOM | 0.0223 | 0.0424 | 0.0000 | 1.000 | 24.5 (0/30 censored) |

Agreement time is right-censored at 160 ticks after the partition heals. A value of 160 does not mean agreement occurred at tick 160; it means the metric was not reached within the observation window. This applies to all four variants in both P50 conditions.

## Variant comparisons

Paired tests are within each condition, matched by seed, with Holm correction over the analysis family.

### Full CR-QISL versus Phase Ablation

Positive median difference below means Phase Ablation minus Full CR-QISL, so positive values favor Full on metrics where lower is better.

| Condition | Final spread difference (Holm p) | Post-heal mean spread difference (Holm p) | Divergence difference (Holm p) |
|---|---:|---:|---:|
| P50-BFS | +0.00872 (0.0000168) | +0.01575 (2.01e-7) | +0.01133 (1.000; not significant) |
| P50-RANDOM | +0.00382 (0.000226) | +0.00786 (4.25e-7) | +0.02357 (4.25e-7) |
| P80-BFS | +0.00932 (2.01e-7) | +0.02004 (2.01e-7) | +0.00474 (0.0000195) |
| P80-RANDOM | +0.00038 (0.000450) | +0.00164 (2.01e-7) | 0.00000 (0.641; not significant) |

Full CR-QISL has lower amplitude spread than Phase Ablation in all four conditions, although the effect is small in P80-RANDOM. The divergence result is not consistent across all conditions: no significant difference in P50-BFS or P80-RANDOM.

### Important baseline qualification

Weighted Gossip has the lowest median amplitude spread in all four conditions. Full CR-QISL should therefore not be described as the overall amplitude-convergence winner.

The Full variant does show better selected stability/agreement outcomes in some conditions:

- P50-BFS: Full median stable fraction 0.045 versus 0.030 for Phase Ablation; not statistically distinguishable after correction. All variants' agreement times are censored.
- P50-RANDOM: Full median stable fraction 0.140 versus 0.045 for Phase Ablation; not statistically distinguishable after correction. All variants' agreement times are censored.
- P80-BFS: all variants have median stable fraction 1.0, but Full has median agreement time 94 (10 censored) versus 138 (14 censored) for Phase Ablation. The paired agreement-time comparison is significant after correction (median paired difference +10 ticks for Phase Ablation minus Full; Holm p=0.00324).
- P80-RANDOM: Full and Phase Ablation both have median stable fraction 1.0 and similar agreement times (24.5 versus 25; no significant difference).

These are simulation-specific, observer-level measurements, not evidence of consensus safety, finality, Byzantine tolerance, or real-world protocol performance.

## Interpretation

1. The earlier contrast between 50% and 80% partition outcomes cannot be explained by partition fraction alone.
2. The BFS/random switch changes realized cut size substantially. The random conditions retain about twice as many cross-group edges as BFS in the four inspected traces.
3. Lower cut ratios appear associated with stronger persistence of competing states and slower or censored agreement in this experiment. This is a descriptive association, not a causal proof by itself.
4. The 80%-BFS result is not equivalent to 80%-RANDOM: the latter is much more permeable across groups and shows faster agreement.
5. The evidence injections are still coupled to the network partition labels. This prevents clean separation of (a) graph-cut severity and (b) alignment between evidence recipients and partition membership.

## Required next experiment

Add an independent evidence-recipient assignment, separate from the partition membership vector used by the network to drop cross-group messages.

At minimum, cross:

- partition geometry: BFS versus random;
- evidence assignment: aligned with partition groups versus independently assigned recipients.

Keep graph, seed, packet loss, latency, partition interval, injection tick/strength/phases, and variant set fixed. Record both vectors in the trace and export realized cross-edge counts/cut ratio for every seed, not only selected trace seeds. This will isolate whether outcomes are driven mainly by graph-cut severity, evidence/partition alignment, or their interaction.

Do not make protocol-level claims from the current run. The defensible conclusion is narrower: in this simulator, the realized graph cut and the coupling of evidence assignment to partition groups strongly condition the observed post-heal behavior.
