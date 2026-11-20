# Cut-Controlled Evidence Placement Follow-up Protocol

Status: committed before execution.  
Purpose: test whether the previously observed evidence-placement interaction depends on the arbitrary BFS start node / partition boundary, rather than treating BFS and unconstrained random partitions as if they had matched cut structure.

## Design

Six conditions form a 2 × 3 design:

- Partition construction: fixed-root BFS (root node 0) versus randomized-root BFS (root selected deterministically from an independent partition RNG stream for each seed).
- Evidence assignment: aligned, reversed, independent-balanced.

All conditions use the same graph generator and same seed range. The graph RNG stream is unchanged. The fixed-root and randomized-root conditions therefore share the same graph for each seed; only the partition root differs. Both partitions are balanced 50/50. The randomized-root BFS is a **cut-structure control**, not an unconstrained random node assignment.

Parameters: 100 nodes, 2 candidates, Watts–Strogatz small-world k=6, p=.1, connected; 20% packet loss; latency 1–5 ticks; partition active ticks 60–139; horizon 300; evidence injected at tick 61 with group 0 receiving candidate 0 (strength .8, phase 0) and group 1 candidate 1 (strength .8, phase π/2). Four simulator variants. Seeds 2001–2100.

Total: 6 conditions × 100 seeds × 4 variants = 2,400 simulations.

## Outcomes

Primary descriptive contrast: the difference in the independent-balanced versus mean(aligned, reversed) final stable fraction between fixed-root BFS and randomized-root BFS. Report seed-paired bootstrap 95% interval.

Strict final agreement rate is a separate key outcome and must never be conflated with local stable fraction. Secondary outcomes: final split, post-heal divergence, final and post-heal mean amplitude spread, conflict entries, messages, censoring rates, graph edge count, cross-network edge count, cut ratio, and evidence overlap.

## Structural validation

For each seed, compare fixed-root and randomized-root partitions on the exact same graph. Report cut-ratio paired differences, distribution overlap, cross-edge counts, group sizes, and evidence overlap. Do not call the control “cut matched” unless the observed cut-ratio distributions show adequate overlap. If randomized-root BFS does not provide overlap, treat this as a failed matching design and do not interpret the outcome contrast as isolating partition boundary effects.

The independent-balanced evidence assignment uses the separate evidence RNG stream; it must remain independent of network group labels. Diagnostics should be consistent across four variants within each seed.

## Analysis safeguards

- Preserve matched seed blocks in bootstrap resampling.
- Report all six cells and all 100 seeds.
- Keep final stable fraction and strict final agreement separate.
- Report censored time-to-event outcomes as censored, not as completed convergence.
- The primary contrast is descriptive/inferential for this simulator only; no claims of real-world consensus, safety, Byzantine tolerance, finality, or quantum advantage.
- This design controls the graph realization and randomizes the BFS root, but it does not establish a general topology effect.

## Execution

Workflow: `.github/workflows/cut_controlled_evidence_placement.yml`.  
Manual trigger via GitHub Actions `workflow_dispatch`.  
Each matrix job runs tests, validates its config, executes 400 simulations, and uploads a 90-day artifact.
