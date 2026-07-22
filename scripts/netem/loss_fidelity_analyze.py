#!/usr/bin/env python3
"""Commit-FT — LOSS-SIGNAL FIDELITY analizi (faithful harness; GERÇEK forward-loss).

Prior-turn "loss_windowed_ppm=0 heryerde → sensör-kör" hükmü rootless-TEK-ns netem'de-ölçüldü (loss-
UYGULANMADI). Bu analiz GERÇEK-forward-loss altında sensörün nonzero-okuyup-okumadığını sınar.
Kod-analizi (loss.rs): loss_windowed_ppm = PRE-RECOVERY erasure-metre (confirmed-DATA-loss/sent-window;
recovery-subtract-etmez). ⇒ gerçek-loss'ta true-loss-fraction'a-yakın-nonzero-BEKLENİR.

Girdi: reports/d1-exit/data/loss-fidelity/*-fuzzy.csv (kolon 5 = loss_windowed_ppm).
Fuzzy CSV: run_id,policy,event_index,t_ms,loss_windowed_ppm,qd_ratio_permille,jitter_ppm,
ack_confidence_permille,bandwidth_bps,btlbw,srtt_us,pacing_rate,inflight_bytes,fuzzy_ratio_permille,mode,no_rule
"""
import csv
import glob
import os
import re
import statistics as st
import sys
from collections import defaultdict

DATA = sys.argv[1] if len(sys.argv) > 1 else "reports/d1-exit/data/loss-fidelity"
RID = re.compile(r"(loss\d+)-(\d+)")

# per loss-profile: cold (ilk %25 event) vs warm (son %50 event) loss_windowed_ppm
prof = defaultdict(lambda: {"all": [], "warm": [], "srtt": [], "ackc": [], "ratio": []})
for path in sorted(glob.glob(os.path.join(DATA, "*-fuzzy.csv"))):
    base = os.path.basename(path)[: -len("-fuzzy.csv")]
    m = RID.match(base)
    if not m:
        continue
    label = m.group(1)
    with open(path) as f:
        rr = list(csv.DictReader(f))
    n = len(rr)
    warm_start = n // 2  # son %50 = warm steady-state (W_loss penceresi dolmuş)
    for i, row in enumerate(rr):
        lp = int(row["loss_windowed_ppm"])
        prof[label]["all"].append(lp)
        if i >= warm_start:
            prof[label]["warm"].append(lp)
            prof[label]["ackc"].append(int(row["ack_confidence_permille"]))
            prof[label]["srtt"].append(int(row["srtt_us"]))
            prof[label]["ratio"].append(int(row["fuzzy_ratio_permille"]))


def stats(xs):
    if not xs:
        return "n=0"
    xs2 = sorted(xs)
    return f"n={len(xs)} min={xs2[0]} p50={xs2[len(xs2)//2]} max={xs2[-1]} mean={st.mean(xs):.0f}"


print("BU ANALİZ ARQ-vs-FEC DEĞİL: SENSÖR-fidelity ölçer (loss_windowed_ppm gerçek-forward-loss-altında).\n")
print("Kod-hükmü (loss.rs): loss_windowed_ppm = PRE-RECOVERY erasure-metre (recovery-subtract-ETMEZ).")
print("Beklenen: gerçek-loss'ta warm-steady loss_windowed_ppm ≈ true-forward-loss-fraction (nonzero).\n")
print(f"{'profile':8} {'cfg-loss':9} {'warm loss_windowed_ppm':40} {'warm ack_conf(permille)':24} {'warm fuzzy_ratio':16}")
for label in sorted(prof):
    cfg = label.replace("loss", "") + "%"
    p = prof[label]
    warm_ppm = f"[{stats(p['warm'])}]"
    ackc = f"p50={sorted(p['ackc'])[len(p['ackc'])//2] if p['ackc'] else -1}"
    ratio = f"p50={sorted(p['ratio'])[len(p['ratio'])//2] if p['ratio'] else -1}"
    print(f"{label:8} {cfg:9} {warm_ppm:40} {ackc:24} {ratio:16}")

print("\n=== FIDELITY VERDICT ===")
# loss0 baseline ≈ 0; loss10/20/30 warm-mean karşılaştır.
for label in sorted(prof):
    p = prof[label]
    wm = st.mean(p["warm"]) if p["warm"] else 0
    cfg_pct = int(label.replace("loss", ""))
    expected_ppm = cfg_pct * 10000  # %X → X*10000 ppm
    if cfg_pct == 0:
        note = "baseline (loss-yok)"
    elif wm > expected_ppm * 0.3:
        note = f"SENSÖR-NONZERO (~{wm:.0f}ppm vs cfg~{expected_ppm}ppm) → gerçek-loss'u-GÖRÜYOR"
    elif wm > 1000:
        note = f"kısmi-nonzero ({wm:.0f}ppm; under-read? confirm-age/window-lag veya recovery-timing)"
    else:
        note = f"~0 ({wm:.0f}ppm) — gerçek-loss'ta-bile-sıfır → daha-derin-inceleme"
    print(f"  {label} (cfg {cfg_pct}% = {expected_ppm}ppm): warm-mean={wm:.0f}ppm → {note}")
