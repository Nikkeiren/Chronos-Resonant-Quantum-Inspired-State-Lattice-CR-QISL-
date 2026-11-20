# Limitations

* **No guarantees.** No safety, Byzantine-fault tolerance, liveness or finality property is proven or claimed.
  `Stable` is a per-node, local, revocable decision. `agreement_*` metrics are observer-level.
* **Assumed mathematics.** Aggregation, resonance, damping, integrity weighting and the decision rule are assumptions
  (docs/ASSUMPTIONS.md). A different true specification can reverse every comparison reported here.
* **Integrity filter is a heuristic.** Median-based outlier scoring penalises honest divergent neighbours, is defeated
  by a local adversarial majority, and can be gamed (e.g. an adversary that reports plausible values). The adversarial
  scenarios record outcomes; they do not demonstrate protection.
* **Unauthenticated protocol.** No signatures (`signature_ops` = 0). Witnesses and decision claims are forgeable; an
  adversary can in principle trigger `CONFLICT_PENDING` storms (`adversary.claim_decision = true` models a simple case).
* **No retransmission layer** (`retransmissions` = 0); loss is repaired only by later gossip and heartbeats.
* **Scheduled evidence.** Competing states arise from configured injections, not from an application workload;
  conclusions depend on injection strength, timing and partition sizes.
* **Small model of time and networks.** Tick-based, one hop latency model, independent loss, no congestion,
  no clock drift, a single partition with two sides.
* **Statistics.** 30 seeds give limited power; p-values are about simulator outcomes under assumptions, not real
  systems. Holm correction covers the reported family only. Strict agreement is frequently right-censored at 300 ticks.
* **Observer metrics.** Divergence and agreement use global knowledge. Divergence counts only nodes in `Stable`
  (nodes that are tentative/pending are not counted as diverged).
* **Verification status.** The Rust code had not been compiled when this package was produced (see README).
* **Viewer.** Renders per-node frames of one run; layout is a force-directed drawing, not a geographic map.
  For very large n the O(n²) layout takes noticeable time.
