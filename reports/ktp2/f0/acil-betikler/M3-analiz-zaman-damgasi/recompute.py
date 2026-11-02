#!/usr/bin/env python3
"""Bağımsız yeniden hesap (M3): ham out.jsonl + rx_log.jsonl'den koşu-başı gecikme/teslim.

Kullanım: python3 -I recompute.py RUNROOT CELL [CELL...] > out.tsv
Her koşu için:
  snd_*  = gönderen teyidi (delivered_ns - sent_ns), out.jsonl
  rx_*   = alıcı tek-yön: rx_log ilk damga - (t0_unix + sent_ns - off)
  *_cD   = (<= D) / n
  *_p95_del  = yalnız teslim edilenler, doğrusal ara değer (B tanımı)
  *_p95_nr   = teslim edilmeyen = +inf, yakın-sıra ceil(0.95 n) (D tanımı; sonra min(.,D))
"""
import json
import math
import os
import sys

INF = math.inf


def lin(vals, q):
    v = sorted(vals)
    if not v:
        return None
    if len(v) == 1:
        return v[0]
    p = q * (len(v) - 1)
    lo = math.floor(p)
    hi = min(lo + 1, len(v) - 1)
    return v[lo] + (v[hi] - v[lo]) * (p - lo)


def nr(vals, q):
    v = sorted(vals)
    if not v:
        return None
    k = max(1, math.ceil(q * len(v)))
    return v[k - 1]


def load(rep):
    meta, ev = None, []
    with open(os.path.join(rep, "out.jsonl")) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            o = json.loads(line)
            if "meta" in o:
                meta = o["meta"]
            elif "event" in o:
                ev.append(o["event"])
    rx = {}
    hdr = None
    nrec = 0
    p = os.path.join(rep, "rx_log.jsonl")
    if os.path.exists(p):
        with open(p) as f:
            for line in f:
                o = json.loads(line)
                if "rx_log_schema" in o:
                    hdr = o
                    continue
                nrec += 1
                u, t = o["u"], o["t"]
                if u >= 2**64 - 1:
                    continue
                if u not in rx or t < rx[u]:
                    rx[u] = t
    return meta, ev, rx, hdr, nrec


def main():
    root = sys.argv[1]
    cells = sys.argv[2:]
    cols = ["cell", "arm", "rep", "n", "D", "setup_s", "t0u0", "snd_ndel", "snd_cD", "snd_p95_del", "snd_p95_nr",
            "snd_p95_nr_cens", "rx_nrec", "rx_ndel", "rx_cD", "rx_p95_del", "rx_p95_nr", "rx_p95_nr_cens",
            "rx_minus_snd_med", "neg_rx", "sent_mono_viol", "sent_plan_err_max_ms"]
    print("\t".join(cols))
    for cell in cells:
        cdir = os.path.join(root, cell)
        for arm in sorted(os.listdir(cdir)):
            adir = os.path.join(cdir, arm)
            if not os.path.isdir(adir):
                continue
            reps = sorted((d for d in os.listdir(adir) if d.startswith("rep")), key=lambda x: int(x[3:]))
            for rep in reps:
                rd = os.path.join(adir, rep)
                meta, ev, rx, hdr, nrec = load(rd)
                n = int(meta.get("msg_count") or len(ev))
                D = meta["max_deadline_ns"] / 1e9
                ival = meta.get("submit_interval_ns", 0) / 1e9
                t0u = meta.get("t0_unix_ns", 0)
                off = meta.get("events_epoch_offset_ns", 0)
                snd = []
                rxl = []
                diffs = []
                neg = 0
                byid = {e["msg_id"]: e for e in ev}
                prev = None
                mono = 0
                plan_err = 0.0
                for i in range(n):
                    e = byid.get(i)
                    if e is None:
                        snd.append(INF)
                        rxl.append(INF)
                        continue
                    s = e["sent_ns"]
                    if prev is not None and s < prev:
                        mono += 1
                    prev = s
                    plan_err = max(plan_err, abs((s - off) / 1e9 - i * ival))
                    d = e.get("delivered_ns")
                    sl = (d - s) / 1e9 if d is not None else INF
                    snd.append(sl)
                    if t0u:
                        su = t0u + s - off
                        if i in rx:
                            v = (rx[i] - su) / 1e9
                            if v < 0:
                                neg += 1
                            rxl.append(v)
                            if sl != INF:
                                diffs.append(sl - v)
                        else:
                            rxl.append(INF)
                    else:
                        rxl.append(INF)
                sd = [x for x in snd if x != INF]
                rdl = [x for x in rxl if x != INF]
                row = [cell, arm, rep[3:], n, D, meta.get("setup_ns", 0) / 1e9, int(t0u == 0), len(sd),
                       sum(1 for x in sd if x <= D) / n, lin(sd, .95), nr(snd, .95), min(nr(snd, .95), D),
                       nrec, len(rdl), sum(1 for x in rdl if x <= D) / n, lin(rdl, .95), nr(rxl, .95),
                       min(nr(rxl, .95), D), lin(diffs, .5), neg, mono, plan_err * 1000]
                print("\t".join("" if v is None else (f"{v:.6g}" if isinstance(v, float) else str(v)) for v in row))


if __name__ == "__main__":
    main()
