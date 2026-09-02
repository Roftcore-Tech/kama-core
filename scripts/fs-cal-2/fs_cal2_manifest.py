#!/usr/bin/env python3
"""FS-CAL-2 v3 §5.3/§6/§8 — typed manifest generator (single source → pilot/training/holdout manifests).

YETKİ-1 role: implement + mechanically verify the exact row budget (800 / 14,800 / 21,600 / 2,100 =
39,300). Output goes to grounding/manifest-preview/ and is explicitly NOT FROZEN — the result-bearing
freeze re-derives these files under YETKİ-2/3 with the joint-action gate re-checked.

Columns: run_id,phase,cell_id,arm,repetition,seed_domain,rtt_ms,structure,p_ppm,markov_b_ppm,ge_l,
tail_len,environment. Deterministic order (manifest order = execution order).
"""
import hashlib, os, sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(REPO, "reports", "fs-cal-2", "v3", "grounding", "manifest-preview")

HDR = "run_id,phase,cell_id,arm,repetition,seed_domain,rtt_ms,structure,p_ppm,markov_b_ppm,ge_l,tail_len,environment\n"


def rows_for(phase, domain, cells, arms, n, env="IN_PROCESS"):
    out = []
    for cell_id, rtt, structure, p, b, l, tail in cells:
        for arm in arms:
            for rep in range(n):
                rid = f"{phase.lower()}--{cell_id}--{arm}--rep{rep:03d}"
                out.append(
                    f"{rid},{phase},{cell_id},{arm},{rep},{domain},{rtt},{structure},{p},{b if b is not None else ''},{l if l is not None else ''},{tail},{env}\n"
                )
    return out


def pct(p):  # percent → ppm
    return int(p * 10_000)


def cell(name, rtt, structure, p_ppm, b=None, l=None, tail=0):
    return (f"{name}-RTT{rtt}", rtt, structure, p_ppm, b, l, tail)


def pilot_cells():
    # §6.0 frozen signal pilot: IID-20, IID-30, MARKOV-BD(p20,b0.35), GE-L10-p20 × RTT {50,1000}
    out = []
    for rtt in (50, 1000):
        out += [
            cell("IID-20", rtt, "IID", pct(20)),
            cell("IID-30", rtt, "IID", pct(30)),
            cell("MBD-p20-b35", rtt, "MARKOV_BD", pct(20), b=350_000),
            cell("GE-20-L10", rtt, "GE", pct(20), l=10),
        ]
    return out


def training_main_cells():
    # §6.1 main training @ RTT 250 — 23 base conditions
    rtt = 250
    out = []
    for p in (5, 10, 20, 30):
        out.append(cell(f"IID-{p}", rtt, "IID", pct(p)))
    for p in (10, 20, 30):
        for b in (150_000, 350_000):
            out.append(cell(f"MBD-p{p}-b{b//10_000}", rtt, "MARKOV_BD", pct(p), b=b))
    for p in (5, 10, 20, 30):
        out.append(cell(f"GE-{p}-L3", rtt, "GE", pct(p), l=3))
    for p in (5, 10, 20, 30):
        out.append(cell(f"GE-{p}-L10", rtt, "GE", pct(p), l=10))
    out.append(cell("TAIL-1", rtt, "TAIL", 0, tail=1))
    out.append(cell("TAIL-4", rtt, "TAIL", 0, tail=4))
    out.append(cell("MIX-IID20-TAIL1", rtt, "MIXED", pct(20), tail=1))
    out.append(cell("MIX-GE20L3-TAIL1", rtt, "MIXED", pct(20), l=3, tail=1))
    out.append(cell("NOLOSS", rtt, "NONE", 0))
    return out


def training_rtt_cells():
    # §6.2 RTT interaction — 7 conditions × RTT {50,1000}
    out = []
    for rtt in (50, 1000):
        out += [
            cell("IID-20", rtt, "IID", pct(20)),
            cell("MBD-p20-b15", rtt, "MARKOV_BD", pct(20), b=150_000),
            cell("MBD-p20-b35", rtt, "MARKOV_BD", pct(20), b=350_000),
            cell("GE-20-L3", rtt, "GE", pct(20), l=3),
            cell("GE-20-L10", rtt, "GE", pct(20), l=10),
            cell("TAIL-1", rtt, "TAIL", 0, tail=1),
            cell("MIX-IID20-TAIL1", rtt, "MIXED", pct(20), tail=1),
        ]
    return out


def holdout_inproc_cells():
    # §8.1 — 36 base conditions (training'de OLMAYAN loss seviyeleri)
    out = []
    for rtt in (50, 1000):
        for p in (7.5, 15, 25):
            out.append(cell(f"IID-{str(p).replace('.', '_')}", rtt, "IID", pct(p)))
        for p in (15, 25):
            for b in (250_000, 450_000):
                out.append(cell(f"MBD-p{str(p).replace('.', '_')}-b{b//10_000}", rtt, "MARKOV_BD", pct(p), b=b))
        for p in (7.5, 15, 25):
            out.append(cell(f"GE-{str(p).replace('.', '_')}-L3", rtt, "GE", pct(p), l=3))
        for p in (7.5, 15, 25):
            out.append(cell(f"GE-{str(p).replace('.', '_')}-L10", rtt, "GE", pct(p), l=10))
        out.append(cell("TAIL-1", rtt, "TAIL", 0, tail=1))
        out.append(cell("TAIL-4", rtt, "TAIL", 0, tail=4))
        out.append(cell("MIX-IID15-TAIL1", rtt, "MIXED", pct(15), tail=1))
        out.append(cell("MIX-GE15L3-TAIL1", rtt, "MIXED", pct(15), l=3, tail=1))
        out.append(cell("NOLOSS", rtt, "NONE", 0))
    return out


def holdout_netns_cells():
    # §8.2 exact 14 cells
    return [
        cell("IID-7_5", 50, "IID", pct(7.5)),
        cell("IID-15", 1000, "IID", pct(15)),
        cell("IID-25", 1000, "IID", pct(25)),
        cell("GE-15-L3", 50, "GE", pct(15), l=3),
        cell("GE-15-L10", 1000, "GE", pct(15), l=10),
        cell("GE-25-L3", 1000, "GE", pct(25), l=3),
        cell("GE-25-L10", 50, "GE", pct(25), l=10),
        cell("TAIL-1", 50, "TAIL", 0, tail=1),
        cell("TAIL-4", 1000, "TAIL", 0, tail=4),
        cell("MIX-IID15-TAIL1", 1000, "MIXED", pct(15), tail=1),
        cell("MIX-GE15L3-TAIL1", 50, "MIXED", pct(15), l=3, tail=1),
        cell("NOLOSS", 50, "NONE", 0),
        cell("MBD-p15-b25", 50, "MARKOV_BD", pct(15), b=250_000),
        cell("MBD-p25-b45", 1000, "MARKOV_BD", pct(25), b=450_000),
    ]


def write(name, rows):
    os.makedirs(OUT, exist_ok=True)
    content = HDR + "".join(rows)
    p = os.path.join(OUT, name)
    with open(p, "w") as f:
        f.write(content)
    d = hashlib.sha256(content.encode()).hexdigest()
    with open(p + ".sha256", "w") as f:
        f.write(f"{d}  {name}\n")
    return d, len(rows)


def main():
    static2 = ["STATIC_R0", "STATIC_R2"]
    static3 = ["STATIC_R0", "STATIC_R2", "CALIBRATED_POLICY"]

    pilot = rows_for("PILOT", "FSFC2-PILOT-V3", pilot_cells(), static2, 50)
    tmain = rows_for("TRAINING", "FSFC2-TRAIN", training_main_cells(), static2, 200)
    trtt = rows_for("TRAINING", "FSFC2-TRAIN", training_rtt_cells(), static2, 200)
    hin = rows_for("HOLDOUT_INPROC", "FSFC2-HOLDOUT-INPROC-V3", holdout_inproc_cells(), static3, 200)
    hnet = rows_for("HOLDOUT_NETNS", "FSFC2-HOLDOUT-NETNS-V3", holdout_netns_cells(), static3, 50, env="NETNS")

    d1, n1 = write("PILOT-MANIFEST-PREVIEW.csv", pilot)
    d2, n2 = write("TRAINING-MANIFEST-PREVIEW.csv", tmain + trtt)
    d3, n3 = write("HOLDOUT-INPROC-MANIFEST-PREVIEW.csv", hin)
    d4, n4 = write("HOLDOUT-NETNS-MANIFEST-PREVIEW.csv", hnet)

    checks = {
        "pilot": (n1, 800, len(pilot_cells()), 8),
        "training": (n2, 14_800, len(training_main_cells()), 23),
        "holdout_inproc": (n3, 21_600, len(holdout_inproc_cells()), 36),
        "holdout_netns": (n4, 2_100, len(holdout_netns_cells()), 14),
    }
    ok = True
    for k, (got, want, cells_got, cells_want) in checks.items():
        good = got == want and cells_got == cells_want
        ok &= good
        print(f"{k}: rows {got}/{want} cells {cells_got}/{cells_want} {'PASS' if good else 'FAIL'}")
    print(f"rtt_cells_training: {len(training_rtt_cells())}/14 {'PASS' if len(training_rtt_cells()) == 14 else 'FAIL'}")
    total = n1 + n2 + n3 + n4
    print(f"TOTAL result-bearing budget: {total}/39300 {'PASS' if total == 39_300 else 'FAIL'}")
    print(f"sha: pilot={d1[:16]} training={d2[:16]} holdout_inproc={d3[:16]} holdout_netns={d4[:16]}")
    print("STATUS: PREVIEW ONLY — NOT FROZEN (result-bearing freeze is YETKİ-2/3)")
    sys.exit(0 if ok and total == 39_300 else 1)


if __name__ == "__main__":
    main()
