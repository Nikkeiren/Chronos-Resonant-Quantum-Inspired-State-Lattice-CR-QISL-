# Independent Evidence Recipient Experiment — Audit

Run: 44712016218
Commit under test: 98c834788188545f56ef89e691d0dc8340605aa0
Date: 2026-11-20
Status: all six jobs passed; artifacts downloaded and inspected; results exploratory.

## Execution integrity

All six matrix jobs completed successfully:
- BFS-ALIGNED
- BFS-REVERSED
- BFS-INDEPENDENT-BALANCED
- RANDOM-ALIGNED
- RANDOM-REVERSED
- RANDOM-INDEPENDENT-BALANCED

Every job passed unit tests, config validation, the paired-seed batch, and artifact upload. Each artifact contains 120 runs (4 variants x 30 seeds), for 720 completed runs total. No run rows reported a non-ok status in the inspected outputs.

## Design/trace audit

The resolved configs contain the intended assignment modes and network partition modes. Trace files expose both `network_group` and `evidence_group`; the legacy `group` field matches `network_group`.

For the four exported trace seeds (1, 10, 20, 30):
- BFS network partitions: mean cut ratio 0.204 (range 0.133–0.253).
- Random network partitions: mean cut ratio 0.514 (range 0.487–0.537).
- Aligned assignments have 100% label overlap; reversed assignments have 0%; independent-balanced assignments have mean overlap 0.435 under BFS and 0.490 under random.
- Both labels have 50/50 group sizes in these traces.
- Independent evidence labels are generated from a separate RNG stream and are the same for the same seed across BFS and random network modes. Thus network partition formation does not consume the evidence-assignment RNG stream.

These group/cut diagnostics are based on the four trace seeds, not all 30 seeds. Do not present them as 30-seed estimates.

## Full CR-QISL descriptive outcomes

Medians across 30 seeds:

| Condition | Final stable fraction | Post-heal divergence | Final amplitude spread | Post-heal mean spread | Conflict entries | Messages sent |
|---|---:|---:|---:|---:|---:|---:|
| BFS aligned | 0.045 | 0.1560 | 0.1139 | 0.1421 | 381.5 | 52,308.5 |
| BFS reversed | 0.020 | 0.1361 | 0.1116 | 0.1451 | 410.5 | 52,970 |
| BFS independent | 0.660 | 0.0000 | 0.0167 | 0.0311 | 0 | 26,114.5 |
| Random aligned | 0.140 | 0.0157 | 0.0145 | 0.0361 | 43 | 34,221.5 |
| Random reversed | 0.100 | 0.0179 | 0.0187 | 0.0339 | 44.5 | 33,481 |
| Random independent | 0.075 | 0.0000 | 0.0054 | 0.0117 | 0 | 26,042 |

Critical distinction:
- Strict final agreement occurred in 0/30 runs in five conditions.
- BFS independent-balanced had final agreement in only 2/30 runs.
- Agreement time and quorum-agreement time were censored in all 30 runs for five conditions and 28/30 runs for BFS independent-balanced.
- Therefore, the high BFS independent stable-fraction median (0.66) is NOT global agreement or consensus. It means more nodes locally entered a stable state, not that they all chose the same candidate.
- Amplitude-convergence time was also censored for many runs; consult the per-condition descriptive tables before using those times.

## Planned paired comparisons — exploratory

For Full CR-QISL, paired by seed. Initial two-sided Wilcoxon comparisons used nine planned contrasts per metric, with Holm correction across those nine contrasts within each metric. These are exploratory comparisons, not a substitute for a pre-registered factorial model or independent replication.

Final stable fraction:
- BFS aligned vs BFS independent: median 0.045 vs 0.660; Holm p approximately 0.00007.
- BFS reversed vs BFS independent: 0.020 vs 0.660; Holm p approximately 0.00002.
- Random aligned vs random independent: 0.140 vs 0.075; Holm p approximately 0.1405.
- Random reversed vs random independent: 0.100 vs 0.075; Holm p approximately 0.1757.
- BFS independent vs random independent: 0.660 vs 0.075; Holm p approximately 0.00002.

The same contrast structure showed large differences in amplitude spread, post-heal divergence, and message counts. The striking BFS-independent outcome should be treated as a hypothesis-generating interaction between topology and evidence placement, not proof of a general mechanism. The random-network independent assignment did not reproduce the same stable-fraction increase.

## Interpretation and next work

1. This run supports the claim that the relationship between evidence placement and network partition membership materially changes simulator outcomes under these settings.
2. It does not isolate a universal causal effect of "independence": BFS and random network partitions have very different cut ratios, and the evidence-group overlap distributions differ.
3. The 50/50 balanced independent evidence assignment is randomized but constrained to match the network group size. That constraint must remain explicit.
4. The strongest apparent effect (BFS independent) has only 2/30 strict final-agreement runs. Do not describe it as consensus success.
5. The simulator remains a classical quantum-inspired model. These results do not establish quantum advantage, real-world consensus safety, Byzantine tolerance, or finality.

Recommended follow-up:
- Add an explicit factorial analysis with network partition method, evidence assignment mode, and their interaction as fixed factors, with seed as a paired/blocking factor.
- Report all outcomes separately: local stable fraction, strict final agreement, split, divergence, amplitude spread, and communication cost.
- Add a diagnostic export for network/evidence group contingency and cut ratio for every seed, not only trace seeds.
- Replicate the strongest contrast with additional independent seeds and vary graph/topology parameters before making broader claims.
