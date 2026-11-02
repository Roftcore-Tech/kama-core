#!/usr/bin/env python3
"""Independent re-derivation (verifier) of S4 retransmit timeline from lane-B captures.

Uses the ENGINE seq carried in the DATA frame header ([0x00][seq u16][unit u64], client-core
wire.rs) instead of a send-rank, and maps ACK PN ranges -> engine seq via the client's own
outgoing PN table. Usage: python3 -I vretx.py <cell_dir> <arm> [...]
"""
import gzip
import json
import pathlib
import re
import sys


def rows_of(path):
    out = []
    with gzip.open(path, "rt", errors="replace") as f:
        for line in f:
            if not line or line[0] == "#":
                continue
            p = line.rstrip("\n").split(",", 6)
            if len(p) < 7 or p[2] != "17":
                continue
            try:
                out.append((int(p[0]), p[1], int(p[4]), int(p[5]), p[6]))
            except ValueError:
                pass
    out.sort(key=lambda r: r[0])
    return out


def split(x):
    if ":" in x:
        h, t = x.split(":", 1)
        return bytes.fromhex(h), bytes.fromhex(t)
    b = bytes.fromhex(x)
    return b[:-16], b[-16:]


def ack_pns16(fr):
    if len(fr) < 9 or fr[0] not in (0x10, 0x14):
        return None
    largest = int.from_bytes(fr[2:4], "big")
    b0 = int.from_bytes(fr[6:8], "big")
    n = fr[8]
    s = set()
    if b0 == 0:
        return s
    cur = (largest - b0 + 1) & 0xFFFF
    for k in range(b0):
        s.add((cur + k) & 0xFFFF)
    off = 9
    for _ in range(n):
        if off + 4 > len(fr):
            break
        gap = int.from_bytes(fr[off:off + 2], "big")
        ln = int.from_bytes(fr[off + 2:off + 4], "big")
        off += 4
        cur = (cur - gap) & 0xFFFF
        if ln:
            hi = cur
            for k in range(ln):
                s.add((hi - k) & 0xFFFF)
            cur = (cur - ln) & 0xFFFF
    return s


def analyze(rep):
    port = int(re.search(r"--bind\s+\S+:(\d+)", (rep / "cmd.txt").read_text()).group(1))
    meta = json.loads((rep / "out.jsonl").open().readline())["meta"]
    t0 = meta["t0_unix_ns"]
    lpc = meta.get("activation", {}).get("left_preconverge_ns")
    lpc_abs = (t0 + lpc) if lpc else None
    cli = rows_of(rep / "capb_cli.csv.gz")
    srv = rows_of(rep / "capb_srv.csv.gz")
    tx = []  # (t, pn, seq, unit)
    for t, d, sp, dp, x in cli:
        if d != "o" or dp != port or not x:
            continue
        fr, tag = split(x)
        if len(tag) < 16 or not fr:
            continue
        if fr[0] in (0x00, 0x05) and len(fr) >= 11:
            tx.append((t, int.from_bytes(tag[:8], "big"), int.from_bytes(fr[1:3], "big"),
                       int.from_bytes(fr[3:11], "big")))
    arrived = set()
    for t, d, sp, dp, x in srv:
        if d != "i" or dp != port or not x:
            continue
        fr, tag = split(x)
        if fr and fr[0] in (0x00, 0x05):
            arrived.add(int.from_bytes(tag[:8], "big"))
    # unwrap seq16 (monotone send order expected)
    full = []
    base = 0
    last = None
    mono_viol = 0
    for t, pn, s16, u in tx:
        if last is not None and s16 < (last & 0xFFFF) and (last & 0xFFFF) - s16 > 30000:
            base += 65536
        s = base + s16
        if last is not None and s <= last:
            mono_viol += 1
        last = s
        full.append((t, pn, s, u))
    pn16_to_seq = {pn & 0xFFFF: s for t, pn, s, u in full}
    acks = []
    run = -1
    for t, d, sp, dp, x in cli:
        if d != "i" or sp != port or not x:
            continue
        fr, tag = split(x)
        a = ack_pns16(fr)
        if a is None:
            continue
        seqs = [pn16_to_seq[p] for p in a if p in pn16_to_seq]
        if seqs:
            run = max(run, max(seqs))
        acks.append((t, run))
    first_tx = {}
    for t, pn, s, u in full:
        first_tx.setdefault(u, t)
    by_unit = {}
    for r in full:
        by_unit.setdefault(r[3], []).append(r)
    ev = []
    for u, lst in by_unit.items():
        for i in range(len(lst) - 1):
            t, pn, s, _ = lst[i]
            if pn in arrived:
                continue
            nxt = lst[i + 1][0]
            ev3 = next((ta for ta, m in acks if ta >= t and m >= s + 3), None)
            ev1 = next((ta for ta, m in acks if ta >= t and m >= s + 1), None)
            rec = {"t": t, "nxt": nxt, "ev3": ev3, "ev1": ev1, "i": i,
                   "pre": (lpc_abs is not None and ev3 is not None and ev3 < lpc_abs)}
            if ev3 is not None and nxt >= ev3:
                rec["wait"] = (nxt - ev3) / 1e9
                rec["n_new"] = sum(1 for uu, ft in first_tx.items() if ev3 < ft < nxt)
                rec["n_any"] = sum(1 for q in full if ev3 < q[0] < nxt)
            ev.append(rec)
    return ev, mono_viol, len(full)


def q(v, p):
    v = sorted(v)
    if not v:
        return float("nan")
    return v[max(0, min(len(v) - 1, int(round(p * (len(v) - 1)))))]


def main():
    cell = pathlib.Path(sys.argv[1])
    for arm in sys.argv[2:]:
        allev = []
        mv = 0
        reps = sorted((cell / arm).glob("rep*"), key=lambda p: int(p.name[3:]))
        for rep in reps:
            try:
                e, m, n = analyze(rep)
            except Exception as ex:  # noqa: BLE001
                print("#", rep, ex, file=sys.stderr)
                continue
            mv += m
            allev += e
        n = len(allev)
        post = [r for r in allev if "wait" in r]
        post_np = [r for r in post if not r["pre"]]
        print(f"== {cell.name}/{arm} reps={len(reps)} lost_with_followup={n} seq_mono_viol={mv}")
        if not n:
            continue
        print(f"  post_ev3 frac={len(post)/n:.3f} (n={len(post)}); of which ev3<left_preconv: "
              f"{sum(1 for r in post if r['pre'])}")
        for lab, S in (("post_ev3", post), ("post_ev3 & !preconv", post_np)):
            w = [r["wait"] for r in S]
            nn = [r["n_new"] for r in S]
            if not w:
                continue
            print(f"  {lab}: wait p50={q(w,.5):.3f} p95={q(w,.95):.3f}  n_new p50={q(nn,.5)} "
                  f"p95={q(nn,.95)} frac>=1={sum(1 for x in nn if x>0)/len(nn):.2f}")
        d_rtx = [(r["nxt"] - r["t"]) / 1e9 for r in allev]
        d_ev3 = [(r["ev3"] - r["t"]) / 1e9 for r in allev if r["ev3"] is not None]
        print(f"  d_rtx p50={q(d_rtx,.5):.3f} p95={q(d_rtx,.95):.3f}; d_ev3 p50={q(d_ev3,.5):.3f}")
        nev1 = sum(1 for r in allev if r["ev1"] is None or r["nxt"] < r["ev1"])
        b13 = sum(1 for r in allev if not (r["ev1"] is None or r["nxt"] < r["ev1"])
                  and (r["ev3"] is None or r["nxt"] < r["ev3"]))
        print(f"  class: pre_ev1={nev1/n:.3f} ev1_ev3={b13/n:.3f} post_ev3={len(post)/n:.3f}")


if __name__ == "__main__":
    main()
