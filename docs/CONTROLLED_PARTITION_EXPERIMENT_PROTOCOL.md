# Controlled partition experiment — follow-up to E3

Date: 2026-11-20
Status: protocol specification; results pending.

## Research question

Does the E3 contrast between partition fractions 0.50 and 0.80 persist when partition membership is assigned independently of graph traversal structure?

The current implementation couples three things: (1) partition membership, (2) which nodes receive candidate-0 versus candidate-1 evidence, and (3) the graph cut induced by the membership assignment. This experiment changes the partition assignment method while preserving the group fraction and the evidence-to-group rule.

## Design

Use a 2 x 2 condition grid:

| Condition | partition_fraction | partition_mode |
|---|---:|---|
| P50-BFS | 0.50 | bfs |
| P50-RANDOM | 0.50 | random |
| P80-BFS | 0.80 | bfs |
| P80-RANDOM | 0.80 | random |

Keep all other settings identical to E3 partition conditions: 100 nodes, 2 candidates, small-world topology (k=6, p=0.1, connected), partition ticks 60–140, evidence injections at tick 61 (group 0 -> candidate 0, strength 0.8, phase 0; group 1 -> candidate 1, strength 0.8, phase pi/2), packet loss and latency as in the E3 baseline, horizon 300, seeds 1–30, and all four protocol variants.

This is 4 conditions x 4 variants x 30 seeds = 480 runs. Use the same seed set across conditions and variants. Preserve trace output for a small, predeclared subset (for example seeds 1, 10, 20, 30) to audit group membership and dynamics without creating unnecessarily large artifacts.

## Required checks before interpreting outcomes

For every seed and condition, compute and report:
- realized group sizes;
- graph edge count;
- cross-group edge count and cut ratio = cross-group edges / all edges;
- each group's internal-edge count;
- stable fraction and final agreement, separately;
- final split, agreement-time censoring, amplitude-convergence censoring;
- post-heal divergence and amplitude spread.

Do not compare only the configured partition fraction. The realized graph cut is a key explanatory variable. Report distributions across seeds, not only one illustrative trace.

## Primary contrasts

1. At fixed fraction, compare BFS versus random assignment. This estimates sensitivity to how the partition is embedded in the graph, while retaining the same nominal group sizes and evidence assignment rule.
2. Within each assignment method, compare 50% versus 80%. Interpret this as a combined group-size / evidence-allocation contrast, not as a pure "partition severity" contrast.
3. Explore whether cut ratio and group-conditioned stable decisions track final agreement. Treat this as descriptive unless a pre-specified causal analysis is added.

Pair all comparisons by seed. Apply Holm correction within clearly defined metric families. Include effect sizes and confidence intervals where available. Do not treat p-values as proof of mechanism.

## Important limitations

- Random partition assignment is still coupled to evidence assignment because evidence is delivered by partition group. This design isolates BFS/graph-structure effects better than E3, but it does not independently randomize evidence recipients.
- The simulator uses a classical, quantum-inspired state update and heuristic decision/reconciliation rules. These results do not establish real-world consensus safety, Byzantine tolerance, finality, or quantum advantage.
- A high stable fraction is not equivalent to global same-candidate agreement.
- Agreement and amplitude-convergence times may be right-censored at the simulation horizon. Never report censored values as observed completion times.

## Next design if the contrast remains ambiguous

Add a separate evidence-recipient assignment vector, independent of network partition groups, with explicit modes such as aligned, reversed, balanced-random, and all-nodes-same-evidence. This requires a code change and should be tested separately from the present configuration-only experiment to avoid changing multiple mechanisms at once.
