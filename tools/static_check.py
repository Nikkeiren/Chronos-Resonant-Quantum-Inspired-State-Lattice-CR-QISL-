#!/usr/bin/env python3
"""Lightweight static sanity checks for the Rust sources (used because no Rust toolchain was
available when this code was written). It is NOT a compiler: it catches unbalanced delimiters,
missing module files, imports of items that do not exist, and obviously dangling cross-module calls.
Run `cargo build && cargo test` for the real verification."""
import re, sys, pathlib

root = pathlib.Path(__file__).resolve().parent.parent
files = sorted((root / "src").glob("*.rs")) + sorted((root / "tests").glob("*.rs"))
errors = []

def strip(code):
    """Remove comments, string and char literals so delimiter counting is reliable."""
    out, i, n = [], 0, len(code)
    while i < n:
        c = code[i]
        if code.startswith("//", i):
            while i < n and code[i] != "\n": i += 1
        elif code.startswith("/*", i):
            j = code.find("*/", i + 2); i = n if j < 0 else j + 2
        elif c == '"':
            i += 1
            while i < n and code[i] != '"':
                i += 2 if code[i] == "\\" else 1
            i += 1; out.append('""')
        elif c == "'":
            m = re.match(r"'(\\.|[^\\'])'", code[i:])
            if m: i += m.end(); out.append("' '")
            else: out.append(c); i += 1   # lifetime
        else:
            out.append(c); i += 1
    return "".join(out)

stripped = {}
for f in files:
    s = strip(f.read_text()); stripped[f.name] = s
    for o, c in ("()", "[]", "{}"):
        if s.count(o) != s.count(c):
            errors.append(f"{f.name}: unbalanced {o}{c}: {s.count(o)} vs {s.count(c)}")
    # per-line stack check for correct nesting
    stack = []
    pairs = {")": "(", "]": "[", "}": "{"}
    for ln, line in enumerate(s.splitlines(), 1):
        for ch in line:
            if ch in "([{": stack.append((ch, ln))
            elif ch in ")]}":
                if not stack or stack[-1][0] != pairs[ch]:
                    errors.append(f"{f.name}:{ln}: mismatched '{ch}'"); stack = []; break
                stack.pop()
    if stack: errors.append(f"{f.name}: unclosed '{stack[-1][0]}' opened at line {stack[-1][1]}")

# modules declared in lib.rs must exist
lib = (root / "src" / "lib.rs").read_text()
mods = re.findall(r"pub mod (\w+);", lib)
for m in mods:
    if not (root / "src" / f"{m}.rs").exists(): errors.append(f"lib.rs declares missing module {m}")
for f in (root / "src").glob("*.rs"):
    if f.stem not in mods and f.stem not in ("lib", "main"): errors.append(f"{f.name} not declared in lib.rs")

# public items per module
items = {}
for m in mods:
    s = stripped[f"{m}.rs"]
    items[m] = set(re.findall(r"pub (?:fn|struct|enum|const|static|type|trait) (\w+)", s))
    items[m] |= set(re.findall(r"pub mod (\w+)", s))

def check_use(fname, text):
    for m in re.finditer(r"use (?:crate|crqisl)::(\w+)(?:::(\{[^}]*\}|\w+))?;", text):
        mod, rest = m.group(1), m.group(2)
        if mod not in items: errors.append(f"{fname}: use of unknown module {mod}"); continue
        if not rest: continue
        names = [x.strip() for x in rest.strip("{}").split(",") if x.strip()]
        for nme in names:
            base = nme.split(" as ")[0].strip()
            if base == "self": continue
            if base not in items[mod]: errors.append(f"{fname}: `{mod}::{base}` is not a pub item of {mod}.rs")
for f in files: check_use(f.name, stripped[f.name])

# associated functions called as Type::name(...) must exist in an impl of that type
impls = {}
for f in files:
    s = stripped[f.name]
    for m in re.finditer(r"impl(?:<[^>]*>)? (?:\w+ for )?(\w+) \{", s):
        t = m.group(1); start = m.end(); depth = 1; i = start
        while i < len(s) and depth:
            depth += {"{": 1, "}": -1}.get(s[i], 0); i += 1
        impls.setdefault(t, set()).update(re.findall(r"fn (\w+)", s[start:i]))
std_types = {"Vec","String","Rng","f64","u64","Some","Ok","Err","Option","Result","BTreeMap","BTreeSet","HashSet","VecDeque","Box","Mutex","AtomicUsize","Ordering","PathBuf","Path","ExitCode","Default","std","serde_json","toml","fs","thread","env","process","mem","slice","Self","usize","u8","i64","char","str","PI","MsgKind","DecState","AdvKind","Variant","Features","Config","Network","Message","Node","Series","RunSummary","Setup","Graph","Report","Args","EventLog","Counters"}
for f in files:
    s = stripped[f.name]
    for m in re.finditer(r"\b([A-Z]\w+)::(\w+)\(", s):
        t, fn = m.groups()
        if t in impls and fn not in impls[t] and t not in ("Some", "Ok", "Err"):
            # enum variants with tuple payloads look like calls too
            if not re.search(rf"\b{fn}\(", "") and not re.search(rf"enum {t} \{{[^}}]*\b{fn}\b", s):
                errors.append(f"{f.name}: {t}::{fn}(..) has no matching fn in impl {t}")

# every METRIC column is inserted by sim.rs
metrics = re.findall(r'"(\w+)",', re.search(r"METRIC_COLUMNS: &\[&str\] = &\[(.*?)\];", stripped["metrics.rs"] and (root / "src" / "metrics.rs").read_text(), re.S).group(1))
simtxt = (root / "src" / "sim.rs").read_text()
for c in metrics:
    if f'"{c}".into()' not in simtxt and f'"{c}"' not in simtxt:
        errors.append(f"sim.rs never writes metric {c}")
print(f"{len(files)} files, {len(metrics)} metrics, modules: {', '.join(mods)}")
if errors:
    print("\n".join(errors)); sys.exit(1)
print("static checks passed (delimiters, modules, imports, associated fns, metrics)")
