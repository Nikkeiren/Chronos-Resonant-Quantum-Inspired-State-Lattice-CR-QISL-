# Confirmatory Replication Protocol — Evidence Placement × Network Partition

Status: protocol committed before execution of the confirmatory batch.  
Date: 2026-11-20  
Prior exploratory source: run 44714090270, commit 059d9d1cb6327f10af3fac281454502fff256628.

## Research question

Does the effect of evidence-recipient assignment (aligned, reversed, or independent-balanced relative to the network partition) differ between BFS and random network partitions in this simulator?

This is a confirmatory replication of the previously observed interaction pattern, not a test of real-world consensus safety or quantum advantage.

## Design

A 2 × 3 factorial design:

- Network partition method: BFS, RANDOM.
- Evidence assignment: ALIGNED, REVERSED, INDEPENDENT_BALANCED.

Shared settings:
- 100 nodes; 2 candidates.
- Small-world graph, k=6, rewiring p=0.1, connected graph required.
- Network partition fraction 0.5; partition active ticks 60 through 139 (end-exclusive 140).
- Evidence injected at tick 61; group 0 receives candidate 0 at strength 0.8, phase 0; group 1 receives candidate 1 at strength 0.8, phase π/2.
- Packet loss 20%; uniform latency 1–5 ticks.
- Horizon 300 ticks.
- Seeds 1001–1100 (100 new seed blocks, not reused from the exploratory 1–30 batch).
- Four variants per seed: weighted_gossip, weighted_gossip_damping, phase_ablation, full_cr_qisl.

Total planned simulations: 6 cells × 100 seeds × 4 variants = 2,400.

The independent-balanced evidence assignment is size-constrained to the 50/50 network group size and generated from its independent RNG stream. It is not an unconstrained assignment over all possible label vectors.

## Outcomes and analysis commitments

Primary outcome:
- Final stable fraction, analyzed at the seed level for each cell and variant.

Key distinct outcome:
- Strict final agreement rate (all honest nodes stable on the same candidate). This must never be conflated with local stable fraction.

Secondary outcomes:
- Final split, post-heal divergence, final amplitude spread, post-heal mean amplitude spread, conflict entries, messages sent, agreement-time censoring, quorum-time censoring, amplitude-convergence censoring.

Primary contrast:
- For Full CR-QISL, difference-in-differences:
  (BFS independent-balanced − mean(BFS aligned, BFS reversed))
  − (RANDOM independent-balanced − mean(RANDOM aligned, RANDOM reversed)).
- Report the estimate and seed-block bootstrap 95% confidence interval. Use the 100 paired seed blocks; resample whole seed blocks, preserving all six cells together.
- Also report all six cell estimates and the raw per-seed data.

Multiplicity and interpretation:
- Treat the interaction on final stable fraction as the single primary inferential contrast.
- Strict final agreement is reported separately as a key endpoint with counts and interval; do not replace it with stable fraction.
- Secondary metrics are descriptive/exploratory, with multiplicity correction if formal tests are added.
- Do not treat correlated secondary metrics as independent replications.
- Report missingness/censoring explicitly; do not convert censored times to successful convergence.
- Report structural diagnostics (evidence overlap, contingency counts, group sizes, graph edges, cross-group edges, cut ratio) for all seeds.

## Important design limitation

BFS and random partitions are expected to have different cut-ratio distributions. This experiment measures those differences for every seed, but does not force cut-ratio matching. Therefore, even a replicated interaction does not isolate topology method from cut structure. Any cut-ratio-adjusted model is secondary and observational because cut ratio is a generated structural property, not randomized independently here.

## Decision rule

A replicated positive interaction in final stable fraction supports repeating the investigation with a design that independently controls or matches cut ratio. It does not establish global consensus: strict final agreement must be considered on its own. If the interaction is weak or unstable, report the exploratory result as not replicated under this seed range.

## Reproducibility

Configs are in `configs/confirmatory_evidence_*.toml`. Workflow: `.github/workflows/confirmatory_evidence_placement.yml`. Run manually via GitHub Actions. Artifacts must be retained and audited before conclusions are written.
