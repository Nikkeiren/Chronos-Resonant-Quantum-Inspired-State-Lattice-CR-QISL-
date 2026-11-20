# CR-QISL — Consolidated Exploratory Research Report

**Status:** Exploratory simulation study; not a validated distributed protocol.  
**Scope:** Classical simulation of a quantum-inspired state-lattice coordination model. No quantum hardware or quantum computation is used.

## Executive summary

This project investigated how evidence placement, network partition structure, communication disruptions, and phase-related mechanisms correlate with simulated node-state coordination in CR-QISL and comparator variants. The results are entirely simulation-bound.

Across repeated batches, the most reproducible pattern was that **independent-balanced evidence recipient placement was associated with a higher final stable-node fraction in the BFS-based partition conditions**, compared with aligned or reversed evidence placement. This pattern replicated across a 30-seed exploratory batch and a 100-seed confirmatory batch, and was observed again in a randomized-root BFS follow-up. However, the design still has topology and partition-structure limitations, and the outcome is a local stable fraction—not global consensus, safety, finality, or real-world reliability.

The study does not establish a quantum advantage, Byzantine fault tolerance, secure consensus, or suitability for deployment.

## Research question

How do evidence-recipient placement and network partition structure interact with the simulated CR-QISL coordination dynamics and comparator variants, as measured by stability, disagreement, amplitude diagnostics, conflicts, and communication?

## Model and implementation overview

- 100 simulated nodes and two candidate states.
- Small-world network configurations used in the documented experiments, generally k=6 and rewiring probability p=0.1.
- Simulation horizon: 300 ticks in the principal experiment batches.
- Evidence events and partitions are simulation inputs.
- State amplitudes are classical numerical values normalized per node; phase is a classical angular variable.
- Complex-valued calculations are numerical constructs in a classical simulator.
- Comparators included Weighted Gossip, Weighted Gossip with Damping, Phase Ablation, and Full CR-QISL.
- No hardware quantum process, physical quantum state, or quantum advantage is measured.

## Mechanism notes and code audit

The audited implementation updates node amplitudes using a weighted neighborhood average and a phase-aware neighborhood quantity, blends toward a target state, and applies normalization. The phase ablation disables the phase-related component. The integrity/trust mechanism is heuristic: neighbor deviations affect weights; it is not an adversarially robust guarantee.

Conflict and witness/reconciliation behavior is simulated. Witness messages are not authenticated, and reconciliation is not a forced global-finality mechanism. The model has not been shown to withstand Byzantine behavior, forged witnesses, malicious local majorities, or realistic security threats.

The convergence diagnostic uses a spread threshold and consecutive-tick criterion. Continuous amplitude-spread diagnostics were added because binary convergence-time thresholds censor information and can obscure non-monotonic behavior.

## Experiment sequence

### Experiment 1 — Baseline comparison
- 100 nodes, two candidates, 30 seeds, four variants.
- 20% packet loss, latency 1–5 ticks, partition ticks 60–140, horizon 300.
- Evidence strength 0.8, with distinct candidate/phase assignments across network groups.
- 120 simulations.

Baseline descriptive results indicated differences across stable fraction, divergence, conflict counts, and communication. Full CR-QISL did not uniformly outperform all comparators. Quorum/agreement-time estimates were heavily censored, and inference based on uncensored subsets was sparse.

### Experiment 2 — Continuous amplitude diagnostics and statistical audit
Continuous diagnostics included final amplitude spread and post-heal mean/minimum spread. Pairwise seed-matched tests were implemented with Wilcoxon signed-rank procedures, effect-size summaries, and Holm correction.

Weighted Gossip had lower spread than Full CR-QISL in the reported comparisons. Full CR-QISL was not an overall winner. The amplitude diagnostics also showed that decision stability and amplitude homogeneity are distinct properties.

### Experiment 3 — Robustness conditions
Five conditions were tested: packet loss 10%, packet loss 30%, latency maximum 10 ticks, partition 50%, and partition 80%, across 30 seeds, four variants (600 simulations).

Full CR-QISL's final and post-heal spread were descriptively lower than Phase Ablation in several conditions, but results were not uniformly better across metrics. Agreement and convergence censoring remained substantial. Partition outcomes were not monotonic in partition severity, motivating follow-up audits.

### Partition-structure audits
Audits established that partition construction and graph cut structure matter. BFS-derived groups and random groups differed materially in cut ratio. Therefore partition size alone did not isolate the structural effect.

A controlled partition follow-up compared 50% and 80% partition fractions with BFS and random grouping. The results were sensitive to grouping method, and partition-related effects could not be interpreted without structural diagnostics.

### Evidence recipient placement — exploratory batch
Six conditions combined BFS or random grouping with aligned, reversed, or independent-balanced evidence placement. The 30-seed batch included 720 simulations.

Independent-balanced evidence placement was associated with higher final stable fractions in the BFS cells for Full CR-QISL, while the random-group cells showed a different pattern. Diagnostics identified cut-ratio differences between BFS and random groups, a material confound for comparisons across topology modes.

### Confirmatory evidence-placement replication
A 100-seed confirmatory batch (seed range 1001–1100) tested six evidence-placement/grouping cells, four variants, and 2,400 simulations.

Full CR-QISL median final stable fractions:
- BFS aligned: 0.050
- BFS reversed: 0.030
- BFS independent-balanced: 0.660
- Random aligned: 0.110
- Random reversed: 0.080
- Random independent-balanced: 0.040

In the seed-block bootstrap factorial analysis, the evidence-placement × grouping interaction for final stable fraction was +0.48130 (95% interval [0.41885, 0.54345]). This is a batch-specific, model-specific estimate. Other metrics were secondary and correlated. Global agreement remained rare, and agreement/quorum-time censoring was substantial.

The network-cut diagnostics showed differing cut ratios between BFS and random grouping modes, so these comparisons do not isolate a pure evidence-placement effect across topology modes.

### Randomized-root BFS follow-up
A six-cell, 100-seed follow-up (2,400 simulations) varied fixed-root versus randomized-root BFS grouping alongside evidence placement. This is a randomized-root BFS comparison, **not a direct cut-ratio-matched experiment**.

Full CR-QISL median final stable fraction:
- Fixed BFS, aligned: 0.090
- Fixed BFS, reversed: 0.235
- Fixed BFS, independent-balanced: 0.650
- Randomized-root BFS, aligned: 0.030
- Randomized-root BFS, reversed: 0.035
- Randomized-root BFS, independent-balanced: 0.620

The independent-balanced pattern persisted in both root modes, while other cells varied. This adds replication of the association but does not resolve all structural confounding or prove a general mechanism.

## Interpretation

The evidence supports a narrow descriptive observation: within these simulation configurations, independent-balanced evidence recipient placement was repeatedly associated with a larger final stable-node fraction in BFS-based conditions for Full CR-QISL.

It does **not** establish:
- global consensus or guaranteed agreement;
- safety, liveness, security, or Byzantine fault tolerance;
- a general causal law;
- superiority over all comparators;
- quantum advantage;
- real-world applicability.

A stable-node fraction is a local/model-defined outcome. A high fraction can coexist with failure to achieve strict global agreement.

## Limitations

1. Simulation-only evidence and a narrow model.
2. Results depend on topology construction, cut ratio, partition assignment, timing, parameter choices, and seed range.
3. Multiple correlated outcomes and exploratory analysis increase interpretive risk.
4. Censoring affects agreement, quorum, and amplitude convergence.
5. No realistic adversarial, authentication, privacy, security, or deployment tests.
6. No external empirical system validation.
7. The confirmatory replication validates a pattern within the simulator and tested operating points only.
8. Randomized-root BFS is not equivalent to cut-ratio matching.

## Recommended next step

Stop expanding simulation batches temporarily. Consolidate the existing work, preserve scripts/configurations/artifacts and audit notes, and conduct an independent review of the report, experiment design, statistical code, and implementation claims. Any future experiment should address one explicitly stated unresolved confound and be preregistered or documented before execution.

## Research integrity statement

All claims in this report are scoped to the classical simulator and reported experimental configurations. The project is exploratory and should not be presented as a proven consensus, safety, security, quantum, or production-ready technology.
