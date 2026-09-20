#!/usr/bin/env python3
"""FAZ 0.5 — TCP SAĞLAMA-1 (fizik: Mathis) + SAĞLAMA-2 (araç: nc). `unshare -rnm` içinde koşar.

Sağlama-1: ~2 MB (2000×995 B) tcp-std aktarımı, kayıp {1,2,5,10}% (simetrik), RTT low(50)/med(250).
  ölçülen goodput = uygulama baytı / t_complete ;  Mathis ≈ 1.22·MSS/(RTT·√p), MSS=1448.
  oran = ölçülen / Mathis ; 0.3–3 dışı → DUR (offload, kuyruk limiti, rate araştır).
Sağlama-2: aynı bayt sayısı `nc` (OpenBSD netcat) ile 3 hücrede; adapter süresiyle oranlanır.
Kullanım: kvt_tcp_sanity.py OUTDIR
"""
import csv, json, math, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kvt_exec as X
import kvt_manifest as M

MSGS, PAY, MSS = 2000, 995, 1448
CELLS = [(1, "low", 25, 3), (2, "low", 25, 3), (5, "low", 25, 3), (10, "low", 25, 3), (1, "med", 125, 2), (5, "med", 125, 2)]
NC_CELLS = {(2, "low"), (10, "low"), (5, "med")}
# Araştırma yeniden-koşusu için (bant-dışı satırın nedeni): KVT_SAN_MSGS=6000 KVT_SAN_CELLS="1:med:125:2"
if os.environ.get("KVT_SAN_MSGS"): MSGS = int(os.environ["KVT_SAN_MSGS"])
if os.environ.get("KVT_SAN_CELLS"):
    CELLS = [(int(a), b, int(c), int(d)) for a, b, c, d in (x.split(":") for x in os.environ["KVT_SAN_CELLS"].split(","))]
    NC_CELLS = set()

def main():
    out = sys.argv[1]; os.makedirs(out, exist_ok=True); lg = open(os.path.join(out, "sanity.log"), "a")
    if not X.topo_up(lg): sys.exit(3)
    rows, port = [], 33000
    for loss, rc, owd, reps in CELLS:
        if not X.gate(owd, lg): X.topo_up(lg); X.gate(owd, lg)
        ne = f"delay {owd}ms loss {loss}%"; rtt = 2 * owd / 1000
        mathis = 1.22 * MSS / (rtt * math.sqrt(loss / 100))
        for rep in range(reps):
            for tool in ["adapter"] + (["nc"] if (loss, rc) in NC_CELLS else []):
                sf, sr = M.seed("KVT1-SAN", loss, rc, rep, tool, "fwd"), M.seed("KVT1-SAN", loss, rc, rep, tool, "rev")
                assert X.set_netem(f"{ne} seed {sf}", f"{ne} seed {sr}")
                assert X.verify("mb0", ne, sf)[0] and X.verify("ma0", ne, sr)[0], "netem doğrulanamadı"
                port += 3; d = os.path.join(out, f"iid{loss}-{rc}-r{rep}-{tool}"); os.makedirs(d, exist_ok=True)
                q0 = json.loads(X.qdump("mb0"))[0]
                if tool == "adapter":
                    m = {"arm": "tcp-std", "msgs": str(MSGS), "payload": str(PAY), "horizon_s": "180", "warmup_msgs": "8",
                         "mode": "warm", "run_id": os.path.basename(d)}
                    X.run_row(m, d, port, lg)
                    ev = [json.loads(l)["event"] for l in open(os.path.join(d, "out.jsonl")) if '"event"' in l]
                    dl = [e["delivered_ns"] for e in ev if e["delivered_ns"] is not None]
                    secs = max(dl) / 1e9 if len(dl) == MSGS else float("nan")
                    nbytes = MSGS * (PAY + 12)
                else:
                    nbytes = MSGS * (PAY + 12)
                    srv = subprocess.Popen(["nsenter", "-t", str(X.T["CHLB"]), "-n", "nc", "-l", "-p", str(port)],
                                           stdout=subprocess.DEVNULL)
                    time.sleep(0.4); t = time.monotonic()
                    subprocess.run(f"head -c {nbytes} /dev/zero | nc -N {X.B_IP} {port}", shell=True, timeout=400)
                    secs = time.monotonic() - t; srv.wait(timeout=10)
                q1 = json.loads(X.qdump("mb0"))[0]; dr, pk = q1["drops"] - q0["drops"], q1["packets"] - q0["packets"]
                gp = nbytes / secs
                rows.append([loss, rc, rep, tool, nbytes, f"{secs:.3f}", f"{gp:.0f}", f"{mathis:.0f}", f"{gp / mathis:.3f}",
                             pk, dr, f"{dr / max(1, dr + pk):.4f}", open("/proc/loadavg").read().split()[0]])
                X.log(lg, f"SANITY {rows[-1]}")
    with open(os.path.join(out, "tcp_sanity.csv"), "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["loss_pct","rtt_class","rep","tool","bytes","secs","goodput_Bps","mathis_Bps","ratio","fwd_pkts","fwd_drops","fwd_loss_realized","loadavg1"])
        w.writerows(rows)
    X.sh(["bash", os.path.join(X.HERE, "kvt_topo.sh"), "down", str(X.T["MID"]), str(X.T["CHLB"])])
    bad = [r for r in rows if r[3] == "adapter" and not (0.3 <= float(r[8]) <= 3.0)]
    print(f"tcp_sanity: {len(rows)} satır; Mathis bandı (0.3–3) DIŞI adapter satırı = {len(bad)}")

if __name__ == "__main__":
    main()
