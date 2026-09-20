#!/usr/bin/env python3
"""KAMA ↔ TCP kampanyası — TEK tipli deney tanımı → manifest.csv (yalnız stdlib).

Bu üretici + çıktısı ölçümden ÖNCE commit'lenip DONDURULUR. Sonuç görüldükten sonra hücre, eşik,
horizon, N DEĞİŞTİRİLEMEZ. Tüm sabitler A-PRİORİ formülden gelir (sonuca bakılarak seçilmedi):

  iş yükü    small = 48×101 B (4848 B; DR-ARQ-PRODUCT-TAIL-1 donmuş iş yükü) · large = 101×995 B (100 495 B;
             995 B = ürün v1 mesaj tavanı, fragmenter KAPALI) — ikisi de burst sunum (aralık 0).
  RTT        low/med/high = OWD 25/125/500 ms (her yön) — DR-ARQ-PRODUCT-TAIL-1 sınıfları.
  kayıp      IID, İKİ yönde simetrik (kayıplı bağ ACK'leri de düşürür).
  deadline   D = 3×LB, LB = serialize + 1.5·RTT (TDD §3 formülü, DEĞİŞMEDİ); serialize = bayt·8/rate (rate yoksa 0).
  horizon    H = ceil(10 s + 10·RTT [+ 10·serialize])  → 11 / 13 / 20 s (rate'siz).
  ısıtma     WARM: 8 eş-boy mesaj (her kolda aynı). COLD: ısıtma yok, kurulum pencerenin içinde.
  seed       netem seed = sha256("KVT1|replika|hücre|rep|yön")[:8] mod 2^62 — kollar arasında ORTAK
             (eşleşmiş), R1/R2 ve yönler arasında AYRIK.
  sıra       hücre içinde kollar ARDIŞIK ve DÖNÜŞÜMLÜ: kol sırası (rep + hücre_indeksi) kadar döner.
"""
import csv, hashlib, math, sys

LOSSES = [10, 20, 30, 40, 50]
RTTS = {"low": 25, "med": 125, "high": 500}
WORKLOADS = {"small": (48, 101), "large": (101, 995)}
MAIN_ARMS = ["kama-v1-full", "tcp-std"]
ABL_ARMS = ["abl-off", "abl-on"]
N_DEFAULT, N_HEADLINE, WARM = 30, 100, 8
# Manşet hücreler (≤4; hipoteze göre ÖNCEDEN, N=100): makasın açılması beklenen köşe (30/high/large) + orta (30/med/large)
# + en yüksek kayıp (50/low/large; 50/high süre bütçesini aştığı için low seçildi) + TCP'nin iyi olabileceği köşe (10/low/small).
HEADLINE = {("30", "high", "large"), ("50", "low", "large"), ("30", "med", "large"), ("10", "low", "small")}
# YALNIZ ÇİZELGELEME içindir (bilimsel etkisi yok): pilot koşudan türetilmiş satır-başı ortalama saniye.
COST_S = {"none": {0: 1.0}, "low": {10: 1.0, 20: 5, 30: 9, 40: 12.5, 50: 15}, "med": {10: 4, 20: 10, 30: 16, 40: 22, 50: 26},
          "high": {10: 12, 20: 22, 30: 30, 40: 34, 50: 36}}
WORKERS = 4
COLS = ["run_id","replica","group","cell","loss_pct","rtt_class","owd_ms","rate","workload","msgs","payload",
        "arm","mode","rep","horizon_s","deadline_ms","warmup_msgs","netem_fwd","netem_rev","seed_fwd","seed_rev","worker"]

def seed(*parts):
    return int.from_bytes(hashlib.sha256("|".join(map(str, parts)).encode()).digest()[:8], "big") % (1 << 62)

def timing(rtt_cls, wl):
    owd = RTTS.get(rtt_cls, 0); rtt_ms = 2 * owd
    lb = 1.5 * rtt_ms                       # rate'siz: serialize = 0
    return math.ceil(10 + 10 * rtt_ms / 1000), max(1, math.ceil(3 * lb))

def block(rows, replica, group, cell, loss, rtt_cls, wl, arms, n, mode, cell_idx):
    owd = RTTS.get(rtt_cls, 0); msgs, pay = WORKLOADS[wl]; hz, dl = timing(rtt_cls, wl)
    if group == "control": hz, dl = 11, 225   # kontrol: low sınıfının zamanlaması, bozulma YOK
    ne = f"delay {owd}ms" + (f" loss {loss}%" if int(loss) else "")
    for rep in range(n):
        k = (rep + cell_idx) % len(arms)
        for arm in arms[k:] + arms[:k]:
            rid = f"{replica}-{group}-{cell}-{wl}-{mode}-{arm}-r{rep:03d}"
            rows.append([rid, replica, group, f"{cell}-{wl}-{mode}", loss, rtt_cls, owd, "none", wl, msgs, pay, arm, mode,
                         rep, hz, dl, WARM if mode == "warm" else 0, ne, ne,
                         seed("KVT1", replica, cell, wl, mode, rep, "fwd"), seed("KVT1", replica, cell, wl, mode, rep, "rev")])

def build(replica, with_abl, with_cold):
    rows, ci = [], 0
    # KONTROL (bozulmasız): her kol 100/100 tamamlamalı — aksi hâlde DUR.
    for wl in WORKLOADS:
        arms = MAIN_ARMS + (ABL_ARMS if (wl == "small" and with_abl) else [])
        block(rows, replica, "control", "ctl", "0", "none", wl, arms, 100, "warm", ci); ci += 1
    for loss in LOSSES:
        for rc in RTTS:
            for wl in WORKLOADS:
                head = (str(loss), rc, wl) in HEADLINE
                block(rows, replica, "main", f"iid{loss}-{rc}", str(loss), rc, wl, MAIN_ARMS,
                      N_HEADLINE if head else N_DEFAULT, "warm", ci); ci += 1
                if head and with_cold:
                    block(rows, replica, "cold", f"iid{loss}-{rc}", str(loss), rc, wl, MAIN_ARMS, N_DEFAULT, "cold", ci); ci += 1
    if with_abl:   # BİLEŞEN ABLASYONU (çıplak Sender; TCP ile aynı tabloya GİRMEZ): small, RTT low (high süre bütçesi nedeniyle dondurma ÖNCESİ çıkarıldı)
        for loss in LOSSES:
            for rc in ("low",):
                block(rows, replica, "ablation", f"iid{loss}-{rc}", str(loss), rc, "small", ABL_ARMS, N_DEFAULT, "warm", ci); ci += 1
    return rows

def main():
    a = sys.argv[1:]; out = a[a.index("--out") + 1]; replica = a[a.index("--replica") + 1]
    rows = build(replica, "--no-ablation" not in a, "--no-cold" not in a)
    if "--pilot" in a:   # SÜRE tahmini için: uç + orta hücreler, kol başına 3 tekrar (sonuç KAMPANYAYA GİRMEZ)
        keep = ("iid50-high", "iid30-med", "iid10-low", "iid40-low")
        rows = [r for r in rows if r[2] == "main" and r[3].rsplit("-", 2)[0] in keep and int(r[13]) < 3]
        for r in rows: r[0] = "PILOT-" + r[0]; r[1] = "P"
    # LPT: blokları (cell sütunu) azalan maliyetle en az yüklü işçiye ata; blok BÖLÜNMEZ.
    blocks = {}
    for r in rows:
        blocks.setdefault(r[3], []).append(r)
    cost = {c: sum(COST_S[x[5]][int(x[4])] * (0.6 if x[12] == "cold" else 1.0) for x in v) for c, v in blocks.items()}
    load = [0.0] * WORKERS
    for c in sorted(blocks, key=lambda c: (-cost[c], c)):
        w = min(range(WORKERS), key=lambda i: (load[i], i)); load[w] += cost[c]
        for x in blocks[c]: x.append(w)
    with open(out, "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n"); w.writerow(COLS); w.writerows(rows)
    from collections import Counter
    c = Counter(r[2] for r in rows); worst = sum(int(r[14]) + 1.2 for r in rows)
    print(f"manifest={out} replica={replica} rows={len(rows)} by_group={dict(c)} worst_case_h={worst/3600:.2f} "
          f"est_serial_h={sum(load)/3600:.2f} est_per_worker_h={[round(x/3600,2) for x in load]}")

if __name__ == "__main__":
    main()
