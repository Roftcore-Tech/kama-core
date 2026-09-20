#!/usr/bin/env python3
"""Tek koşunun ham CSV satırını üretir (yalnız stdlib). HESAP yok — yalnız ölçülen + manifestten gelen.

kvt_row.py ROWDIR  → stdout: tek CSV satırı (başlık: --header)
ROWDIR içeriği (yürütücü yazar): row.json (manifest alanları + sayaç anlık görüntüleri), out.jsonl | abl.csv
"""
import csv, json, os, sys

COLS = ["run_id","replica","group","cell","loss_pct","rtt_class","owd_ms","rate","workload","msgs","payload",
        "arm","mode","rep","horizon_ms","deadline_ms","status","delivered","complete_horizon","complete_deadline",
        "t_complete_ms","wire_bytes","wire_tx_bytes","wire_rx_bytes","fwd_pkts","fwd_drops","rev_pkts","rev_drops",
        "netem_limit","netem_seed_fwd","netem_seed_rev","loadavg1","load_flag","warmup_residue","setup_ns",
        "connect_failed","profile_id","rc","seq_no","unix_ts"]
LOAD_FLAG = 2.0

def qd(snap):
    q = [x for x in snap if x.get("kind") == "netem"]
    return (q[0].get("packets", 0), q[0].get("drops", 0)) if q else (0, 0)

def link(snap):
    s = snap[0]["stats64"]; return s["tx"]["bytes"], s["rx"]["bytes"]

def main():
    if sys.argv[1] == "--header":
        print(",".join(COLS)); return
    d = sys.argv[1]; r = json.load(open(os.path.join(d, "row.json")))
    m = r["m"]; out = dict.fromkeys(COLS, "")
    for k in ("run_id","replica","group","cell","loss_pct","rtt_class","owd_ms","rate","workload","msgs","payload",
              "arm","mode","rep","deadline_ms"):
        out[k] = m[k]
    out["horizon_ms"] = int(m["horizon_s"]) * 1000
    n = int(m["msgs"]); status = "OK"; delivered = 0; t_complete = ""
    prof = ""; residue = ""; setup = ""; cfail = ""
    if m["arm"].startswith("abl-"):
        p = os.path.join(d, "abl.csv")
        rows = list(csv.DictReader(open(p))) if os.path.exists(p) else []
        if len(rows) != 1:
            status = "ERROR-NO-RESULT"
        else:
            a = rows[0]; done = a["complete"] == "1"
            delivered = n if done else ""      # sürücü kısmi teslim sayısı yazmaz
            t_complete = a["t_complete_ms"] if done else ""
            prof = "abl:dr_arqtail_product(bare-Sender)"
    else:
        p = os.path.join(d, "out.jsonl"); meta = None; ev = []
        if os.path.exists(p):
            for line in open(p):
                j = json.loads(line)
                if "meta" in j: meta = j["meta"]
                elif "event" in j: ev.append(j["event"])
        if meta is None or len(ev) != n:
            status = "ERROR-NO-RESULT"
        else:
            off = int(meta.get("events_epoch_offset_ns", 0) or 0)
            dl = [e["delivered_ns"] - off for e in ev if e.get("delivered_ns") is not None]
            delivered = len(dl)
            if delivered == n: t_complete = f"{max(dl) / 1e6:.3f}"
            prof = meta.get("profile_id", ""); residue = meta.get("warmup_residue", "")
            setup = meta.get("setup_ns", ""); cfail = int(bool(meta.get("connect_failed", False)))
            if m["arm"] == "kama-v1-full" and (prof != "kama-v1-full" or meta.get("profile_delta")):
                status = "PROFILE-MISMATCH"
    ch = int(status == "OK" and delivered == n and t_complete != "" and float(t_complete) <= out["horizon_ms"])
    cd = int(ch == 1 and float(t_complete) <= float(m["deadline_ms"]))
    fp0, fd0 = qd(r["q_fwd_before"]); fp1, fd1 = qd(r["q_fwd_after"])
    rp0, rd0 = qd(r["q_rev_before"]); rp1, rd1 = qd(r["q_rev_after"])
    tx0, rx0 = link(r["l_before"]); tx1, rx1 = link(r["l_after"])
    la = float(r["loadavg1"])
    out.update(status=status, delivered=delivered, complete_horizon=ch, complete_deadline=cd, t_complete_ms=t_complete,
               wire_tx_bytes=tx1 - tx0, wire_rx_bytes=rx1 - rx0, wire_bytes=(tx1 - tx0) + (rx1 - rx0),
               fwd_pkts=fp1 - fp0, fwd_drops=fd1 - fd0, rev_pkts=rp1 - rp0, rev_drops=rd1 - rd0,
               netem_limit=r["netem_limit"], netem_seed_fwd=m["seed_fwd"], netem_seed_rev=m["seed_rev"],
               loadavg1=f"{la:.2f}", load_flag=int(la > LOAD_FLAG), warmup_residue=residue, setup_ns=setup,
               connect_failed=cfail, profile_id=prof, rc=r["rc"], seq_no=r["seq_no"], unix_ts=r["unix_ts"])
    w = csv.writer(sys.stdout, lineterminator="\n"); w.writerow([out[c] for c in COLS])

main()
