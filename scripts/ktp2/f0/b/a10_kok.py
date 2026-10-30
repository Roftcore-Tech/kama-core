#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] A-9 / A-10 hız çöküşü kök analizi — Ajan A'nın ürün yolu zaman serisinden.

Girdi: runs/tam/a/out/<hücre>/<kol>/rep<N>/alice.jsonl (`"k":"ts"` satırları, 200 ms; kama-node
canlı tanı aynası: st, bw, pr, srtt, qd, inb, rec.{txq,pl,inf,dq}, rtx, tlp, dup). Salt okur.
Çöküş tanımı A'nınkiyle aynı (RAPOR-A A-10): SON örnekte txq ≥ 50 ve pr < hat hızının %5'i.

Koşu başına ölçüler (backlog = txq ≥ 50):
  t_on            ilk backlog anı (s, t0'dan)
  bw_on, st_on    o andaki BtlBw ve durum
  bw_peak, t_peak backlog içinde en yüksek BtlBw
  pr_end, bw_end, txq_end, st_end   son örnek
  pay_<durum>     backlog örneklerinin durumlara bölünmesi (pay)
  probe_n         backlog içinde ProbeUp'a giriş sayısı
  w_oran          t_peak'ten sona BtlBw düşüşünün pencere başına oranı (log-doğrusal uç nokta;
                  W = 8 × (2·267/bw + 2,097 s), sender.rs:3039-3051)
  faz             "dusuk-tepe": bw_peak < hattın %5'i (backlog boyunca hiç yükselmedi) |
                  "azalma": bw_peak ≥ %5, sonda < %5

Kullanım: python3 -I a10_kok.py <runs/tam/a/out> <çıktı.csv>
"""
import csv
import json
import math
import pathlib
import sys

LINE = {"r12k": 12500.0, "r1k": 1000.0}
STATES = ("PreConverge", "Startup", "Drain", "Steady", "ProbeUp", "DrainRtprop", "RecoverArq", "RecoverFwd", "Idle")


def scan(path):
    rows = []
    t0 = None
    with open(path, errors="replace") as f:
        for line in f:
            if '"k":"mark"' in line[:14] and '"m":"t0"' in line and t0 is None:
                t0 = json.loads(line)["t"] / 1000.0
            if not line.startswith('{"k":"ts"'):
                continue
            try:
                j = json.loads(line)
            except json.JSONDecodeError:
                continue
            s = j.get("s") or []
            if not s:
                continue
            x = s[0]
            r = x.get("rec") or {}
            rows.append((j["t"] / 1000.0, x.get("st"), x.get("bw") or 0, x.get("pr") or 0, x.get("srtt"),
                         r.get("txq") or 0, r.get("pl") or 0, x.get("rtx") or 0))
    return t0, rows


def analyze(rep, line_bps):
    t0, rows = scan(rep / "alice.jsonl")
    if not rows:
        return None
    t0 = t0 or rows[0][0]
    bl = [r for r in rows if r[5] >= 50]
    last = rows[-1]
    out = {"t_end": round(last[0] - t0, 1), "pr_end": last[3], "bw_end": last[2], "txq_end": last[5],
           "pl_end": last[6], "st_end": last[1]}
    out["cokus"] = int(last[5] >= 50 and last[3] < 0.05 * line_bps)
    if not bl:
        out["backlog"] = 0
        return out
    out["backlog"] = 1
    out["t_on"] = round(bl[0][0] - t0, 1)
    out["st_on"], out["bw_on"] = bl[0][1], bl[0][2]
    peak = max(bl, key=lambda r: r[2])
    out["bw_peak"], out["t_peak"] = peak[2], round(peak[0] - t0, 1)
    for s in STATES:
        out["pay_" + s] = round(sum(1 for r in bl if r[1] == s) / len(bl), 4)
    out["probe_n"] = sum(1 for a, b in zip(bl, bl[1:]) if b[1] == "ProbeUp" and a[1] != "ProbeUp")
    # BtlBw ≥ hattın %5'i olduğu anlar ve pr = bw (kazanç 1) payı
    out["pr_eq_bw_pay"] = round(sum(1 for r in bl if r[2] and abs(r[3] - r[2]) <= 0.02 * r[2]) / len(bl), 4)
    if last[2] > 0 and peak[2] > last[2] and last[0] > peak[0]:
        dur = last[0] - peak[0]
        bw_m = math.sqrt(peak[2] * max(last[2], 1))
        W = 8 * (2 * 267 / bw_m + 2.097)
        out["w_oran"] = round(math.exp(math.log(last[2] / peak[2]) / dur * W), 4)
        out["W_s"] = round(W, 1)
    out["faz"] = "dusuk-tepe" if peak[2] < 0.05 * line_bps else ("azalma" if out["cokus"] else "yok")
    # Startup ne zaman bitti: ilk "Drain"/"Steady"/"ProbeUp"/"DrainRtprop" örneği (RecoverArq taban
    # fazı gizler; bu yüzden üst sınır). O andaki txq < 50 → Startup, dosya kuyruğu gelmeden bitti.
    exit_i = next((i for i, r in enumerate(rows) if r[1] in ("Drain", "Steady", "ProbeUp", "DrainRtprop")), None)
    if exit_i is not None:
        ex = rows[exit_i]
        out["t_cikis"], out["bw_cikis"], out["txq_cikis"] = round(ex[0] - t0, 1), ex[2], ex[5]
        out["cikis_once"] = int(ex[5] < 50)
    # 4800 B/srtt tavanı (MIN_CWND, rate.rs:101-108): backlog örneklerinde pr·srtt ≈ 4800 B
    capn = [r for r in bl if r[4] and abs(r[3] * r[4] / 1000.0 - 4800.0) <= 0.03 * 4800.0]
    out["cap4800_pay"] = round(len(capn) / len(bl), 4)
    # backlog başından 60 s içinde en yüksek pacer
    w60 = [r for r in bl if r[0] <= bl[0][0] + 60]
    out["pr_60s_maks"] = max(r[3] for r in w60)
    return out


def main():
    root = pathlib.Path(sys.argv[1])
    out = []
    for cell in sorted(p for p in root.iterdir() if p.is_dir()):
        rid = cell.name.split("_")[1]
        line_bps = LINE.get(rid)
        if line_bps is None:
            continue  # rinf: A-10 tanımı hat hızı ister
        for arm in ("taban", "aday"):
            for rep in sorted((cell / arm).glob("rep*"), key=lambda q: int(q.name[3:])):
                r = analyze(rep, line_bps)
                if r is None:
                    continue
                r.update({"hucre": cell.name, "kol": arm, "rep": int(rep.name[3:]), "hat_Bps": line_bps})
                out.append(r)
    keys = []
    for r in out:
        for k in r:
            if k not in keys:
                keys.append(k)
    with open(sys.argv[2], "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=keys)
        w.writeheader()
        w.writerows(out)
    print(len(out), "koşu")


if __name__ == "__main__":
    main()
