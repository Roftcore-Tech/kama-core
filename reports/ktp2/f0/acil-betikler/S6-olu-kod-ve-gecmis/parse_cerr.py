#!/usr/bin/env python3
"""Read-only: parse Kama-product client stderr (c.err) across F0 raw runs.

Usage: python3 -I parse_cerr.py <runs_tam_dir> <out_tsv>
Only arms whose c.err contains 'KP-MANIFEST' are parsed.
"""
import os
import re
import sys

root = sys.argv[1]
out = sys.argv[2]

pat = {
    "profile": re.compile(r"KP-MANIFEST role=client profile=(\S+)"),
    "fec": re.compile(r"KP-MANIFEST role=client .*? fec=(\S+ \S+ \S+)"),
    "delivered": re.compile(r"KP-CLIENT delivered=(\d+)/(\d+)"),
    "fec_bytes": re.compile(r"KP-CLIENT .*?tx=\d+B\(data=\d+ retx=\d+ dup=\d+ fec=(\d+)"),
    "retx": re.compile(r"KP-CLIENT .*?retx=(\d+)"),
    "fec_blocks": re.compile(r"KP-CLIENT .*? fec_blocks=(\d+)"),
    "fec_repairs_tx": re.compile(r"KP-CLIENT .*? fec_repairs_tx=(\d+)"),
    "gate_opened": re.compile(r"KP-CLIENT .*? gate_opened=(\d+)"),
    "latch": re.compile(r"KP-CLIENT .*? latch=\(([^)]*)\)"),
    "suppress": re.compile(r"KP-CLIENT .*? suppress=(\S+)"),
    "arb": re.compile(r"KP-CLIENT .*? arb=\[([^\]]*)\]"),
    "clamp": re.compile(r"fec2=\[repair_not_arqed=\d+ clamp=(\d+) would_zero=(\d+) partial_blocks=(\d+)\]"),
    "btlbw": re.compile(r"est=\[btlbw=(\d+)B/s srtt=(\d+)us"),
    "preconv_blocked": re.compile(r"preconv_parity_blocked=(\d+)"),
    "recover_fwd": re.compile(r"recover_fwd=(\d+)"),
}

rows = []
for dp, dn, fn in os.walk(root):
    if "c.err" not in fn:
        continue
    p = os.path.join(dp, "c.err")
    try:
        with open(p, encoding="utf-8", errors="replace") as fh:
            t = fh.read()
    except OSError:
        continue
    if "KP-MANIFEST role=client" not in t:
        continue
    relp = os.path.relpath(dp, root)
    parts = relp.split(os.sep)
    # lane/out/cell/arm/repN
    if len(parts) < 5:
        continue
    lane, _, cell, arm, rep = parts[:5]
    r = {"lane": lane, "cell": cell, "arm": arm, "rep": rep}
    for k, rx in pat.items():
        m = rx.search(t)
        if not m:
            r[k] = ""
            continue
        if k == "delivered":
            r[k] = f"{m.group(1)}/{m.group(2)}"
        elif k == "clamp":
            r["clamp"] = m.group(1)
            r["would_zero"] = m.group(2)
            r["partial_blocks"] = m.group(3)
        elif k == "btlbw":
            r["btlbw"] = m.group(1)
            r["srtt_us"] = m.group(2)
        else:
            r[k] = m.group(1)
    rows.append(r)

keys = ["lane", "cell", "arm", "rep", "profile", "fec", "delivered", "fec_bytes", "retx", "fec_blocks",
        "fec_repairs_tx", "gate_opened", "latch", "suppress", "arb", "clamp", "would_zero",
        "partial_blocks", "btlbw", "srtt_us", "preconv_blocked", "recover_fwd"]
with open(out, "w", encoding="utf-8") as fh:
    fh.write("\t".join(keys) + "\n")
    for r in rows:
        fh.write("\t".join(str(r.get(k, "")) for k in keys) + "\n")
print(len(rows), "client runs parsed")
