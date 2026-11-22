#!/usr/bin/env python3
"""kama-target-net-v2 — TEK tipli deney tanımı → manifest CSV (yalnız stdlib, belirlenimli).

Bu üretici ve donma kaydındaki sha256'ları (reports/kama-target-net-v2/FREEZE.md) ölçümden ÖNCE
commit'lenir. Sonuç görüldükten sonra hücre, kol, N, horizon, tohum DEĞİŞTİRİLEMEZ. Manifest
dosyaları repoda tutulmaz: üretici belirlenimlidir (RNG yok, tarih yok, tohum = sha256 türevi),
yürütücü her başlangıçta yeniden üretip donmuş sha256 ile karşılaştırır.

Sabitler a-priori formüllerden gelir (v1 kampanyasıyla aynı; docs/reports/kama-target-net-v2-ONKAYIT.md §4):

  iş yükü   small = 48×101 B (4 848 B) · large = 101×995 B (100 495 B) — burst sunum (aralık 0).
  RTT       low/med/high = tek yön 25/125/500 ms (v1 sınıfları) · field = tek yön 300 ms (saha profili).
  horizon   H = ⌈10 s + 10·RTT + 10·ser⌉, ser = yük_baytı·8/ileri_hız (hız yoksa 0) — v1 formülü.
            İSTİSNA asym grupları: H = 180 s sabit (formül dönüş darboğazını içermez; ONKAYIT §4.3).
  deadline  D = 3·(ser + 1.5·RTT) — v1 formülü (betimsel sütun).
  ısıtma    WARM, 8 eş-boy mesaj (her kolda aynı).
  tohum     netem seed = sha256("KTN2|replika|grup|hücre|yük|rep|yön")[:8] mod 2^62 — kollar arasında
            ORTAK (eşleşmiş tasarım), replikalar ve yönler arasında AYRIK.
  sıra      hücre içinde kollar ARDIŞIK ve DÖNÜŞÜMLÜ: kol sırası (rep + hücre_indeksi) kadar döner.
  blok      (replika, hücre, rep // 10): bir işçi bloğu seri koşar (bir rep'in beş kolu ardışık).

Kullanım:
  tnet_manifest.py --replica R1|R2 --out FILE        # kampanya manifesti
  tnet_manifest.py --replica P --pilot --out FILE    # süre pilotu (sonucu kampanyaya GİRMEZ)
  tnet_manifest.py --estimate [--jobs J]             # süre aritmetiği (ONKAYIT §6 tablosu)
"""
import csv
import hashlib
import math
import sys

# ── Deney tanımı ────────────────────────────────────────────────────────────────────────────────
ARMS = ["kama-port", "kama-port-nofec", "quinn", "quinn-bbr", "tcp"]
WORKLOADS = {"small": (48, 101), "large": (101, 995)}
OWD_MS = {"none": 0, "low": 25, "med": 125, "field": 300, "high": 500}
RATE_BPS = {"none": 0, "100kbit": 12_500, "0.5kbit": 62.5}   # bayt/s (tc: kbit = 1000 bit)
N_PER_REPLICA = 30
WARM = 8
REP_CHUNK = 10
ASYM_HORIZON_S = 180
SEED_DOMAIN = "KTN2"
COLS = ["run_id", "replica", "group", "cell", "cell_idx", "loss_pct", "loss_dir", "rtt_class", "owd_ms",
        "rate_fwd", "rate_rev", "workload", "msgs", "payload", "arm", "mode", "rep", "horizon_s",
        "deadline_ms", "warmup_msgs", "netem_fwd", "netem_rev", "seed_fwd", "seed_rev", "block", "est_cost_s"]


def cells():
    """(grup, hücre_adı, kayıp%, kayıp_yönü, rtt, ileri_hız, dönüş_hızı, iş_yükü) — SIRA donmuştur."""
    out = []
    for wl in WORKLOADS:   # kontrol: bozulmasız, her kol N/N tamamlamalı
        out.append(("control", "ctl", 0, "both", "none", "none", "none", wl))
    for loss in (10, 20, 30, 40, 50):   # v1 sürekliliği: IID simetrik kayıp, hız sınırı yok
        for rc in ("low", "med", "high"):
            for wl in WORKLOADS:
                out.append(("main", f"iid{loss}-{rc}", loss, "both", rc, "none", "none", wl))
    for rc in ("med", "high"):   # düşük bant: ileri 12,5 kB/s, kayıp simetrik
        for loss in (0, 10, 30, 50):
            out.append(("band", f"bw100k-iid{loss}-{rc}", loss, "both", rc, "100kbit", "none", "small"))
        for loss in (0, 10, 30):   # large + %50: v1'de hızsız ortamda bile med/high'da iki kol ≤ 1/60 (ONKAYIT §4.2)
            out.append(("band", f"bw100k-iid{loss}-{rc}", loss, "both", rc, "100kbit", "none", "large"))
    for rev in ("0.5kbit", "none"):   # asimetrik dönüş (saha profili) ve İKİZİ (dönüş sınırsız)
        g = "asym" if rev != "none" else "asym-twin"
        tag = "rev0.5k" if rev != "none" else "revinf"
        for loss in (0, 20):
            for wl in WORKLOADS:
                out.append((g, f"asym-l{loss}-field-{tag}", loss, "fwd", "field", "100kbit", rev, wl))
    return out


def seed(*parts):
    return int.from_bytes(hashlib.sha256("|".join(map(str, parts)).encode()).digest()[:8], "big") % (1 << 62)


def ser_s(wl, rate):
    msgs, pay = WORKLOADS[wl]
    return (msgs * pay / RATE_BPS[rate]) if rate != "none" else 0.0


def timing(group, rc, wl, rate_fwd):
    if group == "control":
        return 11, 225                      # v1 kontrol zamanlaması (low sınıfı), bozulma yok
    rtt = 2 * OWD_MS[rc] / 1000.0
    ser = ser_s(wl, rate_fwd)
    hz = math.ceil(round(10 + 10 * rtt + 10 * ser, 6))
    if group in ("asym", "asym-twin"):
        hz = ASYM_HORIZON_S
    dl = max(1, math.ceil(round(3000 * (ser + 1.5 * rtt), 6)))
    return hz, dl


def netem(owd, loss, loss_dir, rate_fwd, rate_rev):
    base = f"delay {owd}ms"
    fwd = base + (f" loss {loss}%" if loss else "") + (f" rate {rate_fwd}" if rate_fwd != "none" else "")
    rl = loss if (loss and loss_dir == "both") else 0
    rev = base + (f" loss {rl}%" if rl else "") + (f" rate {rate_rev}" if rate_rev != "none" else "")
    return fwd, rev


# ── Süre modeli (YALNIZ çizelgeleme ve bütçe aritmetiği; bilimsel etkisi yok) ──────────────────
# main/control: v1 pilotundan ölçülen satır-başı ortalama saniye (kama-vs-tcp-v1 kvt_manifest.py
# COST_S; v1'in gerçekleşen süresi bu tahmini 1,28× aştı → aşağıda OVERRUN). quinn kolları el
# sıkışması ısıtmadan önce kayıplı yolda koştuğu için ×1,25 + 1 s.
V1_COST_S = {"none": {0: 1.0}, "low": {10: 1.0, 20: 5, 30: 9, 40: 12.5, 50: 15},
             "med": {10: 4, 20: 10, 30: 16, 40: 22, 50: 26}, "high": {10: 12, 20: 22, 30: 30, 40: 34, 50: 36}}
# band/asym (v1'de ölçüm yok): satır = 3 s sabit + f(kayıp)·H; f horizon'un ortalama kullanılan payı
# (a-priori, kötümser). asym'de f ≥ 0,5; asym'de ayrıca ısıtma ≈ 10 s, quinn el sıkışması ≈ 50 s
# (62 B/s dönüşte sunucu el sıkışma uçuşu ≈ 2,5–3 kB).
F_LOSS = {0: 0.15, 10: 0.30, 20: 0.45, 30: 0.60, 40: 0.75, 50: 0.85}
OVERRUN = 1.28


def est_cost(group, rc, loss, wl, arm, hz):
    quinn = arm.startswith("quinn")
    if group in ("main", "control"):
        c = V1_COST_S[rc][loss]
        return c * 1.25 + 1.0 if quinn else c
    f = F_LOSS[loss]
    c = 3.0 + f * hz
    if group == "asym":
        c = 3.0 + max(f, 0.5) * hz + 10.0 + (50.0 if quinn else 0.0)
    elif group == "asym-twin":
        c += 5.0
    return c


def build(replica, pilot=False):
    rows = []
    for ci, (g, cell, loss, ldir, rc, rf, rr, wl) in enumerate(cells()):
        msgs, pay = WORKLOADS[wl]
        hz, dl = timing(g, rc, wl, rf)
        fwd, rev = netem(OWD_MS[rc], loss, ldir, rf, rr)
        ckey = f"{cell}-{wl}"
        for rep in range(N_PER_REPLICA):
            k = (rep + ci) % len(ARMS)
            for arm in ARMS[k:] + ARMS[:k]:
                rid = f"{replica}-{g}-{ckey}-warm-{arm}-r{rep:03d}"
                rows.append([rid, replica, g, ckey, ci, loss, ldir, rc, OWD_MS[rc], rf, rr, wl, msgs, pay, arm,
                             "warm", rep, hz, dl, WARM, fwd, rev,
                             seed(SEED_DOMAIN, replica, g, ckey, rep, "fwd"),
                             seed(SEED_DOMAIN, replica, g, ckey, rep, "rev"),
                             f"{replica}/{g}/{ckey}/c{rep // REP_CHUNK}",
                             round(est_cost(g, rc, loss, wl, arm, hz), 2)])
    if pilot:
        # SÜRE pilotu: her grubun en pahalı köşeleri, kol başına 2 rep. Sonucu kampanyaya GİRMEZ;
        # analizör `P` replikasını reddeder. Yalnız satır duvar-saati süresi okunur (ONKAYIT §6.3).
        keep = {"main/iid50-high-large", "main/iid30-med-large", "band/bw100k-iid30-high-large",
                "band/bw100k-iid50-high-small", "asym/asym-l20-field-rev0.5k-large",
                "asym/asym-l20-field-rev0.5k-small", "asym-twin/asym-l20-field-revinf-large"}
        rows = [r for r in rows if f"{r[2]}/{r[3]}" in keep and int(r[16]) < 2]
        for r in rows:
            r[24] = f"{r[1]}/{r[2]}/{r[3]}/pilot"
    return rows


def lpt(blocks, jobs):
    """Uzun-işlem-önce: blokları azalan maliyetle en az yüklü işçiye (yürütücünün dinamik sırası)."""
    load = [0.0] * jobs
    for c in sorted(blocks.values(), reverse=True):
        i = min(range(jobs), key=lambda j: load[j])
        load[i] += c
    return load


def estimate(jobs):
    from collections import defaultdict
    rows = build("R1") + build("R2")
    blocks, by_group, n_by_group, worst = defaultdict(float), defaultdict(float), defaultdict(int), 0.0
    for r in rows:
        blocks[r[24]] += r[25]
        by_group[r[2]] += r[25]
        n_by_group[r[2]] += 1
        worst += 2 * int(r[17]) + 90 + 3     # yürütücü satır tavanı (2H + 90 s) + kurulum
    total = sum(by_group.values())
    print(f"satır={len(rows)}  hücre={len(cells())}  kol={len(ARMS)}  N/kol/hücre/replika={N_PER_REPLICA}")
    print("| grup | satır | model işçi-saat | ×1,28 (v1 aşımı) |")
    print("|---|---|---|---|")
    for g in ("control", "main", "band", "asym", "asym-twin"):
        print(f"| {g} | {n_by_group[g]} | {by_group[g] / 3600:.1f} | {by_group[g] * OVERRUN / 3600:.1f} |")
    print(f"| TOPLAM | {len(rows)} | {total / 3600:.1f} | {total * OVERRUN / 3600:.1f} |")
    load = lpt(blocks, jobs)
    print(f"J={jobs}: model duvar = {max(load) / 3600:.1f} sa · ×1,28 = {max(load) * OVERRUN / 3600:.1f} sa"
          f" · en uzun blok = {max(blocks.values()) * OVERRUN / 3600:.2f} sa")
    print(f"kötü-durum sınırı (her satır kendi tavanında): {worst / 3600:.0f} işçi-saat = {worst / 3600 / jobs:.0f} sa (J={jobs})")


def main():
    a = sys.argv[1:]
    if "--estimate" in a:
        estimate(int(a[a.index("--jobs") + 1]) if "--jobs" in a else 10)
        return
    out = a[a.index("--out") + 1]
    replica = a[a.index("--replica") + 1]
    rows = build(replica, pilot="--pilot" in a)
    with open(out, "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(COLS)
        w.writerows(rows)
    print(f"manifest={out} replica={replica} rows={len(rows)}")


if __name__ == "__main__":
    main()
