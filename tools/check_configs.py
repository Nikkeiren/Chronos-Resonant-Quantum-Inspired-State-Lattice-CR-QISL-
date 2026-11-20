#!/usr/bin/env python3
"""Static check of configs/*.toml against src/config.rs (no Rust toolchain needed).

Verifies TOML syntax, that every key exists in the matching Rust struct (serde would reject
unknown keys because of deny_unknown_fields), enum-like strings, and the main range checks
mirrored from Config::validate. This is a convenience check, not a replacement for `cargo test`.
"""
import re, sys, glob, tomllib, pathlib

root = pathlib.Path(__file__).resolve().parent.parent
src = (root / "src" / "config.rs").read_text()
structs = {}
for m in re.finditer(r"pub struct (\w+) \{(.*?)\n\}", src, re.S):
    structs[m.group(1)] = re.findall(r"pub (\w+):", m.group(2))
tables = {"topology": "TopologyConfig", "model": "ModelConfig", "initial": "InitialConfig", "network": "NetworkConfig",
          "communication": "CommConfig", "decision": "DecisionConfig", "integrity": "IntegrityConfig",
          "conflict": "ConflictConfig", "adversary": "AdversaryConfig", "features": "FeatureOverrides",
          "metrics": "MetricsConfig", "telemetry": "TelemetryConfig", "batch": "BatchConfig"}
variants = {"weighted_gossip", "weighted_gossip_damping", "phase_ablation", "full_cr_qisl"}
errors = []

def check(path):
    d = tomllib.loads(pathlib.Path(path).read_text())
    for k, v in d.items():
        if k == "events":
            for ev in v:
                for ek in ev:
                    if ek not in structs["EventConfig"]:
                        errors.append(f"{path}: unknown events key '{ek}'")
        elif k in tables:
            for kk in v:
                if kk not in structs[tables[k]]:
                    errors.append(f"{path}: unknown key [{k}].{kk}")
        elif k not in structs["Config"]:
            errors.append(f"{path}: unknown top-level key '{k}'")
    if "variant" in d and d["variant"] not in variants:
        errors.append(f"{path}: bad variant {d['variant']}")
    for v in d.get("batch", {}).get("variants", []):
        if v not in variants:
            errors.append(f"{path}: bad batch variant {v}")
    n = d.get("nodes", 100); ticks = d.get("max_ticks", 300)
    net = d.get("network", {})
    if net.get("partition_enabled", True):
        if net.get("partition_end", 140) <= net.get("partition_start", 60):
            errors.append(f"{path}: partition_end <= partition_start")
        if net.get("partition_end", 140) >= ticks:
            errors.append(f"{path}: partition_end must be < max_ticks")
    if not 0 <= net.get("packet_loss", 0.2) <= 1:
        errors.append(f"{path}: packet_loss out of range")
    for ev in d.get("events", []):
        if ev.get("tick", 61) >= ticks:
            errors.append(f"{path}: event tick beyond max_ticks")
        if ev.get("group", 0) not in (-1, 0, 1):
            errors.append(f"{path}: event group invalid")
        if not isinstance(ev.get("strength", 0.6), (int, float)) or not 0 <= ev.get("strength", 0.6) <= 1:
            errors.append(f"{path}: event strength invalid")
    adv = d.get("adversary", {})
    if adv.get("behavior", "none") not in ("none", "amplitude", "phase", "invalid"):
        errors.append(f"{path}: adversary.behavior invalid")
    if adv.get("placement", "random") not in ("random", "clustered"):
        errors.append(f"{path}: adversary.placement invalid")
    if "alpha" in d.get("model", {}) and not 0 <= d["model"]["alpha"] < 1:
        errors.append(f"{path}: alpha out of range")
    return d

files = sorted(glob.glob(str(root / "configs" / "*.toml")) + glob.glob(str(root / "configs" / "scenarios" / "*.toml")))
for f in files:
    check(f)
# the explicit initial experiment must equal the documented defaults
init = tomllib.loads((root / "configs" / "initial_experiment.toml").read_text())
assert init["nodes"] == 100 and init["candidates"] == 2 and init["max_ticks"] == 300
assert init["network"]["packet_loss"] == 0.2 and init["network"]["partition_start"] == 60 and init["network"]["partition_end"] == 140
assert init["batch"]["seeds"] == 30
# every field of every struct must be present in initial_experiment.toml (it documents the full surface)
for tname, sname in tables.items():
    missing = [f for f in structs[sname] if f not in init.get(tname, {})]
    if missing:
        errors.append(f"initial_experiment.toml: [{tname}] lacks {missing}")
missing_top = [f for f in structs["Config"] if f not in init and f not in tables]
if missing_top:
    errors.append(f"initial_experiment.toml: top-level lacks {missing_top}")
print(f"checked {len(files)} config files")
if errors:
    print("\n".join(errors)); sys.exit(1)
print("all configs consistent with src/config.rs")
