#!/usr/bin/env python3
"""[KTP2-F0 / Ajan D] D-13 betimsel doğrulama: tcp karışık akış, TSO açık ↔ gso_max_segs 1.

Kıyas: `d_mix_r12k_d300_l30` tcp_mix/tcp_nofile (TSO açık) ↔ `d_tcp_gso1_r12k_d300_l30` aynı kollar
(aynı ikili, aynı iş yükü, her iki uçta gso_max_segs = 1). Hücreler FARKLI zamanda koştu; aralık yalnız
koşular arası saçılımı taşır, oturum farkını taşımaz. Çıktı betimseldir; karar yönlendirmez.

Ölçüler (koşu-başı; fark = medyan(gso1) − medyan(TSO açık); %95 koşu birimli yüzdelik bootstrap,
B = 10000, sabit tohum): TEXT p95 ve dosya bitiş 60 s'de sansürlü; TEXT ≤10 s; PTT ≤0,8 s; dosya oranı.
Kayıp: netem ileri yön `drops/(packets+drops+qlen)`, havuz (payda: sayaç birimi) ve koşu medyanı.
Birim/segment oranı doğrudan ölçülemez (karışık kollarda /proc/net/snmp görüntüsü yok); sayaçlardan
türetilir: oran = (1 − p) / (1 − m), p = profil kaybı, m = ölçülen kayıp [türetilmiş, skb başına kaybın
p olduğu varsayımıyla]. `drops` skb, `packets` gso_segs sayar; TSO yoksa oran 1.

Kullanım: python3 -I d13_analiz.py [--tam KÖK] [--md ÇIKTI]
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import karisik_analiz as KA  # noqa: E402
import tam_analiz as TA  # noqa: E402
from rakip_analiz import fmt, quartiles, read_json_objects  # noqa: E402

CENSOR = 60.0
P = 0.30
ON = "d_mix_r12k_d300_l30"
OFF = "d_tcp_gso1_r12k_d300_l30"
BOOT = 10000
METRICS = [("text_p95", "TEXT p95 s (60 s sansür)", True), ("text@10", "TEXT ≤10 s oranı", False),
           ("ptt@0.8", "PTT ≤0,8 s oranı", False), ("file_done_s", "dosya bitiş s (60 s sansür)", True),
           ("file_frac", "dosya oranı", False), ("text_frac", "TEXT teslim oranı (pencere sonu)", False),
           ("ptt_frac", "PTT teslim oranı (pencere sonu)", False)]


def netem(obj: dict, dev: str) -> dict:
    for q in obj.get("mid_qdisc", []):
        if q.get("dev") == dev and q.get("kind") == "netem":
            return q
    return {}


def load(root: str, cell: str, arm: str) -> list[dict]:
    adir = os.path.join(root, cell, arm)
    out = []
    for rep in sorted(os.listdir(adir), key=lambda x: int(x[3:])):
        d = os.path.join(adir, rep)
        r = KA.load_run(d)
        m = KA.per_run(r, [0.8]) if "flows" in r else {}
        pre = read_json_objects(os.path.join(d, "realized_pre.json"))[-1]
        post = read_json_objects(os.path.join(d, "realized_post.json"))[-1]
        a, b = netem(pre, "mb0"), netem(post, "mb0")
        fl = r.get("flows", {})
        for fk, mk in (("text", "text_frac"), ("ptt", "ptt_frac")):
            if fl.get(fk):
                m[mk] = sum(1 for x in fl[fk] if not math.isinf(x)) / len(fl[fk])
        m.update({"rep": rep, "rc": r.get("rc"), "connected": bool(r.get("meta", {}).get("connected")),
                  "setup_s": r.get("meta", {}).get("setup_ns", 0) / 1e9,
                  "pk": b.get("packets", 0) - a.get("packets", 0), "dr": b.get("drops", 0) - a.get("drops", 0),
                  "ql": b.get("qlen", 0), "by": b.get("bytes", 0) - a.get("bytes", 0),
                  "dirty": int(bool(a.get("qlen") or a.get("backlog"))),
                  "gso_pre": pre.get("gso_max_segs", ""), "gso_post": post.get("gso_max_segs", "")})
        out.append(m)
    return out


def vals(runs: list[dict], key: str, cens: bool) -> list[float]:
    v = [x[key] for x in runs if key in x and not (isinstance(x[key], float) and math.isnan(x[key]))]
    return [min(x, CENSOR) for x in v] if cens else v


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tam", default=os.path.expanduser("~/kama-k4g-work/ktp2-f0/runs/tam/d/out"))
    ap.add_argument("--md", default="")
    a = ap.parse_args()
    md = ["# D-13 doğrulama: tcp, TSO açık ↔ gso_max_segs 1 (betimsel; r12k_d300_l30)\n"]
    md.append("| kol | ölçü | TSO açık medyan [Q1–Q3] | gso1 medyan [Q1–Q3] | fark gso1 − açık [%95] | yön |")
    md.append("|---|---|---|---|---|---|")
    data = {}
    for arm in ("tcp_mix", "tcp_nofile"):
        on, off = load(a.tam, ON, arm), load(a.tam, OFF, arm)
        data[arm] = (on, off)
        for key, label, low_good in METRICS:
            cens = key in ("text_p95", "file_done_s")
            va, vb = vals(off, key, cens), vals(on, key, cens)
            if not va or not vb:
                continue
            ds = TA.boot_sorted(va, vb, f"d13|{arm}|{key}", BOOT)
            lo, hi = ds[int(0.025 * BOOT)], ds[int(0.975 * BOOT) - 1]
            d = TA.statistics.median(va) - TA.statistics.median(vb)
            if lo > 0:
                yon = "kötüleşiyor" if low_good else "iyileşiyor"
            elif hi < 0:
                yon = "iyileşiyor" if low_good else "kötüleşiyor"
            else:
                yon = "fark gösterilmedi"
            qa, qb = quartiles(vb), quartiles(va)
            md.append(f"| {arm} | {label} | {fmt(qa[2], 3)} [{fmt(qa[1], 3)}–{fmt(qa[3], 3)}] | "
                      f"{fmt(qb[2], 3)} [{fmt(qb[1], 3)}–{fmt(qb[3], 3)}] | {d:+.3f} [{lo:+.3f}, {hi:+.3f}] | {yon} |")
    md.append("\n| kol | hücre | N | rc≠0 | bağlanmadı | kirli pre | gso geri okuma (pre/post) | ileri kayıp havuz m (±%95) | koşu medyanı | birim/segment (1−p)/(1−m) [türetilmiş] | bayt/paket |")
    md.append("|---|---|---|---|---|---|---|---|---|---|---|")
    for arm in ("tcp_mix", "tcp_nofile"):
        for cell, runs in ((ON, data[arm][0]), (OFF, data[arm][1])):
            pk = sum(x["pk"] for x in runs)
            dr = sum(x["dr"] for x in runs)
            ql = sum(x["ql"] for x in runs)
            n = pk + dr + ql
            m = dr / n
            se = 1.96 * math.sqrt(m * (1 - m) / n)
            per = [x["dr"] / (x["pk"] + x["dr"] + x["ql"]) for x in runs if x["pk"] + x["dr"] + x["ql"]]
            gso = sorted({f"{x['gso_pre'] or '—'} / {x['gso_post'] or '—'}" for x in runs})
            by = sum(x["by"] for x in runs)
            md.append(f"| {arm} | {cell} | {len(runs)} | {sum(1 for x in runs if x['rc'] != 0)} | "
                      f"{sum(1 for x in runs if not x['connected'])} | {sum(x['dirty'] for x in runs)} | "
                      f"{'; '.join(gso)} | {m:.4f} ± {se:.4f} (n = {n}) | {fmt(quartiles(per)[2], 3)} | "
                      f"{(1 - P) / (1 - m):.3f} [{(1 - P) / (1 - (m - se)):.3f}, {(1 - P) / (1 - (m + se)):.3f}] | "
                      f"{by / max(1, pk):.0f} |")
    out = "\n".join(md) + "\n"
    if a.md:
        with open(a.md, "w", encoding="utf-8") as f:
            f.write(out)
    sys.stdout.write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
