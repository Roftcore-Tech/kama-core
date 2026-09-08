#!/usr/bin/env python3
"""[FAZ-5] Tek hücre raporu — donmuş `bench metrics` + varsayımsız dağılım ifadeleri."""
import json, glob, statistics, subprocess, sys, pathlib
ROOT = "/home/ryuzaki/kama-phase5-results/stageB"
BENCH = "/home/ryuzaki/Desktop/kama-core/target/release/bench"
ARMS = [("kama-product", "kama-v1-full"), ("quinn", "quinn-reliable-stream"), ("tcp", "tcp-default")]

def comp(f, dl_ns):
    ev = [json.loads(l)["event"] for l in open(f) if '"event"' in l]
    if not ev: return 0.0
    ok = sum(1 for e in ev if e.get("delivered_ns") is not None
             and e["delivered_ns"] - e["sent_ns"] <= dl_ns)
    return ok / len(ev)

def main():
    cell = sys.argv[1]; D = int(sys.argv[2]) if len(sys.argv) > 2 else 20
    cd = pathlib.Path(ROOT) / cell
    cfg = json.loads((cd / "cell.json").read_text())
    print(f"######## {cell}  fwd='{cfg['fwd']}'  rev='{cfg['rev']}'  "
          f"geo={cfg['msgs']}x{cfg['payload']}B@{cfg['submit_interval_ms']}ms  D={D}s ########")
    dist = {}
    for arm, label in ARMS:
        runs = sorted(glob.glob(str(cd / arm / "rep*/out.jsonl")))
        if not runs: continue
        out = subprocess.run([BENCH, "metrics", "--runs", *runs, "--deadlines-s", f"5,10,{D}"],
                             capture_output=True, text=True).stdout
        print(f"--- {label} ---")
        for ln in out.splitlines():
            if ln.startswith(("deadline=", "# outcomes", "# blackout", "# all-runs",
                              "# stall-free", "# stall inc", "# claim")):
                print("  " + ln)
        dist[arm] = sorted(comp(f, D * 10**9) for f in runs)
    print("\n=== VARSAYIMSIZ DAĞILIM ===")
    for arm, label in ARMS:
        v = dist.get(arm)
        if not v: continue
        print(f"  {label:<24} min={v[0]:.3f} med={statistics.median(v):.3f} max={v[-1]:.3f} "
              f"| >=0.99: {sum(1 for x in v if x>=0.99)}/{len(v)} | <0.50: {sum(1 for x in v if x<0.50)}/{len(v)}")
    k = dist.get("kama-product")
    if k:
        for arm, label in ARMS[1:]:
            o = dist.get(arm)
            if not o: continue
            w = sum(1 for x in k for y in o if x > y) + 0.5*sum(1 for x in k for y in o if x == y)
            print(f"  P(kama > {label}) = {w/(len(k)*len(o)):.3f}   "
                  f"[kama_min={min(k):.3f} vs {label}_med={statistics.median(o):.3f}]")
    fs = sorted(glob.glob(str(cd / "kama-product" / "rep*/manifest.json")))
    if fs:
        a = [json.load(open(f))["activation"] for f in fs]
        med = lambda x: statistics.median(m[x] for m in a)
        print("\n=== KAMA AKTİVASYON (medyan) ===")
        print(f"  parity={med('parity_emitted')} denied={med('parity_token_denied')} "
              f"retx={med('retransmits')} dup={med('dups')} probes={med('tail_probes')} "
              f"recover_arq={med('recover_arq_entries')}")
        print(f"  tel: data={med('bytes_data')}B retx={med('bytes_retx')}B parity={med('bytes_parity')}B")
        print(f"  srtt={med('srtt_us')/1000:.1f}ms btlbw={med('btlbw_bps')}B/s "
              f"fwd_loss={med('return_forward_loss_permille')}‰ ret_ack_loss={med('return_ack_loss_permille')}‰")
        ident = {json.load(open(f))['arm'] for f in fs}
        delta = sum(1 for f in fs if json.load(open(f))['profile_delta'])
        print(f"  kimlik={sorted(ident)} sapmalı_koşu={delta}")
    for arm, label in ARMS[1:]:
        v = [json.loads(open(f).readline())["meta"]["udp_tx_bytes"]
             for f in sorted(glob.glob(str(cd / arm / "rep*/out.jsonl")))]
        if v: print(f"  {label} tx medyan={int(statistics.median(v))}B")

main()
