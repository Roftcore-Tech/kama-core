#!/usr/bin/env python3
"""[KTP2-F0 / Ajan D] TAM SONUÇ analizi (TAM-ANALIZ-BRIEF §2) — rakip kıyası + karışık akış.

Girdi: f0_cell.sh kök(ler)i (ör. runs/tam/d/out, runs/tam/b/out). YALNIZ `DONE` dosyası olan
hücreler okunur; olmayanlar "bekliyor" listesine yazılır. Ad sonu `_quinn` olan hücreler (B'nin r25
quinn hücreleri) aynı profilin taban hücresiyle birleştirilir.

Çıktı (`--out DİZİN`):
  kosular.csv           koşu başına satır (ızgara + betimsel d_* hücreleri)
  karisik_kosular.csv   karışık akış koşu başına satır
  TAM-OTOMATIK.md       bütün tablolar (geçerlilik, sonuç, karşılaştırma, K-2, el sıkışma, karışık)
  SHA256SUMS            üç dosyanın sha256'sı

Yöntem:
* Birim = koşu. Hücre içi dağılım: medyan [Q1–Q3] (min–maks).
* Teslim penceresi D = hücrenin kendi `--max-deadline-s` değeri (B ızgarası: 1000 B → 60 s; 65 B →
  r1k/r12k 30 s, r25 60 s). Pencere listesi {10 s, 30 s, D}.
* Kollar arası fark: completion(D) koşu-başı değerlerinin MEDYAN farkı; %95 aralık koşu birimli
  yüzdelik bootstrap (B = 2000, her çift için sha256(hücre|kol|kol|ölçü)'den türetilen sabit
  tohum → aynı girdi aynı çıktı). Ön-kayıtlı hüküm kuralı (RAPOR-D §7) AYRICA uygulanır: iki kolun
  medyan GA'ları (sıra istatistiği, binom) örtüşmüyorsa KAZANILDI/KAYBEDİLDİ; N < 30 ya da stres
  hücresi (l50/l80) → betimsel.
* El sıkışma yok: rakip kolda tel baytı 0 ve teslim 0 → "el-sıkışma-yok" (geçerli 0, ayrı sayılır).
* tcp kolları D-13 sınırlamasını taşır (veth TSO açık; ileri yön ölçülen kayıp sayaç yapıtıyla düşük).
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import random
import re
import statistics
import sys

sys.dont_write_bytecode = True  # depo ağacına __pycache__ yazma
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import karisik_analiz as KA  # noqa: E402
from rakip_analiz import (  # noqa: E402
    INF, PROFILE_RE, Run, fmt, load_run, median_ci, quantile_nr, quartiles,
)

KAMA = ("taban", "aday")
# Karışık akış gözlem penceresi: son plan ≈ 30 s + tail 30 s → ∞ (gelmedi) bu değerde sansürlenir.
MIX_CENSOR_S = 60.0
STRESS = (50, 80)
BOOT_B = 2000


def is_kama(arm: str) -> bool:
    return arm in KAMA or any(arm.startswith(k + "_") for k in KAMA)


def loss_of(cell: str) -> int | None:
    m = PROFILE_RE.search(cell)
    return int(m.group(3)) if m else None


def run_class(r: Run) -> str:
    if r.rc is None and not r.ok:
        return "çıktı-yok"
    if not r.ok:
        return "zaman-aşımı" if r.rc == 124 else f"düşen(rc={r.rc})"
    delivered = sum(1 for x in r.lat_s if not math.isinf(x))
    if not is_kama(r.arm) and r.udp_tx == 0 and delivered == 0:
        return "el-sıkışma-yok"
    return "ok"


def meas_loss(side: dict) -> float:
    if not side or not side.get("ok"):
        return float("nan")
    n = side.get("packets", 0) + side.get("drops", 0)
    return side.get("drops", 0) / n if n else float("nan")


def boot_diff(a: list[float], b: list[float], key: str) -> tuple[float, float, float] | None:
    """medyan(a) − medyan(b) ve %95 yüzdelik bootstrap aralığı (koşu birimli, sabit tohum)."""
    if len(a) < 2 or len(b) < 2:
        return None
    if any(math.isinf(x) or math.isnan(x) for x in a + b):
        return None
    seed = int(hashlib.sha256(key.encode()).hexdigest()[:12], 16)
    rng = random.Random(seed)
    na, nb = len(a), len(b)
    diffs = []
    for _ in range(BOOT_B):
        sa = [a[rng.randrange(na)] for _ in range(na)]
        sb = [b[rng.randrange(nb)] for _ in range(nb)]
        diffs.append(statistics.median(sa) - statistics.median(sb))
    diffs.sort()
    lo = diffs[int(0.025 * BOOT_B)]
    hi = diffs[int(0.975 * BOOT_B) - 1]
    return statistics.median(a) - statistics.median(b), lo, hi


def prereg(a: list[float], b: list[float]) -> str:
    ca, cb = median_ci(a), median_ci(b)
    if ca is None or cb is None:
        return "ölçülemedi(n<6)"
    if ca[0] > cb[1]:
        return "KAZANILDI"
    if ca[1] < cb[0]:
        return "KAYBEDİLDİ"
    return "ayrışmadı"


def qfmt(vals: list[float], nd: int = 3) -> str:
    q = quartiles(vals)
    return f"{fmt(q[2], nd)} [{fmt(q[1], nd)}–{fmt(q[3], nd)}] ({fmt(q[0], nd)}–{fmt(q[4], nd)})"


# ───────────────────────────── keşif ─────────────────────────────


def discover(roots: list[str]):
    cells: dict[str, dict[str, list[Run]]] = {}
    mixed: dict[str, str] = {}
    pending: list[str] = []
    for root in roots:
        for cell in sorted(os.listdir(root)):
            cdir = os.path.join(root, cell)
            if not os.path.isdir(cdir):
                continue
            if not os.path.exists(os.path.join(cdir, "DONE")):
                pending.append(cell)
                continue
            arms = [a for a in sorted(os.listdir(cdir)) if os.path.isdir(os.path.join(cdir, a))]
            if any(os.path.exists(os.path.join(cdir, a, "rep1", "mixed_tx.jsonl")) for a in arms):
                mixed[cell] = cdir
                continue
            base = re.sub(r"_quinn$", "", cell)
            for arm in arms:
                adir = os.path.join(cdir, arm)
                reps = sorted((d for d in os.listdir(adir) if re.fullmatch(r"rep\d+", d)),
                              key=lambda x: int(x[3:]))
                for rep in reps:
                    r = load_run(base, arm, int(rep[3:]), os.path.join(adir, rep))
                    cells.setdefault(base, {}).setdefault(arm, []).append(r)
    return cells, mixed, pending


def cell_deadline(arms: dict[str, list[Run]]) -> float:
    ds = [r.max_deadline_ns for runs in arms.values() for r in runs if r.max_deadline_ns]
    return (statistics.median(ds) / 1e9) if ds else 60.0


# ───────────────────────────── CSV ─────────────────────────────


CSV_COLS = ["cell", "arm", "rep", "class", "rc", "transport", "cc", "payload", "msgs", "deadline_s",
            "setup_s", "c10_ss", "c30_ss", "cD_ss", "cD_cold", "cD_ow", "lat_p50_s", "lat_p95_s",
            "ow_p95_s", "fwd_bytes", "fwd_pkts", "fwd_drops", "rev_bytes", "rev_pkts", "rev_drops",
            "loss_fwd", "loss_rev", "pre_dirty", "kama_data", "kama_retx", "kama_dup", "kama_fec",
            "kama_ctrl", "tcp_retrans_segs", "quinn_lost_pkts_t0"]


def run_row(r: Run, D: float) -> dict:
    ow95 = quantile_nr(r.ow_s, 0.95) if r.ow_s else float("nan")
    return {
        "cell": r.cell, "arm": r.arm, "rep": r.rep, "class": run_class(r), "rc": r.rc,
        "transport": r.transport, "cc": r.cc, "payload": r.payload, "msgs": r.n, "deadline_s": D,
        "setup_s": r.setup_ns / 1e9, "c10_ss": r.completion(10.0), "c30_ss": r.completion(30.0),
        "cD_ss": r.completion(D), "cD_cold": r.completion(D, True), "cD_ow": r.ow_completion(D),
        "lat_p50_s": quantile_nr(r.lat_s, 0.5) if r.lat_s else float("nan"),
        "lat_p95_s": quantile_nr(r.lat_s, 0.95) if r.lat_s else float("nan"),
        "ow_p95_s": ow95,
        "fwd_bytes": r.fwd.get("bytes", ""), "fwd_pkts": r.fwd.get("packets", ""),
        "fwd_drops": r.fwd.get("drops", ""), "rev_bytes": r.rev.get("bytes", ""),
        "rev_pkts": r.rev.get("packets", ""), "rev_drops": r.rev.get("drops", ""),
        "loss_fwd": meas_loss(r.fwd), "loss_rev": meas_loss(r.rev), "pre_dirty": int(r.contaminated()),
        "kama_data": r.tx_comp.get("data", "") if r.transport.startswith("kama") else "",
        "kama_retx": r.tx_comp.get("retx", "") if r.transport.startswith("kama") else "",
        "kama_dup": r.tx_comp.get("dup", "") if r.transport.startswith("kama") else "",
        "kama_fec": r.tx_comp.get("fec", "") if r.transport.startswith("kama") else "",
        "kama_ctrl": r.tx_comp.get("ctrl", "") if r.transport.startswith("kama") else "",
        "tcp_retrans_segs": r.tcp_snmp.get("RetransSegs", "") if r.transport.startswith("tcp") else "",
        "quinn_lost_pkts_t0": r.quinn.get("lost_packets_t0", "") if r.quinn else "",
    }


def fmtcsv(v):
    if isinstance(v, float):
        if math.isnan(v):
            return ""
        if math.isinf(v):
            return "inf"
        return f"{v:.6g}"
    return v


def p95_cens(r: Run, D: float) -> float:
    """Koşu-başı gecikme p95'i, D'de sansürlü (teslim edilmeyen ya da > D → D)."""
    if not r.ok or not r.lat_s:
        return D
    return min(quantile_nr(r.lat_s, 0.95), D)


def p50_cens(r: Run, D: float) -> float:
    if not r.ok or not r.lat_s:
        return D
    return min(quantile_nr(r.lat_s, 0.5), D)


def wire_per_msg(r: Run) -> float:
    k = sum(1 for x in r.lat_s if not math.isinf(x))
    if not (r.fwd.get("ok") and r.rev.get("ok")) or k == 0:
        return float("nan")
    return (r.fwd.get("bytes", 0) + r.rev.get("bytes", 0)) / k


def s4_summary(cells) -> list[str]:
    """Hücre başına tek satır: dört kolun completion@D, p95 (D'de sansürlü), tel B/teslim ve fark
    hükümleri. Fark aralıkları §3'te."""
    md = ["## 0. S4 özeti — ızgara (B) hücreleri\n",
          "Kol başına medyan: completion@D (ss) · p95 s (D'de sansürlü) · tel B/teslim edilen mesaj. "
          "Kama = bench üst sınırı [D-9]; tcp [D-13]. Hüküm sütunları: c = completion (ön-kayıtlı GA "
          "kuralı), p95 = gecikme farkının %95 bootstrap aralığı 0'ı dışlıyorsa yön (K = Kama iyi, R = "
          "rakip iyi, = dışlamıyor). taban kolu esas alınır; aday ayrı sütunda.\n",
          "| hücre | D s | taban [D-9] | aday [D-9] | quinn | tcp [D-13] | taban↔quinn c / p95 | taban↔tcp c / p95 | aday↔taban c / p95 |",
          "|---|---|---|---|---|---|---|---|---|"]
    for cell in sorted(c for c in cells if c.startswith("b_")):
        arms = cells[cell]
        D = cell_deadline(arms)
        stress = (loss_of(cell) or 0) in STRESS

        def st(arm):
            if arm not in arms:
                return "—"
            runs = arms[arm]
            c = quartiles([r.completion(D) for r in runs])[2]
            p = quartiles([p95_cens(r, D) for r in runs])[2]
            w = quartiles([x for x in (wire_per_msg(r) for r in runs) if not math.isnan(x)])[2]
            pt = f"≥{D:g}" if p >= D else f"{p:.1f}"
            return f"{c:.2f} · {pt} · {fmt(w, 0)} (N={len(runs)})"

        def vd(a, b):
            if a not in arms or b not in arms:
                return "—"
            ra, rb = arms[a], arms[b]
            ca = [r.completion(D) for r in ra]
            cb = [r.completion(D) for r in rb]
            if len(ra) < 30 or len(rb) < 30 or stress:
                cv = "betimsel"
            else:
                cv = {"KAZANILDI": "K", "KAYBEDİLDİ": "R", "ayrışmadı": "="}.get(prereg(ca, cb), "?")
            bd = boot_diff([p95_cens(r, D) for r in ra], [p95_cens(r, D) for r in rb], f"{cell}|{a}|{b}|p95")
            if bd is None:
                pv = "—"
            elif bd[2] < 0:
                pv = f"K ({bd[0]:+.1f} s)"
            elif bd[1] > 0:
                pv = f"R ({bd[0]:+.1f} s)"
            else:
                pv = "="
            return f"{cv} / {pv}"

        md.append(f"| {cell} | {D:g} | {st('taban')} | {st('aday')} | {st('quinn')} | {st('tcp')} | "
                  f"{vd('taban', 'quinn')} | {vd('taban', 'tcp')} | {vd('aday', 'taban')} |")
    return md


# ───────────────────────────── rapor ─────────────────────────────


def arm_order(arms) -> list[str]:
    pri = {"taban": 0, "aday": 1, "quinn": 2, "tcp": 3}
    return sorted(arms, key=lambda a: (pri.get(a, 9), a))


def grid_md(cells, pending) -> list[str]:
    md = []
    md.append("## 1. Geçerlilik — ızgara ve betimsel hücreler\n")
    md.append("Sınıflar: ok · el-sıkışma-yok (geçerli 0) · zaman-aşımı · düşen · çıktı-yok. Kayıp: "
              "netem `drops/(packets+drops)`, koşu medyanı (min–maks). tcp ileri yönü D-13 "
              "(skb başına kayıp; segment sayan `packets` nedeniyle düşük görünür).\n")
    md.append("| hücre | kol | N | sınıflar | kirli pre | kayıp ileri | kayıp dönüş |")
    md.append("|---|---|---|---|---|---|---|")
    for cell in sorted(cells):
        for arm in arm_order(cells[cell]):
            runs = cells[cell][arm]
            cls: dict[str, int] = {}
            for r in runs:
                c = run_class(r)
                cls[c] = cls.get(c, 0) + 1
            lf = [meas_loss(r.fwd) for r in runs if not math.isnan(meas_loss(r.fwd))]
            lr = [meas_loss(r.rev) for r in runs if not math.isnan(meas_loss(r.rev))]
            qf, qr = quartiles(lf), quartiles(lr)
            md.append(f"| {cell} | {arm} | {len(runs)} | " + " ".join(f"{k}={v}" for k, v in sorted(cls.items()))
                      + f" | {sum(r.contaminated() for r in runs)} | {fmt(qf[2])} ({fmt(qf[0])}–{fmt(qf[4])})"
                      f" | {fmt(qr[2])} ({fmt(qr[0])}–{fmt(qr[4])}) |")
    if pending:
        md.append("")
        md.append("Bekliyor (DONE yok; bu turda analiz dışı): " + ", ".join(sorted(pending)))
    md.append("\n## 2. Sonuç tabloları — completion (gönderen teyidi), alıcı tek-yön, gecikme, tel\n")
    md.append("Kama değerleri bench üst sınırıdır [D-9]. D = hücrenin teslim penceresi.\n")
    md.append("| hücre | kol | N | c@10 s | c@30 s | c@D (ss) | c@D (cold) | c@D alıcı | p50 s | p95 s | kurulum s | tel B / teslim | yararlı/ileri |")
    md.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for cell in sorted(cells):
        arms = cells[cell]
        D = cell_deadline(arms)
        for arm in arm_order(arms):
            runs = arms[arm]
            tag = " [D-9]" if is_kama(arm) else (" [D-13]" if arm.startswith("tcp") else "")
            ow = [r.ow_completion(D) for r in runs if r.ow_s is not None]
            p50 = [quantile_nr(r.lat_s, 0.5) if (r.ok and r.lat_s) else INF for r in runs]
            p95 = [quantile_nr(r.lat_s, 0.95) if (r.ok and r.lat_s) else INF for r in runs]
            setup = [r.setup_ns / 1e9 for r in runs if r.ok]
            wire, useful = [], []
            for r in runs:
                k = sum(1 for x in r.lat_s if not math.isinf(x))
                if r.fwd.get("ok") and r.rev.get("ok"):
                    tot = r.fwd.get("bytes", 0) + r.rev.get("bytes", 0)
                    if k:
                        wire.append(tot / (k * max(1, r.payload)))
                    if r.fwd.get("bytes"):
                        useful.append(k * r.payload / r.fwd["bytes"])
            md.append(
                f"| {cell} (D={D:g}) | {arm}{tag} | {len(runs)} | "
                f"{qfmt([r.completion(10.0) for r in runs])} | {qfmt([r.completion(30.0) for r in runs])} | "
                f"{qfmt([r.completion(D) for r in runs])} | {fmt(quartiles([r.completion(D, True) for r in runs])[2])} | "
                f"{fmt(quartiles(ow)[2]) if ow else '—'} | {fmt(quartiles(p50)[2], 2)} | {fmt(quartiles(p95)[2], 2)} | "
                f"{fmt(quartiles(setup)[2], 2)} | {fmt(quartiles(wire)[2], 2)} | {fmt(quartiles(useful)[2])} |")
    md.append("\n## 3. Kollar arası fark — completion@D (ss / cold / alıcı)\n")
    md.append("Δ = medyan(Kama kolu) − medyan(rakip); [aralık] = %95 koşu-birimli bootstrap. Hüküm = "
              "ön-kayıtlı medyan-GA kuralı (N ≥ 30 ve stres dışı); diğerleri betimsel.\n")
    md.append("| hücre | Kama kolu | rakip | Δ ss [%95] | hüküm ss | Δ cold [%95] | hüküm cold | Δ alıcı [%95] "
              "| Δ p95 s (D'de sansürlü) [%95] | Δ tel B/teslim [%95] |")
    md.append("|---|---|---|---|---|---|---|---|---|---|")
    for cell in sorted(cells):
        arms = cells[cell]
        D = cell_deadline(arms)
        stress = (loss_of(cell) or 0) in STRESS
        kas = [a for a in arm_order(arms) if is_kama(a)]
        rivals = [a for a in arm_order(arms) if not is_kama(a)]
        pairs = [(k, r) for k in kas for r in rivals]
        if "taban" in arms and "aday" in arms:
            pairs.append(("aday", "taban"))
        for k, rv in pairs:
            ra, rb = arms[k], arms[rv]
            row = [cell, k, rv]
            for view in ("ss", "cold", "ow"):
                if view == "ow":
                    a = [r.ow_completion(D) for r in ra if r.ow_s is not None]
                    b = [r.ow_completion(D) for r in rb if r.ow_s is not None]
                else:
                    a = [r.completion(D, view == "cold") for r in ra]
                    b = [r.completion(D, view == "cold") for r in rb]
                bd = boot_diff(a, b, f"{cell}|{k}|{rv}|{view}") if a and b else None
                s = "—" if bd is None else f"{bd[0]:+.3f} [{bd[1]:+.3f}, {bd[2]:+.3f}]"
                row.append(s)
                if view != "ow":
                    if len(a) < 30 or len(b) < 30 or stress:
                        row.append("betimsel")
                    else:
                        row.append(prereg(a, b))
            bd = boot_diff([p95_cens(r, D) for r in ra], [p95_cens(r, D) for r in rb], f"{cell}|{k}|{rv}|p95")
            row.append("—" if bd is None else f"{bd[0]:+.2f} [{bd[1]:+.2f}, {bd[2]:+.2f}]")
            wa = [x for x in (wire_per_msg(r) for r in ra) if not math.isnan(x)]
            wb = [x for x in (wire_per_msg(r) for r in rb) if not math.isnan(x)]
            bd = boot_diff(wa, wb, f"{cell}|{k}|{rv}|wire") if wa and wb else None
            row.append("—" if bd is None else f"{bd[0]:+.0f} [{bd[1]:+.0f}, {bd[2]:+.0f}]")
            md.append("| " + " | ".join(row) + " |")
    return md


def k2_md(cells) -> list[str]:
    md = ["\n## 4. K-2 — completion@D medyanının 0,90 altına düştüğü ilk kayıp\n",
          "Birincil kayıplar l0–l30; l50/l80 stres (betimsel). `>lX` = ölçülen en yüksek kayba dek "
          "düşmedi. Görünüm: ss (gönderen teyidi) / cold (kurulum dahil) / alıcı (tek-yön).\n",
          "| hız | gecikme | yük B | kol | ss: kayıp→medyan (N) | kesişim ss | kesişim cold | kesişim alıcı |",
          "|---|---|---|---|---|---|---|---|"]
    groups: dict[tuple, dict[int, list[Run]]] = {}
    for cell, arms in cells.items():
        m = PROFILE_RE.search(cell)
        if not m or not cell.startswith("b_"):
            continue
        for arm, runs in arms.items():
            pay = next((r.payload for r in runs if r.payload), 0)
            groups.setdefault((m.group(1), m.group(2), pay, arm), {})[int(m.group(3))] = runs
    for (rate, delay, pay, arm), byl in sorted(groups.items(), key=lambda kv: (kv[0][0], kv[0][1], kv[0][2], arm_order([kv[0][3]]))):
        losses = sorted(byl)
        seq, cross = [], {"ss": None, "cold": None, "ow": None}
        for l in losses:
            runs = byl[l]
            D = statistics.median([r.max_deadline_ns for r in runs if r.max_deadline_ns] or [60e9]) / 1e9
            vals = {"ss": [r.completion(D) for r in runs], "cold": [r.completion(D, True) for r in runs],
                    "ow": [r.ow_completion(D) for r in runs if r.ow_s is not None]}
            seq.append(f"l{l}→{fmt(quartiles(vals['ss'])[2])}({len(runs)})")
            for v in cross:
                if l in STRESS:
                    continue
                if cross[v] is None and vals[v] and quartiles(vals[v])[2] < 0.90:
                    cross[v] = l
        prim = [l for l in losses if l not in STRESS]
        top = max(prim) if prim else None

        def cs(v):
            return f"l{cross[v]}" if cross[v] is not None else (f">l{top}" if top is not None else "—")

        tag = " [D-9]" if is_kama(arm) else (" [D-13]" if arm.startswith("tcp") else "")
        md.append(f"| {rate} | {delay} | {pay} | {arm}{tag} | {', '.join(seq)} | {cs('ss')} | {cs('cold')} | {cs('ow')} |")
    return md


def hs_md(cells) -> list[str]:
    md = ["\n## 5. quinn el sıkışma — bağlanamayan oran ve kurulum süresi\n",
          "| hücre | kol | N | el-sıkışma-yok | oran [%95 Clopper–Pearson] | bağlananlarda kurulum s medyan [Q1–Q3] (min–maks) |",
          "|---|---|---|---|---|---|"]
    for cell in sorted(cells):
        for arm in arm_order(cells[cell]):
            if not arm.startswith("quinn"):
                continue
            runs = cells[cell][arm]
            k = sum(1 for r in runs if run_class(r) == "el-sıkışma-yok")
            n = len(runs)
            lo, hi = clopper(k, n)
            setup = [r.setup_ns / 1e9 for r in runs if run_class(r) == "ok"]
            md.append(f"| {cell} | {arm} | {n} | {k} | {k / n:.2f} [{lo:.2f}, {hi:.2f}] | "
                      f"{qfmt(setup, 1) if setup else '—'} |")
    return md


def clopper(k: int, n: int, conf: float = 0.95) -> tuple[float, float]:
    """Kesin binom (Clopper–Pearson) aralığı — ikiye bölmeyle, kütüphanesiz."""
    if n == 0:
        return 0.0, 1.0
    a = (1 - conf) / 2
    lower = 0.0 if k == 0 else _bisect_lower(n, k, a)
    upper = 1.0 if k == n else _bisect_upper(n, k, a)
    return lower, upper


def _bisect_lower(n, k, a):
    # P(X >= k | p) = a/2  → artan p ile artar
    lo, hi = 0.0, 1.0
    for _ in range(60):
        mid = (lo + hi) / 2
        tail = sum(math.comb(n, i) * mid ** i * (1 - mid) ** (n - i) for i in range(k, n + 1))
        if tail < a:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


def _bisect_upper(n, k, a):
    # P(X <= k | p) = a/2 → azalan p ile artar
    lo, hi = 0.0, 1.0
    for _ in range(60):
        mid = (lo + hi) / 2
        cdfv = sum(math.comb(n, i) * mid ** i * (1 - mid) ** (n - i) for i in range(0, k + 1))
        if cdfv > a:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


# ───────────────────────────── karışık ─────────────────────────────


MIX_COLS = ["cell", "arm", "rep", "connected", "rc", "setup_s", "ptt@0.4", "ptt@0.8", "ptt@1.2",
            "text_p50", "text_p95", "text@10", "file_done_s", "file_frac", "fwd_bytes", "rev_bytes",
            "delivered_payload_bytes", "wire_per_useful"]


def mixed_runs(cdir: str):
    out = {}
    for arm in sorted(os.listdir(cdir)):
        adir = os.path.join(cdir, arm)
        if not os.path.isdir(adir):
            continue
        reps = sorted((d for d in os.listdir(adir) if re.fullmatch(r"rep\d+", d)), key=lambda x: int(x[3:]))
        rows = []
        for rep in reps:
            d = os.path.join(adir, rep)
            r = KA.load_run(d)
            m = KA.per_run(r, [0.4, 0.8, 1.2]) if "flows" in r else {}
            # alınan faydalı bayt (alıcı günlüğü)
            got = 0
            p = os.path.join(d, "mixed_rx.jsonl")
            if os.path.exists(p):
                for line in open(p, encoding="utf-8"):
                    try:
                        got += int(json.loads(line)["rx"]["bytes"])
                    except (ValueError, KeyError):
                        pass
            fb, rb = m.get("fwd_bytes", float("nan")), m.get("rev_bytes", float("nan"))
            rows.append({
                "cell": os.path.basename(cdir), "arm": arm, "rep": int(rep[3:]),
                "connected": int(bool(r.get("meta", {}).get("connected"))), "rc": r.get("rc"),
                "setup_s": r.get("meta", {}).get("setup_ns", 0) / 1e9,
                "ptt@0.4": m.get("ptt@0.4", float("nan")), "ptt@0.8": m.get("ptt@0.8", float("nan")),
                "ptt@1.2": m.get("ptt@1.2", float("nan")),
                "text_p50": m.get("text_p50", float("nan")), "text_p95": m.get("text_p95", float("nan")),
                "text@10": m.get("text@10", float("nan")),
                "file_done_s": m.get("file_done_s", float("nan")), "file_frac": m.get("file_frac", float("nan")),
                "fwd_bytes": fb, "rev_bytes": rb, "delivered_payload_bytes": got,
                "wire_per_useful": (fb + rb) / got if got else float("nan"),
            })
        out[arm] = rows
    return out


def mixed_md(mixed: dict[str, str], prod_rows: list[str]) -> tuple[list[str], list[dict]]:
    md = ["\n## 6. Karışık akış (taşıma düzeyi; teslim alıcıda damgalı)\n",
          "Kama kolları bench üst sınırı [D-9]: S1 `Reorder` sıralaması ve Noise baytları yok. tcp [D-13].\n"]
    allrows = []
    for cell, cdir in sorted(mixed.items()):
        arms = mixed_runs(cdir)
        for rows in arms.values():
            allrows += rows
        md.append(f"### {cell}\n")
        md.append("| kol | N (bağlanmadı) | TEXT p50 s | TEXT p95 s | PTT ≤0,8 s | PTT ≤1,2 s | dosya bitiş s | dosya oranı | tel B / faydalı B |")
        md.append("|---|---|---|---|---|---|---|---|---|")
        pri = {"taban": 0, "aday": 1, "quinn": 2, "tcp": 3}
        for arm in sorted(arms, key=lambda x: (pri.get(x.rsplit("_", 1)[0], 9), x)):
            rows = arms[arm]
            pre = arm.rsplit("_", 1)[0]
            tag = " [D-9]" if pre in KAMA else (" [D-13]" if pre == "tcp" else "")

            def col(k, nd=2):
                v = [x[k] for x in rows if not (isinstance(x[k], float) and math.isnan(x[k]))]
                return qfmt(v, nd) if v else "—"
            md.append(f"| {arm}{tag} | {len(rows)} ({sum(1 for x in rows if not x['connected'])}) | "
                      f"{col('text_p50')} | {col('text_p95')} | {col('ptt@0.8', 3)} | {col('ptt@1.2', 3)} | "
                      f"{col('file_done_s', 1)} | {col('file_frac', 3)} | {col('wire_per_useful', 3)} |")
        md.append("")
        md.append("Dosya kaynaklı fark (dosyalı − dosyasız, aynı taşıma) ve taşımalar arası fark (dosyalı): "
                  f"Δ medyan [%95 koşu-birimli bootstrap]. TEXT p95 ve dosya bitişi gözlem penceresinde "
                  f"sansürlüdür: ∞ (pencere içinde gelmedi) → {MIX_CENSOR_S:g} s.\n")
        md.append("| karşılaştırma | TEXT p95 Δ s | PTT ≤0,8 s Δ | dosya bitiş Δ s |")
        md.append("|---|---|---|---|")
        prefixes = sorted({a.rsplit("_", 1)[0] for a in arms})
        comps = [(f"{p}_mix", f"{p}_nofile") for p in prefixes if f"{p}_mix" in arms and f"{p}_nofile" in arms]
        comps += [(f"{a}_mix", f"{b}_mix") for a in ("taban", "aday") for b in ("quinn", "tcp")
                  if f"{a}_mix" in arms and f"{b}_mix" in arms]
        comps += [("aday_mix", "taban_mix")] if "aday_mix" in arms and "taban_mix" in arms else []
        for a, b in comps:
            cells_ = []
            for k in ("text_p95", "ptt@0.8", "file_done_s"):
                va = [min(x[k], MIX_CENSOR_S) for x in arms[a] if not (isinstance(x[k], float) and math.isnan(x[k]))]
                vb = [min(x[k], MIX_CENSOR_S) for x in arms[b] if not (isinstance(x[k], float) and math.isnan(x[k]))]
                bd = boot_diff(va, vb, f"{cell}|{a}|{b}|{k}") if va and vb else None
                cells_.append("—" if bd is None else f"{bd[0]:+.2f} [{bd[1]:+.2f}, {bd[2]:+.2f}]")
            md.append(f"| {a} − {b} | " + " | ".join(cells_) + " |")
        if prod_rows:
            md.append("")
            md += prod_rows
        md.append("")
    return md, allrows


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("roots", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--product-note", default="", help="karışık bölüme eklenecek ürün yolu satırları (md dosyası)")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    cells, mixed, pending = discover(a.roots)
    md = ["# KTP2-F0 / D — TAM SONUÇ (otomatik; tam_analiz.py)\n",
          "Kökler: " + ", ".join(a.roots) + "\n"]
    md += s4_summary(cells)
    md += grid_md(cells, pending)
    md += k2_md(cells)
    md += hs_md(cells)
    prod = []
    if a.product_note and os.path.exists(a.product_note):
        prod = open(a.product_note, encoding="utf-8").read().splitlines()
    mmd, mrows = mixed_md(mixed, prod)
    md += mmd
    p_md = os.path.join(a.out, "TAM-OTOMATIK.md")
    with open(p_md, "w", encoding="utf-8") as f:
        f.write("\n".join(md) + "\n")
    p_csv = os.path.join(a.out, "kosular.csv")
    with open(p_csv, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=CSV_COLS)
        w.writeheader()
        for cell in sorted(cells):
            D = cell_deadline(cells[cell])
            for arm in arm_order(cells[cell]):
                for r in cells[cell][arm]:
                    w.writerow({k: fmtcsv(v) for k, v in run_row(r, D).items()})
    p_mcsv = os.path.join(a.out, "karisik_kosular.csv")
    with open(p_mcsv, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=MIX_COLS)
        w.writeheader()
        for row in mrows:
            w.writerow({k: fmtcsv(v) for k, v in row.items()})
    with open(os.path.join(a.out, "SHA256SUMS"), "w", encoding="utf-8") as f:
        for p in (p_md, p_csv, p_mcsv):
            f.write(f"{hashlib.sha256(open(p, 'rb').read()).hexdigest()}  {os.path.basename(p)}\n")
    print(f"hücre {len(cells)} (+ karışık {len(mixed)}), bekliyor {len(pending)} → {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
