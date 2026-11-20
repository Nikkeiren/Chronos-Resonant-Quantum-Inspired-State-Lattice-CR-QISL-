# Independent Review — CR-QISL Consolidated Study

Date: 2026-11-20
Review scope: Consolidated report, source-level statistical routines, analysis handling, metrics definitions, and selected experiment audits. This is a source review—not an external peer review and not a complete rerun of every experiment.

## Overall verdict

**Promising exploratory simulation pattern; insufficient evidence for protocol-level claims.** The project has improved materially in transparency: assumptions, censoring, structural diagnostics, raw artifacts, and limitations are documented. The repeated evidence-placement pattern is worth retaining as a hypothesis, but the research is not yet ready to claim a general causal mechanism or a validated coordination protocol.

## What is solid

1. Scope discipline: the model is framed as a classical quantum-inspired simulator, not quantum hardware or demonstrated quantum advantage.
2. Separate outcome definitions: local stable fraction, strict agreement, divergence, amplitude spread, and censored convergence are not collapsed into a single success claim.
3. Reproducibility structure: seeds, variants, configs, artifacts, and seed-matched comparisons are preserved in workflows and audits.
4. Confound discovery: audits explicitly identify graph cut-ratio differences between BFS and random grouping, and distinguish randomized-root BFS from true cut-ratio matching.
5. Statistical transparency: paired comparisons, effect-size summaries, Holm correction, and censoring flags are implemented/documented. Statistical code includes unit tests for several reference cases.

## Review findings and required qualifications

### R1 — The primary replicated outcome is local stability, not global coordination
The confirmatory evidence-placement interaction is substantial for final stable fraction, but strict final agreement is rare. The primary result must remain worded as a model-defined local stability association. Do not headline it as consensus or successful coordination.

Status: adequately disclosed in the consolidated report; maintain this distinction in abstracts, titles, posters, and presentations.

### R2 — Partition geometry and cut ratio remain material
BFS and random partition modes have different realized cut ratios in inspected experiments. Randomized-root BFS is a useful sensitivity test, but it is not a cut-ratio-matched comparison. It does not fully isolate evidence placement from graph geometry and related node-selection effects.

Status: unresolved. A future topology-controlled design would be needed to make stronger causal claims. Do not add more batches without a clearly documented design.

### R3 — Exploratory multiplicity and analysis flexibility
The project has numerous variants, metrics, conditions, and follow-up analyses. Holm correction within specified analysis families does not automatically correct across all historical experiments, all metric choices, or all researcher decisions made after observing results. The confirmatory batch is stronger because it defines a primary outcome and contrast, but it is still confirmation within a particular simulator/configuration.

Status: qualification required. Treat other metrics as exploratory/secondary and avoid selectively emphasizing favorable ones.

### R4 — Censoring and threshold-defined diagnostics
Agreement, quorum, and amplitude convergence are frequently censored at the horizon. A censored value is not an observed success time. Continuous amplitude spread helps but cannot substitute for an endpoint that was not reached.

Status: adequately disclosed; continue reporting censoring rates prominently.

### R5 — Source-level statistical implementation review is partial
The inspected Wilcoxon implementation uses zero-difference removal, rank ties, exact subset-sum inference for n≤30 without ties, and a normal approximation with tie/continuity correction otherwise. Its tests cover known reference cases. Holm adjustment appears consistent with the step-down method.

However, this review did not independently reimplement or verify every analysis pathway, including seed-block bootstrap code, data loading/matching, all metric censoring logic, or the complete experiment pipeline. In the inspected function, paired inputs are iterated with zip, which silently truncates unequal-length arrays; callers must guarantee equal-length pairing. That behavior should be explicitly guarded or tested.

Status: partial confidence. Add length-mismatch validation and conduct a separate, independent statistical-code audit before relying on analysis code as validated.

### R6 — Model assumptions and baseline naming
The assumptions documentation acknowledges that Weighted Gossip is a cache-based weighted-neighbor average rather than a mass-conserving protocol such as push-sum, and that the model's trust/integrity and witness mechanisms are heuristic and unauthenticated. These details matter when comparing variants or relating the model to literature.

Status: disclosed; keep comparator descriptions precise and avoid claims of security or equivalence to established protocols.

### R7 — Reproducibility does not equal validation
Successful workflow execution, unit tests, and artifact creation establish computational execution and some software consistency. They do not validate the model's assumptions, scientific interpretation, external applicability, or protocol properties.

Status: explicit in future research communication.

## Recommendations

1. Preserve the consolidated report as the current synthesis; do not expand the claim scope.
2. Add a length-mismatch guard/test to paired statistical routines.
3. Independently audit bootstrap, pairing, censoring, and metric definitions before any publication-grade analysis.
4. If further simulation is pursued, first specify a topology/cut-controlled design with the primary contrast, outcome, exclusions, seed range, and analysis plan documented in advance.
5. Use neutral terminology such as “final stable-node fraction” and “simulated association.” Avoid “consensus achieved,” “secure,” “fault tolerant,” “quantum advantage,” or “validated protocol.”
6. Seek review by a person with relevant expertise in distributed systems and statistical methods before making external scientific claims.

## Final assessment

The current project is best described as a carefully documented exploratory classical simulation with a repeated, model-specific evidence-placement association. The strongest contribution so far is the disciplined audit trail and identification of important structural confounds—not a demonstrated consensus protocol or quantum result.
