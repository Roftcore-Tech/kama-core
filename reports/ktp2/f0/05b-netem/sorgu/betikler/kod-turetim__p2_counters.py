#!/usr/bin/env python3
"""[betimsel, sonradan] p2_loss80 ham sayaçları — kol × oturum. Salt okur."""
import json, pathlib, re, statistics, sys

D_NS = 20 * 10**9
RUNS = {"S1(a8c4e8c6)": "/home/ryuzaki/kama-k4g-work/k4g05b/netem7/run-a8c4e8c6",
        "S2(49864510)": "/home/ryuzaki/kama-k4g-work/k4g05b/netem7/run-49864510"}
PROF = sys.argv[1] if len(sys.argv) > 1 else "p2_loss80"

def kv(line, key):
    m = re.search(r'(?:^|[\s\[,(])' + re.escape(key) + r'=([^\s\]]+)', line)
    return m.group(1) if m else None

def num(s):
    if s is None: return None
    m = re.match(r'-?[0-9.]+', s)
    return float(m.group(0)) if m else None

def parse(rep):
    r = {}
    out = rep / "out.jsonl"
    ev = []; meta = None
    for line in open(out):
        j = json.loads(line)
        if "meta" in j: meta = j["meta"]
        if "event" in j: ev.append(j["event"])
    r["n_ev"] = len(ev)
    ok = [e for e in ev if e.get("delivered_ns") is not None and e["delivered_ns"] - e["sent_ns"] <= D_NS]
    r["m1"] = len(ok) / len(ev) if ev else None
    dl = [e for e in ev if e.get("delivered_ns") is not None]
    r["conf_total"] = len(dl)
    r["first_conf_s"] = min((e["delivered_ns"] for e in dl), default=None)
    r["first_conf_s"] = r["first_conf_s"] / 1e9 if r["first_conf_s"] is not None else None
    lat = sorted((e["delivered_ns"] - e["sent_ns"]) / 1e9 for e in dl)
    r["lat_med_s"] = statistics.median(lat) if lat else None
    man = json.loads((rep / "manifest.json").read_text())
    a = man.get("activation", {})
    for k in ("tail_probes", "tail_cancelled", "parity_emitted", "parity_token_denied", "retransmits", "dups",
              "state_transitions", "recover_arq_entries", "left_preconverge_ns", "btlbw_bps", "srtt_us",
              "return_ack_loss_permille", "return_forward_loss_permille", "bytes_data", "bytes_retx", "bytes_parity"):
        r[k] = a.get(k)
    r["setup_ns"] = man.get("setup_ns")
    r["left_preconv_s"] = r["left_preconverge_ns"] / 1e9 if r["left_preconverge_ns"] is not None else None
    c = (rep / "c.err").read_text().splitlines()
    for line in c:
        if line.startswith("KP-CLIENT"):
            r["cl_delivered"] = num(kv(line, "delivered"))
            r["tx_total"] = num(kv(line, "tx"))
            m = re.search(r'tx=\d+B\(data=(\d+) retx=(\d+) dup=(\d+) fec=(\d+) ctrl=(\d+)\)', line)
            if m:
                r["b_data"], r["b_retx"], r["b_dup"], r["b_fec"], r["b_ctrl"] = map(int, m.groups())
            r["rx_B"] = num(kv(line, "rx"))
            r["stale"] = num(kv(line, "stale"))
            r["establish_tx"] = num(kv(line, "establish_tx"))
            r["state_end"] = kv(line, "state")
            r["fec_rec_ctrl"] = num(kv(line, "fec_recovered_ctrl"))
            r["spurious"] = num(kv(line, "spurious"))
        if line.startswith("KP-ACTIVATED"):
            r["rtt_samples"] = num(kv(line, "rtt_samples"))
            r["data_denied"] = num(kv(line, "data_denied"))
            r["retx_denied"] = num(kv(line, "retx_denied"))
            r["ret_ack_loss"] = num(kv(line, "ret_ack_loss"))
            r["ack_int_ms"] = num(kv(line, "ack_int"))
            r["silence_ms"] = num(kv(line, "silence"))
        if line.startswith("KP-ACTIVATION"):
            r["recover_fwd"] = num(kv(line, "recover_fwd"))
            r["tail_probe_at_ms"] = None
            m = re.search(r'tail_probe=(\d+)@(\d+)ms', line)
            if m: r["tail_probe_at_ms"] = int(m.group(2))
            r["pace_Bps"] = num(kv(line, "pace"))
            r["est_btlbw"] = num(kv(line, "btlbw"))
            r["probe_up"] = num(kv(line, "probe_up"))
    s = (rep / "s.err").read_text()
    m = re.search(r'KP-SERVER delivered=(\d+)', s); r["srv_delivered"] = int(m.group(1)) if m else None
    for k in ("rx_datagrams", "tx_datagrams", "fec_recovered_units", "fec_repairs_rx", "fec_repairs_useful", "rx_bytes"):
        m = re.search(r'\b' + k + r': (\d+)', s); r["srv_" + k] = int(m.group(1)) if m else None
    r["rc"] = (rep / "rc").read_text().strip() if (rep / "rc").exists() else None
    return r

def main():
    rows = []
    for sess, root in RUNS.items():
        pd = pathlib.Path(root) / PROF
        for arm in ("taban", "aday"):
            for rep in sorted(pd.glob(f"{arm}/rep*"), key=lambda q: int(q.name[3:])):
                if not (rep / "out.jsonl").exists():
                    continue
                r = parse(rep); r["sess"] = sess; r["arm"] = arm; r["rep"] = int(rep.name[3:])
                rows.append(r)
    json.dump(rows, open(pathlib.Path(__file__).parent / f"{PROF}_rows.json", "w"), indent=0, default=str)
    keys = ["m1", "cl_delivered", "srv_delivered", "conf_total", "first_conf_s", "left_preconv_s", "setup_ns", "establish_tx",
            "tail_probes", "tail_probe_at_ms", "tail_cancelled", "retransmits", "dups", "stale", "b_retx", "b_dup", "b_fec",
            "parity_emitted", "parity_token_denied", "state_transitions", "recover_arq_entries", "recover_fwd", "rtt_samples",
            "btlbw_bps", "pace_Bps", "srtt_us", "tx_total", "srv_rx_datagrams", "srv_tx_datagrams", "srv_fec_recovered_units",
            "ret_ack_loss", "return_forward_loss_permille", "data_denied", "retx_denied", "spurious"]
    for k in keys:
        line = f"{k:<28}"
        for sess in RUNS:
            for arm in ("taban", "aday"):
                v = [r[k] for r in rows if r["sess"] == sess and r["arm"] == arm and isinstance(r.get(k), (int, float))]
                if v:
                    line += f" | {sess[:2]}-{arm:<5} med={statistics.median(v):>10.3f} [{min(v):.3g},{max(v):.3g}]"
                else:
                    line += f" | {sess[:2]}-{arm:<5} -"
        print(line)

if __name__ == "__main__":
    main()
