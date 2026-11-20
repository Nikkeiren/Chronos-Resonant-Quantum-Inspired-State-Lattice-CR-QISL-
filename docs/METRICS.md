# Metric definitions

"Level": **local** = defined from one node's own state; **observer** = computed by the simulator from all honest nodes
(not available to any node, not finality). Honest nodes only (adversaries are excluded). `reference tick` = healing tick
if a partition is configured, else 0. Times are measured from the reference tick; a condition must hold for
`metrics.hold_ticks` consecutive ticks; the reported time is the *start* of the first such run.
Times that never occur are right-censored: value = horizon − reference and the matching `*_censored` flag is 1.

| Column | Level | Definition |
|---|---|---|
| `agreement_time_after_heal` / `agreement_censored` | observer | All honest nodes are `Stable` on the same candidate. |
| `quorum_agreement_time_after_heal` / `quorum_censored` | observer | ≥ `metrics.quorum_fraction` of honest nodes are `Stable` on the same candidate. |
| `amplitude_convergence_time` / `convergence_censored` | observer | Max over honest nodes and candidates of \|a − mean\| < `convergence_eps`, and no non-finite state. |
| `final_amplitude_spread` | observer | Spread at the final tick, computed over finite honest nodes. |
| `post_heal_amplitude_spread_mean`, `post_heal_amplitude_spread_min` | observer | Mean and minimum spread from the healing reference tick (inclusive) to the horizon; only ticks with zero non-finite honest nodes are included. If no valid ticks exist, value is NaN. |
| `stability_time_median`, `stability_time_p90`, `nodes_never_stable`, `stability_censored_fraction` | local | Tick at which each honest node *first* entered `Stable` (never = horizon, censored); median/p90 across nodes. |
| `divergence_pre_heal`, `divergence_post_heal` | observer | Mean over the window of 1 − (largest Stable-decision group)/(nodes in Stable), 0 when nobody is Stable. Pre = [partition_start, partition_end); post = [partition_end, horizon). Without a partition: pre = whole run, post = NaN. |
| `split_ticks_pre_heal`, `split_ticks_post_heal` | observer | Number of ticks in those windows with at least two different Stable decisions. |
| `oscillation_mean` | local | Mean number of tentative-dominant-candidate switches per honest node. |
| `stable_revocations` | local | Total STABLE → non-stable transitions of honest nodes. |
| `conflict_entries`, `conflict_pending_final`, `reconcile_timeouts` | local | Entries into `CONFLICT_PENDING`; nodes still pending at the end; conflicts older than `reconcile_timeout` (logged only). |
| `normalization_resets` | local | Uniform resets after degenerate normalisation (validation on). |
| `final_stable_fraction`, `final_agreement`, `final_split` | observer | State at the last tick. |
| `adv_target_adoption_final` | observer | Fraction of honest nodes Stable on the adversary's target candidate. |
| `nan_corrupted_nodes` | observer | Honest nodes with non-finite amplitude/phase at the end. |
| `messages_sent`, `messages_delivered` | — | All transmissions attempted (incl. adversary, witness forwards) / arrivals at a receiver. |
| `dropped_loss`, `dropped_partition`, `dropped_invalid`, `dropped_total` | — | Random loss; partition (at send or in flight); rejected by receiver validation (after delivery); total. |
| `undelivered_end`, `stale_ignored` | — | Still in flight at the horizon; reordered/stale messages ignored by the cache. |
| `bytes_sent`, `bytes_delivered` | — | Per the byte model A9. |
| `state_updates`, `heartbeats`, `witness_messages`, `reconciliation_messages`, `adversary_messages` | — | Message counts per kind (adversary messages are also in `state_updates`). |
| `retransmissions`, `signature_ops` | — | Always 0 (not implemented). |

Identity (tested): `messages_sent = messages_delivered + dropped_loss + dropped_partition + undelivered_end`.

Per-tick series (`series/*.csv`): tick, partition flag, undecided/tentative/conflict counts, `stable_k`, `mean_amp_k`,
`coherence_k` (|Σ a e^{iθ}| / Σ a), amplitude spread, divergence, agree flag, cumulative message counters, NaN nodes.
Per-node histories (`traces/*.json`): amplitude, phase, decision state and decided candidate per node and frame.
Network/decision event history: `events/*.csv`.
