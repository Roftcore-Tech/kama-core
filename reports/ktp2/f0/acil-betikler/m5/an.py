#!/usr/bin/env python3
# M5 derin inceleme: b_r12k_d1000_l30_p1000 ham koşu çözümlemesi (salt okuma).
import gzip, json, sys, statistics as st
from pathlib import Path

BASE = Path("/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/b_r12k_d1000_l30_p1000")


def pct(v, q):
    v = sorted(v)
    if not v:
        return None
    k = (len(v) - 1) * q
    f = int(k)
    c = min(f + 1, len(v) - 1)
    return v[f] + (v[c] - v[f]) * (k - f)


def read_out(rep):
    meta = None
    ev = []
    for line in open(rep / "out.jsonl"):
        o = json.loads(line)
        if "meta" in o:
            meta = o["meta"]
        else:
            ev.append(o["event"])
    return meta, ev


def read_rx(rep):
    rx = {}
    for line in open(rep / "rx_log.jsonl"):
        o = json.loads(line)
        if "u" in o:
            rx.setdefault(o["u"], o["t"])
    return rx


def read_cap(p):
    rows = []
    with gzip.open(p, "rt") as f:
        for line in f:
            if line.startswith("#"):
                continue
            a = line.rstrip("\n").split(",")
            rows.append((int(a[0]), a[1], int(a[2]), int(a[3]), int(a[4]), int(a[5]), a[6]))
    return rows


def ack16(fr):
    if len(fr) < 9:
        return set()
    largest = int.from_bytes(fr[2:4], "big")
    b0 = int.from_bytes(fr[6:8], "big")
    n = fr[8]
    s = set()
    if b0 == 0:
        return s
    cur = (largest - b0 + 1) & 0xFFFF
    for k in range(b0):
        s.add((cur + k) & 0xFFFF)
    off = 9
    for _ in range(n):
        if off + 4 > len(fr):
            break
        gap = int.from_bytes(fr[off:off + 2], "big")
        ln = int.from_bytes(fr[off + 2:off + 4], "big")
        off += 4
        cur = (cur - gap) & 0xFFFF
        for k in range(ln):
            s.add((cur - k) & 0xFFFF)
        cur = (cur - ln) & 0xFFFF
    return s


def kama_run(rep):
    meta, ev = read_out(rep)
    rx = read_rx(rep)
    t0 = meta["t0_unix_ns"]
    cap = read_cap(rep / "capb_cli.csv.gz")
    off = {e["msg_id"]: t0 + e["sent_ns"] for e in ev}
    conf = {e["msg_id"]: t0 + e["delivered_ns"] for e in ev if e.get("delivered_ns")}
    tx = []  # (t, unit, seq, l2)
    acks = []  # (t, set16)
    for t, d, pr, ln, sp, dp, x in cap:
        if pr != 17 or not x:
            continue
        if d == "o":
            if ":" in x:
                h, tl = x.split(":", 1)
                b = bytes.fromhex(h); tail = bytes.fromhex(tl)
            else:
                b = bytes.fromhex(x); tail = b[-16:]
            pn = int.from_bytes(tail[:8], "big") & 0xFFFF
            if b[0] == 0x00 and len(b) >= 11:
                tx.append((t, int.from_bytes(b[3:11], "big"), pn, ln))
            else:
                tx.append((t, None, pn, ln))
        else:
            if ":" in x:
                continue
            b = bytes.fromhex(x)[:-16]
            if b and b[0] in (0x10, 0x14):
                acks.append((t, ack16(b)))
    return meta, ev, rx, off, conf, tx, acks


def analyse(rep, verbose=False):
    meta, ev, rx, off, conf, tx, acks = kama_run(rep)
    t0 = meta["t0_unix_ns"]
    first = {}
    ntx = {}
    seq_t = {}
    seq_u = {}
    for t, u, s, ln in tx:
        if u is None:
            continue
        first.setdefault(u, t)
        ntx[u] = ntx.get(u, 0) + 1
        seq_t.setdefault(s, t)
        seq_u[s] = u
    # ACK sırası: her birimin ilk ACK'lendiği an (seq üzerinden)
    acked_seq_t = {}
    for t, s16 in acks:
        for s in s16:
            if s in seq_t and s not in acked_seq_t and t > seq_t[s]:
                acked_seq_t[s] = t
    # RTT örnekleri (ilk ACK'lenme − gönderim; ACK gecikmesi dahil)
    rtts = [(seq_t[s], (acked_seq_t[s] - seq_t[s]) / 1e9) for s in acked_seq_t]
    res = []
    for m in sorted(off):
        o = off[m]
        f = first.get(m)
        r = rx.get(m)
        c = conf.get(m)
        res.append(dict(m=m, off=(o - t0) / 1e9, wait=(f - o) / 1e9 if f else None,
                        fwd=(r - f) / 1e9 if (r and f) else None, rxlat=(r - o) / 1e9 if r else None,
                        conf=(c - o) / 1e9 if c else None, ntx=ntx.get(m, 0)))
    return meta, res, tx, acks, rtts, t0


def summary(arm, reps):
    rows = []
    for k in reps:
        rep = BASE / arm / f"rep{k}"
        meta, res, tx, acks, rtts, t0 = analyse(rep)
        w = [r["wait"] for r in res if r["wait"] is not None]
        fw = [r["fwd"] for r in res if r["fwd"] is not None]
        rl = [r["rxlat"] for r in res if r["rxlat"] is not None]
        cf = [r["conf"] for r in res if r["conf"] is not None]
        # emisyon hızı: ilk 5 s, tüm akış
        t_first = min(t for t, *_ in tx)
        data_tx = [(t, u, ln) for t, u, s, ln in tx if u is not None]
        t_last = max(t for t, *_ in data_tx)
        tot = sum(ln + 14 for t, u, ln in data_tx)
        dur = (t_last - t_first) / 1e9
        a = meta["activation"]
        # mesaj 99'un ilk iletimi ne zaman (sunum 28.2 s)
        last_first = max((r["off"] + r["wait"]) for r in res if r["wait"] is not None)
        rows.append(dict(rep=k, n=len(rl), wait_p50=pct(w, .5), wait_p95=pct(w, .95), wait_max=max(w),
                         fwd_p50=pct(fw, .5), fwd_p95=pct(fw, .95), rx_p50=pct(rl, .5), rx_p95=pct(rl, .95),
                         conf_p95=pct(cf, .95), last_first_tx=last_first, data_pkts=len(data_tx),
                         emit_Bps=tot / dur if dur else None, dur=dur,
                         btlbw=a["btlbw_bps"], srtt=a["srtt_us"] / 1e6, rtprop=a["rtprop_us"] / 1e6,
                         retx=a["retransmits"], trans=a["state_transitions"], rarq=a["recover_arq_entries"],
                         parity=a["parity_emitted"], tailp=a["tail_probes"]))
    return rows


if __name__ == "__main__":
    arm = sys.argv[1]
    reps = range(1, 31)
    rows = summary(arm, reps)
    keys = list(rows[0].keys())
    print("\t".join(keys))
    for r in rows:
        print("\t".join(f"{r[k]:.3f}" if isinstance(r[k], float) else str(r[k]) for k in keys))
    for k in ["wait_p50", "wait_p95", "fwd_p50", "fwd_p95", "rx_p50", "rx_p95", "conf_p95", "last_first_tx", "emit_Bps", "btlbw", "srtt", "rtprop", "retx", "rarq", "trans"]:
        v = [r[k] for r in rows if r[k] is not None]
        print(f"MED {k} = {st.median(v):.3f}  (min {min(v):.3f} max {max(v):.3f})")
