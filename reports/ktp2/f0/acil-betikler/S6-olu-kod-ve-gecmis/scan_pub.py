#!/usr/bin/env python3
"""Read-only scan: pub items in target crates vs references in production sources.

Usage: python3 -I scan_pub.py <base_crates_dir> <out_tsv>
"""
import os
import re
import sys

base = sys.argv[1]
out = sys.argv[2]

TARGET = ["adaptive-transport", "kama-engine", "kama-fec-codec", "client-core", "kama-app"]
# production source roots (non-test). bench: only kama_product.rs + main.rs + manifest.rs
PROD_DIRS = [
    "adaptive-transport/src", "kama-engine/src", "kama-fec-codec/src", "client-core/src",
    "kama-app/src", "kama-node/src", "kama-noise-wire/src", "kama-transport-schema/src",
    "hub-server/src", "kama-mobile/src", "kama-store/src", "kama-contracts/src",
]
PROD_EXCLUDE_FILES = {
    "adaptive-transport/src/udp_runner.rs", "adaptive-transport/src/fs_harness.rs",
    "adaptive-transport/src/fs_trace.rs", "adaptive-transport/src/sim.rs",
    "kama-node/src/test_support.rs", "kama-node/src/node/tests.rs",
    "kama-node/src/node/signaling_tests.rs", "kama-node/src/node/tests/glare.rs",
}
BENCH_FILES = ["bench/src/kama_product.rs", "bench/src/main.rs", "bench/src/manifest.rs"]
TEST_DIRS_LABEL = {
    "test": ["/tests/"],
    "example": ["/examples/"],
    "integration": ["kama-integration/"],
    "bench_other": ["bench/src/"],
}


def strip_test_regions(text):
    """Remove #[cfg(test)] mod blocks and items gated by cfg(any(test, ...)). Brace matching."""
    lines = text.split("\n")
    keep = [True] * len(lines)
    i = 0
    n = len(lines)
    while i < n:
        l = lines[i].strip()
        if l.startswith("#[cfg(test)]") or (l.startswith("#[cfg(any(test") ) or l.startswith("#[cfg(all(test"):
            # find the start of the item and brace-match
            j = i
            depth = 0
            started = False
            while j < n:
                s = lines[j]
                # crude: strip string literals and line comments
                s2 = re.sub(r'"(\\.|[^"\\])*"', '""', s)
                s2 = s2.split("//")[0]
                for ch in s2:
                    if ch == "{":
                        depth += 1
                        started = True
                    elif ch == "}":
                        depth -= 1
                keep[j] = False
                if started and depth <= 0:
                    break
                if not started and s2.rstrip().endswith(";") and j > i:
                    break
                j += 1
            i = j + 1
            continue
        i += 1
    return "\n".join(l if k else "" for l, k in zip(lines, keep))


def rs_files(root):
    for dp, dn, fn in os.walk(root):
        for f in fn:
            if f.endswith(".rs"):
                yield os.path.join(dp, f)


def rel(p):
    return os.path.relpath(p, base)


prod_text = {}
for d in PROD_DIRS:
    root = os.path.join(base, d)
    if not os.path.isdir(root):
        continue
    for p in rs_files(root):
        r = rel(p)
        if r in PROD_EXCLUDE_FILES:
            continue
        with open(p, encoding="utf-8") as fh:
            prod_text[r] = strip_test_regions(fh.read())
for r in BENCH_FILES:
    p = os.path.join(base, r)
    if os.path.exists(p):
        with open(p, encoding="utf-8") as fh:
            prod_text[r] = strip_test_regions(fh.read())

other_text = {}
for p in rs_files(base):
    r = rel(p)
    if r in prod_text:
        continue
    with open(p, encoding="utf-8") as fh:
        other_text[r] = fh.read()

item_re = re.compile(r'^\s*pub(?:\([^)]*\))?\s+(?:const\s+fn|async\s+fn|unsafe\s+fn|fn|const|static|struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)')
rows = []
for c in TARGET:
    root = os.path.join(base, c, "src")
    for p in rs_files(root):
        r = rel(p)
        if r in PROD_EXCLUDE_FILES:
            continue
        with open(p, encoding="utf-8") as fh:
            raw = fh.read()
        stripped = strip_test_regions(raw)
        for ln, line in enumerate(stripped.split("\n"), 1):
            m = item_re.match(line)
            if not m:
                continue
            if "pub(crate)" in line or "pub(super)" in line:
                vis = "crate"
            else:
                vis = "pub"
            name = m.group(1)
            pat = re.compile(r'\b' + re.escape(name) + r'\b')
            prod_refs = 0
            prod_files = set()
            for fr, t in prod_text.items():
                cnt = len(pat.findall(t))
                if fr == r:
                    cnt -= 1  # its own definition
                if cnt > 0:
                    prod_refs += cnt
                    prod_files.add(fr)
            other = {}
            for fr, t in other_text.items():
                if pat.search(t):
                    lab = "other"
                    for k, subs in TEST_DIRS_LABEL.items():
                        if any(s in fr for s in subs):
                            lab = k
                            break
                    if "/src/" in fr and lab == "other":
                        lab = "src_test_or_harness"
                    other[lab] = other.get(lab, 0) + 1
            rows.append((r, ln, vis, name, prod_refs, ";".join(sorted(prod_files))[:200],
                         ",".join(f"{k}:{v}" for k, v in sorted(other.items()))))

with open(out, "w", encoding="utf-8") as fh:
    for row in rows:
        fh.write("\t".join(str(x) for x in row) + "\n")
print(len(rows), "items;", sum(1 for r in rows if r[4] == 0), "with zero production refs")
