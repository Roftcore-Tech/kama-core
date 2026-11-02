#!/usr/bin/env python3
"""B ızgarası ham veriden koşu başına alıcı tek-yön gecikmesi (analiz_b.py:1035-1045 ile aynı tanım).
offer = t0_unix + (sent_ns - events_epoch_offset_ns); lat = rx_log.t - offer.
Çıktı: JSON {cell: {arm: [{p50,p95,lat:[...]}...]}} (lat yalnız özet istatistik için).
"""
import json, math, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sim import pct

ROOT = "/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out"


def load_rep(d):
    meta, ev = None, []
    with open(os.path.join(d, "out.jsonl")) as f:
        for line in f:
            o = json.loads(line)
            if "meta" in o:
                meta = o["meta"]
            elif "event" in o:
                ev.append(o["event"])
    rx = {}
    p = os.path.join(d, "rx_log.jsonl")
    if os.path.exists(p):
        with open(p) as f:
            for line in f:
                o = json.loads(line)
                if "u" in o and o["u"] not in rx:
                    rx[o["u"]] = o["t"] / 1e9
    t0 = meta["t0_unix_ns"] / 1e9
    off = meta.get("events_epoch_offset_ns", 0)
    offer = {e["msg_id"]: t0 + (e["sent_ns"] - off) / 1e9 for e in ev}
    lat = {i: rx[i] - t for i, t in offer.items() if i in rx}
    return meta, offer, lat


def main(cells, out):
    res = {}
    for c in cells:
        cd = os.path.join(ROOT, c)
        res[c] = {}
        for arm in ("taban", "aday", "quinn", "tcp"):
            ad = os.path.join(cd, arm)
            if not os.path.isdir(ad):
                continue
            runs = []
            for rep in sorted(os.listdir(ad)):
                rd = os.path.join(ad, rep)
                try:
                    meta, offer, lat = load_rep(rd)
                except Exception as e:  # noqa
                    continue
                n = len(offer)
                ids = sorted(offer)
                vals = [lat[i] for i in ids if i in lat]
                if not vals:
                    continue
                third = max(1, n // 3)
                first = [lat[i] for i in ids[:third] if i in lat]
                last = [lat[i] for i in ids[-third:] if i in lat]
                act = meta.get("activation") or {}
                runs.append({
                    "rep": rep, "n": n, "nrx": len(vals),
                    "p50": pct(vals, .5), "p95": pct(vals, .95),
                    "min_first": min(first) if first else None, "min_last": min(last) if last else None,
                    "p50_first": pct(first, .5) if first else None, "p50_last": pct(last, .5) if last else None,
                    "p95_skip10": pct([lat[i] for i in ids[10:] if i in lat], .95),
                    "btlbw": act.get("btlbw_bps"), "retx": act.get("retransmits"),
                    "parity": act.get("parity_emitted"), "srtt_us": act.get("srtt_us"),
                    "setup_s": (meta.get("setup_ns") or 0) / 1e9,
                })
            res[c][arm] = runs
    json.dump(res, open(out, "w"))


if __name__ == "__main__":
    cells = sorted(x for x in os.listdir(ROOT) if os.path.isdir(os.path.join(ROOT, x)))
    main(cells, sys.argv[1])
