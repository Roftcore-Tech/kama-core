#!/usr/bin/env python3
"""KAMA ↔ TCP ↔ QUIC — MANİFEST EKİ (kama-vs-tcp-v1-ext1). v1'de "Yapılmadı" kalan eksenler; v1 manifestine DOKUNMAZ.

Üç kol HER hücrede ARDIŞIK ve DÖNÜŞÜMLÜ koşar (taze satırlar; v1 satırları yeniden kullanılmaz/üstüne yazılmaz):
  kama-v1-full · tcp-std · quinn (bench quinn kolu = quinn-reliable-stream, CUBIC; TLS 1.3 quinn'de kapatılamaz → TCP'ye göre dezavantaj, raporda belirtilir)
Bloklar (a-priori; v1 sonuçları GÖRÜLDÜKTEN sonra tasarlandığı için hücre seçimi v1 EKSENLERİNİN alt kümesidir, eşik/horizon formülü v1 ile AYNI):
  Q  quinn ekseni : IID {10,30,50}% × RTT {low,med} + IID {20,30}% × RTT high ; small+large ; rate yok
  B  bant ekseni  : rate 256kbit (iki yön) ; IID {10,30}% × RTT {low,med} ; small+large
  G  burst        : Gilbert–Elliott (DR-ARQ-PRODUCT-TAIL-1 parametrizasyonu: loss gemodel p 50%, p=50L/(100−L)), L {10,20}% İLERİ yönde,
                    dönüş yönü kayıpsız (o manifestteki gibi) × RTT {low,med,high} ; small+large ; rate yok
horizon H = ⌈10 s + 10·RTT + 10·serialize⌉ , deadline D = 3×(serialize + 1.5·RTT) ; serialize = yük_baytı·8/rate (rate yoksa 0)  — v1 formülü.
N = 30/kol/hücre/replika ; R1,R2 ayrık seed (alan "KVT1X") ; seed kollar arasında ORTAK.
"""
import csv, hashlib, math, sys
RTTS = {"low": 25, "med": 125, "high": 500}; WORKLOADS = {"small": (48, 101), "large": (101, 995)}
ARMS = ["kama-v1-full", "tcp-std", "quinn"]; N, WARM, WORKERS = 30, 8, 6
RATE = ("256kbit", 32000)   # bayt/s
COLS = ["run_id","replica","group","cell","loss_pct","rtt_class","owd_ms","rate","workload","msgs","payload","arm","mode","rep",
        "horizon_s","deadline_ms","warmup_msgs","netem_fwd","netem_rev","seed_fwd","seed_rev","worker"]
COST = {"low": {10: 1.5, 20: 5, 30: 9, 50: 15}, "med": {10: 4, 20: 10, 30: 16, 50: 26}, "high": {20: 22, 30: 30}}

def seed(*p): return int.from_bytes(hashlib.sha256("|".join(map(str, p)).encode()).digest()[:8], "big") % (1 << 62)
def gemodel_p(L): return round(50.0 * L / (100 - L), 2)

def block(rows, replica, group, cell, loss, rc, wl, fwd, rev, rate, ci):
    owd = RTTS[rc]; msgs, pay = WORKLOADS[wl]; ser = (msgs * pay / RATE[1]) if rate != "none" else 0.0
    rtt = 2 * owd / 1000; hz = math.ceil(round(10 + 10 * rtt + 10 * ser, 6)); dl = math.ceil(round(3000 * (ser + 1.5 * rtt), 6))
    for rep in range(N):
        k = (rep + ci) % len(ARMS)
        for arm in ARMS[k:] + ARMS[:k]:
            rows.append([f"{replica}-{group}-{cell}-{wl}-warm-{arm}-r{rep:03d}", replica, group, f"{cell}-{wl}-warm", loss, rc, owd, rate, wl,
                         msgs, pay, arm, "warm", rep, hz, dl, WARM, fwd, rev,
                         seed("KVT1X", replica, group, cell, wl, rep, "fwd"), seed("KVT1X", replica, group, cell, wl, rep, "rev")])

def main():
    a = sys.argv[1:]; out = a[a.index("--out") + 1]; replica = a[a.index("--replica") + 1]; rows, ci = [], 0
    for loss, rcs in ((10, ("low", "med")), (30, ("low", "med", "high")), (50, ("low", "med")), (20, ("high",))):
        for rc in rcs:
            for wl in WORKLOADS:
                ne = f"delay {RTTS[rc]}ms loss {loss}%"; block(rows, replica, "quic", f"iid{loss}-{rc}", loss, rc, wl, ne, ne, "none", ci); ci += 1
    for loss in (10, 30):
        for rc in ("low", "med"):
            for wl in WORKLOADS:
                ne = f"delay {RTTS[rc]}ms loss {loss}% rate {RATE[0]}"; block(rows, replica, "band", f"bw256-iid{loss}-{rc}", loss, rc, wl, ne, ne, RATE[0], ci); ci += 1
    for L in (10, 20):
        for rc in RTTS:
            for wl in WORKLOADS:
                fwd = f"delay {RTTS[rc]}ms loss gemodel {gemodel_p(L)}% 50%"; rev = f"delay {RTTS[rc]}ms"
                block(rows, replica, "burst", f"ge{L}-{rc}", L, rc, wl, fwd, rev, "none", ci); ci += 1
    blocks = {}
    for r in rows: blocks.setdefault(r[3] + r[2], []).append(r)
    cost = {c: sum(COST[x[5]].get(int(x[4]), 10) * (2.5 if x[2] == "band" and x[8] == "large" else 1.0) for x in v) for c, v in blocks.items()}
    load = [0.0] * WORKERS
    for c in sorted(blocks, key=lambda c: (-cost[c], c)):
        w = min(range(WORKERS), key=lambda i: (load[i], i)); load[w] += cost[c]
        for x in blocks[c]: x.append(w)
    with open(out, "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n"); w.writerow(COLS); w.writerows(rows)
    from collections import Counter
    print(f"manifest={out} rows={len(rows)} by_group={dict(Counter(r[2] for r in rows))} est_per_worker_h={[round(x / 3600, 2) for x in load]}")

if __name__ == "__main__":
    main()
