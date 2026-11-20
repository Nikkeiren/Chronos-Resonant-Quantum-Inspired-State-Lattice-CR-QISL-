# Independent evidence-recipient experiment — protocol

Date: 2026-11-20
Status: initial implementation committed; CI validation and experiment results pending.

## Motivation

The controlled partition follow-up (P50/P80 x BFS/random) still applies candidate evidence according to the same group labels that gate network communication. Consequently, network partition membership and evidence-recipient assignment are coupled. A difference between conditions cannot cleanly distinguish a communication-topology effect from an evidence-placement effect.

This experiment introduces a second, independently generated node label: `evidence_group`. Network communication continues to use `network_group`; scheduled evidence injection uses `evidence_group`. They must be stored and exported separately.

## Research question

When network partition membership is held fixed, how do evidence placement and its alignment with the network partition affect local stability, global agreement, and post-heal dynamics?

## Proposed design

Use a fixed nominal 50/50 network partition and a 2 x 3 factorial design:

| Factor | Levels |
|---|---|
| Network partition assignment | BFS, RANDOM |
| Evidence recipient assignment | ALIGNED, REVERSED, INDEPENDENT_BALANCED |

Definitions:

- **ALIGNED**: evidence_group equals network_group for each node. This reproduces the current coupled assignment.
- **REVERSED**: evidence_group is the complement of network_group. Candidate evidence remains attached to evidence_group 0/1, so the mapping is deliberately swapped relative to network membership.
- **INDEPENDENT_BALANCED**: generate a separate, seed-deterministic, balanced random assignment of evidence_group labels, independently of network_group. Preserve the same group-0 count as the network partition to avoid changing evidence-group sizes.

For every condition, inject candidate 0 to evidence_group 0 and candidate 1 to evidence_group 1 at tick 61, with the same strength and phase values used in the controlled follow-up. Network messages are still gated only by network_group.

## Fixed simulation settings

- 100 nodes, 2 candidates.
- Small-world graph, k=6, p=0.1, ensure connected.
- Network partition fraction 0.5; partition active on ticks 60 through 139 (existing end-exclusive semantics).
- Packet loss 20%; uniform latency 1–5 ticks.
- Horizon 300; seeds 1–30.
- Four variants: weighted_gossip, weighted_gossip_damping, phase_ablation, full_cr_qisl.
- Trace seeds: 1, 10, 20, 30.
- Total: 6 conditions x 4 variants x 30 seeds = 720 simulations.

Use independent deterministic RNG streams for network partition assignment and evidence assignment. Do not let generating evidence labels consume or perturb random numbers used for graph generation, network behavior, or other setup components.

## Required implementation semantics

1. Preserve `Setup.groups` as the network partition membership used by message gating.
2. Add `Setup.evidence_groups`, generated from an explicit config setting and a separate RNG stream.
3. Scheduled evidence events with group 0/1 must filter against `evidence_groups`, not `groups`.
4. Group -1 must continue to mean all honest nodes.
5. Include both `network_group` and `evidence_group` in trace output. Keep the old `nodes.group` field as a compatibility alias for network_group if existing viewer code depends on it.
6. Log evidence injections with the evidence-group label and document this meaning.
7. Add deterministic unit tests for all assignment modes, group sizes, and independence from network-group labels.
8. Default the new setting to ALIGNED so existing experiments remain behaviorally unchanged.
9. Validate unknown modes and ensure all assignments are binary and contain both groups.

## Required outputs and analyses

For every seed/condition:
- realized network-group and evidence-group sizes;
- graph edge count, cross-network-group edges, cut ratio, and internal edges;
- evidence/network label overlap (agreement rate), and contingency table;
- final stable fraction and final same-candidate agreement as separate outcomes;
- final split, agreement-time censoring, amplitude-convergence censoring;
- post-heal divergence and amplitude-spread diagnostics;
- per-variant paired comparisons by seed, effect sizes, confidence intervals where available, and Holm correction within declared metric families.

Primary contrasts:
1. At each network assignment method, compare ALIGNED, REVERSED, and INDEPENDENT_BALANCED evidence assignment.
2. At each evidence assignment mode, compare BFS and RANDOM network partition assignment.
3. Test whether evidence/network label overlap and cut ratio descriptively track outcomes. Treat these as exploratory associations, not causal proof.

## Interpretation guardrails

- Stable fraction is not equivalent to global agreement.
- A result from one seed or trace is illustrative only.
- Agreement and convergence times censored at tick 300 are not observed completion times.
- The simulator is classical and quantum-inspired. These experiments do not establish quantum advantage, real-world consensus safety, Byzantine tolerance, or finality.
- The independent balanced assignment is independent in its randomization stream, but its group-size constraint intentionally matches the network partition sizes. State this when interpreting results.

## Exit criteria before running

Do not launch the workflow until:
- the code compiles and all tests pass;
- default ALIGNED mode reproduces the previous controlled-partition behavior for fixed seeds;
- trace output exposes both labels;
- all six config files validate;
- the workflow command is exercised successfully at least once.
