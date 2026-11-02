#!/usr/bin/env python3
# Bagimsiz ham-veri ayristirici (V-M2 dogrulama). Salt okuma.
import os, re, sys, csv, statistics as st

ROOT = "/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam"
OUT = os.path.dirname(os.path.abspath(__file__))

def grab(pat, s, cast=int, default=None):
    m = re.search(pat, s)
    if not m:
        return default
    try:
        return cast(m.group(1))
    except Exception:
        return default

rows = []
for lane in ("b", "c", "d"):
    lroot = os.path.join(ROOT, lane, "out")
    for cell in sorted(os.listdir(lroot)):
        cdir = os.path.join(lroot, cell)
        if not os.path.isdir(cdir):
            continue
        for arm in sorted(os.listdir(cdir)):
            adir = os.path.join(cdir, arm)
            if not os.path.isdir(adir):
                continue
            for rep in sorted(os.listdir(adir)):
                rdir = os.path.join(adir, rep)
                if not (os.path.isdir(rdir) and rep.startswith("rep")):
                    continue
                ce = os.path.join(rdir, "c.err")
                se = os.path.join(rdir, "s.err")
                c = open(ce, errors="replace").read() if os.path.exists(ce) else ""
                s = open(se, errors="replace").read() if os.path.exists(se) else ""
                kp = "KP-CLIENT" in c
                mx = "MIXED-CLIENT" in c
                if not (kp or mx):
                    continue
                r = dict(lane=lane, cell=cell, arm=arm, rep=rep)
                if kp:
                    line = next((l for l in c.splitlines() if l.startswith("KP-CLIENT ")), "")
                    r["fecB"] = grab(r" fec=(\d+) ctrl=", line)
                    r["retxB"] = grab(r"retx=(\d+) dup=", line)
                    r["dataB"] = grab(r"\(data=(\d+)", line)
                    r["fec_blocks"] = grab(r"fec_blocks=(\d+)", line)
                    r["fec_repairs_tx"] = grab(r"fec_repairs_tx=(\d+)", line)
                    r["fec_rec_ctrl"] = grab(r"fec_recovered_ctrl=(\d+)", line)
                    m = re.search(r"latch=\((\d+), (\d+), (\d+), (\d+), (\d+)\)", line)
                    if m:
                        r["l_noelig"], r["l_mism"], r["l_norule"], r["l_enq"], r["l_emit"] = map(int, m.groups())
                    r["suppress"] = grab(r"suppress=(\S+)", line, str)
                    r["delivered"] = grab(r"delivered=(\d+)/", line)
                    act = next((l for l in c.splitlines() if l.startswith("KP-ACTIVATED")), "")
                    m = re.search(r"fec2=\[repair_not_arqed=(\d+) clamp=(\d+) would_zero=(\d+) partial_blocks=(\d+)\]", act)
                    if m:
                        r["rna"], r["clamp"], r["would_zero"], r["partial"] = map(int, m.groups())
                    r["preconv_blocked"] = grab(r"preconv_parity_blocked=(\d+)", act)
                    av = next((l for l in c.splitlines() if l.startswith("KP-ACTIVATION")), "")
                    r["parity_fired"] = grab(r" parity=(\d+) parity_denied", av)
                    r["parity_denied"] = grab(r"parity_denied=(\d+)", av)
                    r["retx_fired"] = grab(r"parity_denied=\d+ retx=(\d+)", av)
                    r["btlbw"] = grab(r"est=\[btlbw=(\d+)B/s", av)
                    r["srtt_us"] = grab(r"srtt=(\d+)us", av)
                    r["recover_fwd"] = grab(r"recover_fwd=(\d+)", av)
                else:
                    line = next((l for l in c.splitlines() if l.startswith("MIXED-CLIENT")), "")
                    r["mixed_line"] = line[:400]
                    r["fecB"] = grab(r" fec=(\d+)", line)
                    r["retxB"] = grab(r"retx=(\d+)", line)
                srv = next((l for l in s.splitlines() if "ServerStats" in l), "")
                for k in ("fec_repairs_rx", "fec_recovered_units", "fec_decode_failures", "fec_recovered_ctrl_tx", "delivered"):
                    r["srv_" + k] = grab(r"\b" + k + r": (\d+)", srv)
                rows.append(r)

keys = []
for r in rows:
    for k in r:
        if k not in keys:
            keys.append(k)
with open(os.path.join(OUT, "vruns.csv"), "w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=keys)
    w.writeheader()
    for r in rows:
        w.writerow(r)
print("rows", len(rows))
