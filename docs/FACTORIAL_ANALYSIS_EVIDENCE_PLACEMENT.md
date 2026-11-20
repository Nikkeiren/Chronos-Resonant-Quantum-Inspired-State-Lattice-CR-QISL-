# Exploratory factorial contrast — independent evidence placement

Date: 2026-11-20  
Source run: GitHub Actions run 44712016218  
Source commit: 98c834788188545f56ef89e691d0dc8340605aa0  
Scope: Full CR-QISL variant only; 30 paired seeds in each of six cells.

## Purpose

The 2 x 3 design crosses network partition method (BFS, RANDOM) with evidence assignment (ALIGNED, REVERSED, INDEPENDENT_BALANCED). This note adds an explicit interaction contrast to the earlier pairwise audit. It is an exploratory, seed-blocked analysis, not a confirmatory factorial model.

## Analysis method

For each metric, calculate the independent-assignment contrast within each network method:
- BFS contrast = BFS-INDEPENDENT_BALANCED minus the mean of BFS-ALIGNED and BFS-REVERSED.
- RANDOM contrast = RANDOM-INDEPENDENT_BALANCED minus the mean of RANDOM-ALIGNED and RANDOM-REVERSED.
- Interaction contrast = BFS contrast minus RANDOM contrast.

All six cells are paired by seed (1–30). A percentile 95% confidence interval was estimated by resampling the 30 seed blocks with replacement 20,000 times (NumPy RNG seed 20261120). This preserves the within-seed relationship across conditions. The contrast is a descriptive difference-in-differences; no p-value is claimed. Since independent-balanced assignments are randomized, the seed is also part of the assignment generation. Results are conditional on this one set of 30 assignment seeds.

## Cell means and interaction contrasts

Means are shown (not medians) because the interaction is calculated on the arithmetic-mean scale.

| Metric | BFS independent | BFS aligned/reversed avg | Random independent | Random aligned/reversed avg | Interaction | 95% bootstrap CI |
|---|---:|---:|---:|---:|---:|---:|
| Final stable fraction | 0.6560 | 0.1668 | 0.0723 | 0.1185 | +0.5353 | [+0.4290, +0.6428] |
| Final strict agreement | 0.0667 | 0.0000 | 0.0000 | 0.0000 | +0.0667 | [0.0000, +0.1667] |
| Final split metric | 0.0000 | 0.3833 | 0.0000 | 0.0000 | -0.3833 | [-0.5000, -0.2667] |
| Post-heal divergence | 0.00065 | 0.15106 | 0.00062 | 0.02735 | -0.12368 | [-0.13635, -0.11089] |
| Final amplitude spread | 0.01933 | 0.10786 | 0.00562 | 0.01819 | -0.07596 | [-0.08302, -0.06867] |
| Post-heal mean amplitude spread | 0.03282 | 0.13906 | 0.01255 | 0.03582 | -0.08296 | [-0.08936, -0.07644] |
| Conflict entries | 10.97 | 375.02 | 0.00 | 40.40 | -323.65 | [-346.52, -297.38] |
| Messages sent | 26,677.77 | 52,208.70 | 26,065.90 | 33,630.53 | -17,966.30 | [-19,609.39, -16,285.43] |

## Interpretation

1. The independent-balanced evidence assignment behaves very differently under BFS than under RANDOM in this experiment. The stable-fraction interaction is +0.535 on the proportion scale, and the paired bootstrap interval is well above zero.
2. The same BFS-specific pattern appears descriptively for lower post-heal divergence, amplitude spread, conflict entries, and message count. These metrics are related and should not be treated as eight independent confirmations.
3. Strict final agreement remains rare: only 2/30 BFS-INDEPENDENT_BALANCED Full runs reached final agreement; none of the other five cells did. The interaction in stable fraction is therefore emphatically not evidence of reliable global consensus.
4. The final_split field is retained as an outcome metric, but should be interpreted using its implementation definition; it is not interchangeable with final agreement.
5. The independent-balanced assignment is random but size-constrained to match the network partition group sizes. BFS and RANDOM also have materially different graph cut ratios (previously measured on four trace seeds only), so topology and evidence placement are not isolated from all structural differences.
6. This is a post-hoc exploratory contrast over one 30-seed batch. It does not establish a general causal law, protocol safety, Byzantine tolerance, finality, or quantum advantage.

## What this analysis does and does not establish

It establishes a strong, seed-paired interaction pattern in this particular simulator configuration: independent evidence placement coincides with much higher local stable fraction under BFS, but not under RANDOM. It does not show why this happens, whether it generalizes to other graph realizations or partition parameters, or whether the stable nodes agree on one candidate.

## Next experiment recommended

Before increasing seed count, add per-seed structural diagnostics to the machine-readable run output: network/evidence group contingency counts, group sizes, cross-group edge count, cut ratio, and evidence-label overlap. Export these for all seeds, not only trace seeds. Then repeat the factorial design with additional independent seeds and at least one alternate graph/topology setting. Keep local stability and strict final agreement as separate primary outcomes.