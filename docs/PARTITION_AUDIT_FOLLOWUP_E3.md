# Partition audit follow-up — Experiment 3

Date: 2026-11-20

## Verified source-level behavior

- Partition membership is generated once per seed using the graph BFS order from node 0 (unless another configured mode is selected), then the first ceil(fraction*n) nodes are group 0.
- Partition is enforced at send time and again at delivery time, for ticks >= start and < end.
- The scenario evidence injections at tick 61 are group-scoped: group 0 receives candidate 0 and group 1 receives candidate 1.
- Observer metrics distinguish the fraction of honest nodes that are locally Stable from global agreement. Stable fraction alone is not a same-candidate agreement metric; final_agreement is separate.

## Trace-level check (seed 1, Full CR-QISL; descriptive, not representative)

| Scenario | Realized groups | Graph edges | Cross-group edges | Stable nodes at tick 299 | Decisions among stable nodes |
|---|---:|---:|---:|---:|---|
| Partition 50% | 50/50 | 300 | 69 | 2 | candidate 0: 2 |
| Partition 80% | 80/20 | 300 | 57 | 100 | candidate 0: 100 |

These trace observations illustrate that the configured group ratio changes not only the number of nodes exposed to each evidence injection but also the realized graph cut. They do **not** explain the across-seed outcome or establish a causal mechanism.

## Across-seed summary (Full CR-QISL, 30 runs)

- Partition 50%: median final stable fraction = 0.045; median final_agreement = 0; all 30 runs had agreement censored at horizon; median post-heal divergence = 0.155971.
- Partition 80%: median final stable fraction = 1.0; median final_agreement = 1.0; 21/30 runs recorded final_agreement=1; median agreement time after heal = 94; no runs reported final_split=1.

## Interpretation / limitation

The 50% and 80% partition scenarios are materially different experimental setups: they change evidence-group sizes and the graph cut induced by the BFS ordering. The surprising across-seed contrast remains unresolved. A further controlled audit should decouple partition membership from evidence assignment, compare random and graph-aware partitioning, and report realized cut ratio and group-conditioned outcomes. Do not treat the 80% scenario as evidence that larger partitions are safer or better.

No claim of protocol security, consensus safety, real-world performance, or superiority is warranted.
