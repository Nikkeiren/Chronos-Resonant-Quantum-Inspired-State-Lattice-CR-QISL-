# Getting started — CR-QISL

This guide explains how to run the simulator and inspect its outputs without editing Rust source code.

## 1. What this project is

CR-QISL is a quantum-inspired **classical simulator** for studying distributed state coordination under message loss, latency, and network partitions. It is not quantum hardware and it is not a proven consensus protocol. Its metrics are observations from this simulator, not guarantees for real systems.

## 2. Run the automated checks

Open the repository's **Actions** tab and select the `ci` workflow. A successful run should complete static checks, configuration checks, viewer smoke tests, release build, Rust tests, and the quick batch experiment.

Do not treat a red workflow as a valid experiment result. Open the failed job and inspect its logs first.

## 3. Run the main experiment

After CI passes:

1. Open **Actions**.
2. Select **main-experiment**.
3. Press **Run workflow**, keep the default branch, then confirm.
4. Wait for the job to finish successfully.
5. Open the completed run and download the artifact named `cr-qisl-main-experiment-<run number>`.

The predefined configuration runs 4 variants × 30 seeds = 120 runs. The workflow checks that the manifest contains 120 run records. Each run produces a summary row, time-series CSV and event CSV. The first seed of each variant also produces a per-node trace for the viewer.

## 4. Read the results

After extracting the artifact ZIP:

- `manifest.json`: run count, failed runs, configuration hash and provenance.
- `config.resolved.toml` / `config.resolved.json`: exact configuration used.
- `runs.csv`: one row per variant and seed, including all run-level metrics.
- `analysis/descriptive.csv`: median, quartiles, IQR, min/max for each metric and variant.
- `analysis/paired_tests.csv`: paired seed comparisons, Holm-adjusted p-values and uncensored sensitivity results.
- `analysis/report.md`: readable statistical report.
- `series/`: per-tick observer metrics for every run.
- `events/`: network and decision event logs.
- `traces/`: per-node amplitude, phase and decision histories for the trace seeds.

Always check `failed_runs` in the manifest before interpreting the analysis. The intended full experiment has 120 run records and zero failed runs.

## 5. Open the interactive viewer

The viewer is `viewer/index.html` in the repository.

1. Open the repository's `viewer/index.html` page, or save that file and open it in a modern browser.
2. In the viewer, choose a `*.trace.json` file from the extracted artifact's `traces/` folder.
3. Use Play, the timeline slider, candidate selector and colour mode selector to explore the recorded frames.

The default batch configuration records traces for the first seed of each variant. A trace is a sampled per-node history; it is not a live simulation. The CSV time series contains the full observer-level timeline for every run.

## 6. New amplitude-spread diagnostics

- `final_amplitude_spread`: final tick's spread, only valid when no honest node has non-finite state at that tick.
- `post_heal_amplitude_spread_mean`: average spread from the healing reference tick (inclusive) through the end, using only ticks with zero non-finite honest nodes.
- `post_heal_amplitude_spread_min`: smallest spread in that same valid post-heal window.

A lower spread means honest nodes' amplitudes are more similar. It does **not** mean they agree on a decision. If there are no valid observations, the diagnostic is NaN. These metrics complement, rather than replace, the threshold-based amplitude-convergence time.

## 7. Preserve experiment provenance

Do not overwrite or rename old artifacts. Keep each workflow run's artifact and record its run URL, commit SHA, configuration hash, run count and failed-run count. A rerun with the same configuration and seeds is a reproducibility check; changing the configuration creates a different experiment and must be described as such.

## 8. Current limits

The predefined experiment uses no adversarial nodes. It therefore does not test Byzantine or adversarial resilience. Censored convergence times are summarized with the horizon value in the primary analysis, with a separate uncensored-pair sensitivity analysis; this is not a survival-analysis model. Conclusions must remain limited to the simulator and its stated assumptions.
