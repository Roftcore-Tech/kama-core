#!/usr/bin/env python3
"""FAZ 0.5 — KANAL KALİBRASYONU (hiçbir protokol kolu koşmadan ÖNCE). `unshare -rnm` içinde koşar.

Her (kayıp × RTT) hücresinde sıra numaralı sabit-hızlı UDP probe (baselines/udp_probe; ≥2000 paket):
gerçekleşen İLERİ kayıp (alıcıda benzersiz/ gönderilen), DÖNÜŞ kaybı (echo/ alıcıda benzersiz),
RTT medyan/p90, ve tc -s qdisc sayaçlarından aynı oranlar (çapraz teyit).
KABUL: gerçekleşen kayıp, ayarlananın binom %99 merkezi aralığında (iki yön AYRI) VE RTT medyanı 2·OWD ±%5.
Tutmayan hücre kampanyadan ÇIKAR (calibration.csv `accept=0`).  Kullanım: kvt_calibrate.py OUTDIR
"""
import csv, json, math, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kvt_exec as X
import kvt_manifest as M  # noqa: E402  (yalnız sabitler: LOSSES, RTTS, seed)

PROBE = os.path.join(X.REPO, "baselines/target/release/udp_probe")
COUNT, PPS, SIZE = 3000, 250, 200

def binom_interval(n, p, alpha=0.01):
    """Binomial(n,p)'nin merkezi (1-alpha) aralığı [k_lo, k_hi] — log-uzayda kesin toplam."""
    if p <= 0: return 0, 0
    lp, lq = math.log(p), math.log(1 - p); cdf, lo, hi = 0.0, None, n
    for k in range(n + 1):
        pm = math.exp(math.lgamma(n + 1) - math.lgamma(k + 1) - math.lgamma(n - k + 1) + k * lp + (n - k) * lq)
        cdf += pm
        if lo is None and cdf >= alpha / 2: lo = k
        if cdf >= 1 - alpha / 2: hi = k; break
    return lo or 0, hi

def main():
    out = sys.argv[1]; os.makedirs(out, exist_ok=True); lg = open(os.path.join(out, "calibration.log"), "a")
    if not X.topo_up(lg): sys.exit(3)
    cells = [("ctl", 0, "none", 0)] + [(f"iid{l}-{rc}", l, rc, owd) for l in M.LOSSES for rc, owd in M.RTTS.items()]
    cols = ["cell","loss_pct","rtt_class","owd_ms","sent","rx_unique","echoed","fwd_loss","rev_loss","fwd_lo","fwd_hi",
            "rev_lo","rev_hi","rtt_p50_ms","rtt_p90_ms","rtt_target_ms","tc_fwd_loss","tc_rev_loss","netem_limit",
            "loadavg1","accept_loss","accept_rtt","accept"]
    rows, port = [], 30000
    for cell, loss, rc, owd in cells:
        if not X.gate(owd, lg): X.topo_up(lg); X.gate(owd, lg)
        ne = f"delay {owd}ms" + (f" loss {loss}%" if loss else "")
        sf, sr = M.seed("KVT1-CAL", cell, "fwd"), M.seed("KVT1-CAL", cell, "rev")
        assert X.set_netem(f"{ne} seed {sf}", f"{ne} seed {sr}")
        vf, vr = X.verify("mb0", ne, sf), X.verify("ma0", ne, sr)
        if not (vf[0] and vr[0]):
            X.log(lg, f"NETEM-SKIP {cell}: {vf[2]} {vr[2]}"); continue
        port += 2; d = os.path.join(out, cell); os.makedirs(d, exist_ok=True)
        # tc sayaçları `replace`te SIFIRLANMAZ (birikimli) → hücre-içi FARK alınır (ölçüldü 2026-09-20).
        qf0, qr0 = json.loads(X.qdump("mb0"))[0], json.loads(X.qdump("ma0"))[0]
        rx = subprocess.Popen(["nsenter", "-t", str(X.T["CHLB"]), "-n", PROBE, "rx", "--bind", f"{X.B_IP}:{port}",
                               "--out", os.path.join(d, "rx.json"), "--idle-ms", str(max(3000, 6 * owd))])
        time.sleep(0.3)
        subprocess.run([PROBE, "tx", "--peer", f"{X.B_IP}:{port}", "--count", str(COUNT), "--pps", str(PPS),
                        "--size", str(SIZE), "--drain-ms", str(max(3000, 6 * owd)), "--out", os.path.join(d, "tx.json")])
        rx.wait(timeout=30)
        t, r = json.load(open(os.path.join(d, "tx.json"))), json.load(open(os.path.join(d, "rx.json")))
        qf, qr = json.loads(X.qdump("mb0"))[0], json.loads(X.qdump("ma0"))[0]
        fl = 1 - r["rx_unique"] / t["sent"]; rl = 1 - t["echoed"] / max(1, r["rx_unique"])
        flo, fhi = binom_interval(t["sent"], loss / 100); rlo, rhi = binom_interval(r["rx_unique"], loss / 100)
        fdrop, rdrop = t["sent"] - r["rx_unique"], r["rx_unique"] - t["echoed"]
        a_loss = int(flo <= fdrop <= fhi and rlo <= rdrop <= rhi)
        tgt = 2 * owd; p50 = t["rtt_p50_ns"] / 1e6
        a_rtt = int(abs(p50 - tgt) <= max(0.05 * tgt, 0.5))      # kontrol hücresinde (tgt=0) mutlak 0.5 ms
        rows.append([cell, loss, rc, owd, t["sent"], r["rx_unique"], t["echoed"], f"{fl:.4f}", f"{rl:.4f}",
                     f"{flo / t['sent']:.4f}", f"{fhi / t['sent']:.4f}", f"{rlo / max(1, r['rx_unique']):.4f}",
                     f"{rhi / max(1, r['rx_unique']):.4f}", f"{p50:.3f}", f"{t['rtt_p90_ns'] / 1e6:.3f}", tgt,
                     f"{(qf['drops'] - qf0['drops']) / max(1, (qf['drops'] - qf0['drops']) + (qf['packets'] - qf0['packets'])):.4f}",
                     f"{(qr['drops'] - qr0['drops']) / max(1, (qr['drops'] - qr0['drops']) + (qr['packets'] - qr0['packets'])):.4f}", vf[1],
                     open("/proc/loadavg").read().split()[0], a_loss, a_rtt, int(a_loss and a_rtt)])
        X.log(lg, f"CAL {rows[-1]}")
    with open(os.path.join(out, "calibration.csv"), "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n"); w.writerow(cols); w.writerows(rows)
    X.sh(["bash", os.path.join(X.HERE, "kvt_topo.sh"), "down", str(X.T["MID"]), str(X.T["CHLB"])])
    bad = [r[0] for r in rows if not r[-1]]
    print(f"calibration: {len(rows)} hücre, kabul={len(rows) - len(bad)}, RED={bad}")

if __name__ == "__main__":
    main()
