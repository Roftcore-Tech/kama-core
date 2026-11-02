#!/usr/bin/env python3
"""S4 read-only: per-lost-transmission retransmit timeline from F0 lane-B captures (Kama arms).

For every Kama DATA transmission that did not arrive at the server (forward loss) and whose
unit was transmitted again, compute:
  d_rtx   = t(next transmission of same unit) - t(send)
  d_ev1   = t(first ACK at client acking any later tracked PN) - t(send)       (time-thr eligibility)
  d_ev3   = t(first ACK at client whose largest tracked rank >= rank+3) - t(send)  (pkt-thr K=3)
  d_rcv3  = t(arrival at server of tracked rank+3 .. first arrived rank>=rank+3) - t(send)
  hold3   = t(first server ACK emission covering rank>=rank+3) - t(arrival at server of that packet)
  sched   = d_rtx - d_ev3 (if d_rtx >= d_ev3)
Classification of the trigger window: rtx before any later-ACK (pto/dup), between ev1 and ev3
(time-threshold/PTO), after ev3 (packet threshold available).
Also per message: number of transmissions until first arrival and first-arrival latency.
Usage: python3 -I retx_timeline.py <cell_dir> <arm> [<arm>...]
"""
import gzip
import json
import pathlib
import statistics
import sys

FT = {0x00: "data", 0x05: "data_frag", 0x01: "fec", 0x02: "recovered", 0x03: "param",
      0x04: "param_ack", 0x06: "cs_probe", 0x07: "cs_echo", 0x10: "ack", 0x14: "ack"}
TRACKED = ("data", "data_frag", "fec")


def read_cap(path):
    rows = []
    with gzip.open(path, "rt", errors="replace") as f:
        for line in f:
            if not line or line[0] == "#":
                continue
            p = line.rstrip("\n").split(",", 6)
            if len(p) < 7:
                continue
            try:
                rows.append((int(p[0]), p[1], int(p[2]), int(p[3]), int(p[4]), int(p[5]), p[6]))
            except ValueError:
                continue
    rows.sort(key=lambda r: r[0])
    return rows


def ack_decode(fr):
    if len(fr) < 9:
        return set()
    v2 = fr[0] == 0x14
    largest = int.from_bytes(fr[2:4], "big")
    b0 = int.from_bytes(fr[6:8], "big")
    n = fr[8]
    s = set()
    if b0 == 0:
        return s
    cursor = (largest - b0 + 1) & 0xFFFF
    for k in range(b0):
        s.add((cursor + k) & 0xFFFF)
    off = 9
    for _ in range(n):
        if off + 4 > len(fr):
            break
        gap = int.from_bytes(fr[off:off + 2], "big")
        ln = int.from_bytes(fr[off + 2:off + 4], "big")
        off += 4
        cursor = (cursor - gap) & 0xFFFF
        if ln:
            for k in range(ln):
                s.add((cursor - k) & 0xFFFF)
            cursor = (cursor - ln) & 0xFFFF
    _ = v2
    return s


def decode(x):
    if ":" in x:
        h, t = x.split(":", 1)
        head, tail = bytes.fromhex(h), bytes.fromhex(t)
        frame = head
        full = False
    else:
        b = bytes.fromhex(x)
        if len(b) < 17:
            return None
        head, tail, frame = b, b[-16:], b[:-16]
        full = True
    if len(tail) < 16 or not frame:
        return None
    pn = int.from_bytes(tail[:8], "big")
    ft = FT.get(frame[0], "other")
    d = {"pn": pn, "type": ft}
    if ft in ("data", "data_frag") and len(frame) >= 11:
        d["unit"] = int.from_bytes(frame[3:11], "big")
    if ft == "ack" and full:
        d["acked16"] = ack_decode(frame)
    return d


def port_of(rep):
    import re
    m = re.search(r"--bind\s+\S+:(\d+)", (rep / "cmd.txt").read_text())
    return int(m.group(1)) if m else None


def udp(rows, direction, to_server, port):
    out = []
    for t, dr, pr, ipl, sp, dp, x in rows:
        if pr != 17 or dr != direction or not x:
            continue
        if port is not None and (dp if to_server else sp) != port:
            continue
        d = decode(x)
        if d is None:
            continue
        d["t"] = t / 1e9
        out.append(d)
    return out


def analyze(rep):
    cli = read_cap(rep / "capb_cli.csv.gz")
    srv = read_cap(rep / "capb_srv.csv.gz")
    port = port_of(rep)
    a_out = udp(cli, "o", True, port)
    b_in = udp(srv, "i", True, port)
    b_out = udp(srv, "o", False, port)
    a_in = udp(cli, "i", False, port)
    trk = [p for p in a_out if p["type"] in TRACKED]
    rank = {p["pn"]: i for i, p in enumerate(trk)}
    low2full = {p["pn"] & 0xFFFF: p["pn"] for p in trk}
    arr = {}
    for p in b_in:
        arr.setdefault(p["pn"], p["t"])
    # ACK arrivals at client: (t, max_rank_acked, set_of_ranks)
    acks_a = []
    for p in a_in:
        if p["type"] != "ack" or "acked16" not in p:
            continue
        rs = [rank[low2full[lo]] for lo in p["acked16"] if lo in low2full and low2full[lo] in rank]
        if rs:
            acks_a.append((p["t"], max(rs)))
    acks_b = []
    for p in b_out:
        if p["type"] != "ack" or "acked16" not in p:
            continue
        rs = [rank[low2full[lo]] for lo in p["acked16"] if lo in low2full and low2full[lo] in rank]
        if rs:
            acks_b.append((p["t"], max(rs)))
    # running max at client
    run_a = []
    m = -1
    for t, r in acks_a:
        m = max(m, r)
        run_a.append((t, m))
    run_b = []
    m = -1
    for t, r in acks_b:
        m = max(m, r)
        run_b.append((t, m))

    def first_after(run, t0, need_rank):
        for t, r in run:
            if t >= t0 and r >= need_rank:
                return t
        return None

    # unit transmissions
    tx = {}
    for p in a_out:
        if p["type"] in ("data", "data_frag") and "unit" in p:
            tx.setdefault(p["unit"], []).append(p)
    recs = []
    msgs = []
    first_pn = {}
    for p in a_out:
        if p["type"] in ("data", "data_frag") and "unit" in p:
            first_pn.setdefault(p["unit"], p["pn"])
    for u, lst in tx.items():
        lst.sort(key=lambda p: p["t"])
        # message-level: transmissions until first arrival
        first_arr = None
        n_until = None
        for i, p in enumerate(lst):
            if p["pn"] in arr:
                first_arr = arr[p["pn"]]
                n_until = i + 1
                break
        msgs.append((u, lst[0]["t"], first_arr, n_until, len(lst)))
        for i, p in enumerate(lst[:-1]):
            if p["pn"] in arr:
                continue  # arrived; later copy is not a loss recovery for this tx
            t0 = p["t"]
            r0 = rank.get(p["pn"])
            if r0 is None:
                continue
            nxt = lst[i + 1]["t"]
            ev1 = first_after(run_a, t0, r0 + 1)
            ev3 = first_after(run_a, t0, r0 + 3)
            # earliest arrival at server of any tracked packet with rank >= r0+3
            rcv3 = None
            for q in trk[r0 + 3:]:
                if q["pn"] in arr:
                    rcv3 = arr[q["pn"]]
                    break
            b3 = first_after(run_b, t0, r0 + 3)
            n_new = n_any = 0
            if ev3 is not None and nxt >= ev3:
                for q2 in a_out:
                    if ev3 < q2["t"] < nxt and q2["type"] in ("data", "data_frag"):
                        n_any += 1
                        if q2["pn"] == first_pn.get(q2.get("unit")):
                            n_new += 1
            recs.append({
                "unit": u, "i": i, "t0": t0, "d_rtx": nxt - t0,
                "d_ev1": None if ev1 is None else ev1 - t0,
                "d_ev3": None if ev3 is None else ev3 - t0,
                "d_rcv3": None if rcv3 is None else rcv3 - t0,
                "hold3": None if (b3 is None or rcv3 is None) else b3 - rcv3,
                "cls": ("pre_ev1" if (ev1 is None or nxt < ev1) else
                        ("ev1_ev3" if (ev3 is None or nxt < ev3) else "post_ev3")),
                "sched": None if (ev3 is None or nxt < ev3) else nxt - ev3,
                "n_new": n_new, "n_any": n_any,
            })
    # one-way fwd delay of originals that arrived (reference)
    owd = [arr[p["pn"]] - p["t"] for p in trk if p["pn"] in arr]
    return recs, msgs, owd


def q(v, p):
    v = sorted(x for x in v if x is not None)
    if not v:
        return float("nan")
    k = max(0, min(len(v) - 1, int(round(p * (len(v) - 1)))))
    return v[k]


def main():
    cell = pathlib.Path(sys.argv[1])
    for arm in sys.argv[2:]:
        allr, allm, allo = [], [], []
        reps = sorted((cell / arm).glob("rep*"), key=lambda p: int(p.name[3:]))
        for rep in reps:
            if not (rep / "capb_cli.csv.gz").exists() or not (rep / "capb_srv.csv.gz").exists():
                continue
            try:
                r, m, o = analyze(rep)
            except Exception as e:  # noqa: BLE001
                print(f"# {rep}: {e}", file=sys.stderr)
                continue
            allr += r
            allm += m
            allo += o
        n = len(allr)
        print(f"== {cell.name} / {arm}: reps={len(reps)} lost_tx_with_followup={n}")
        if not n:
            continue
        cls = {}
        for r in allr:
            cls[r["cls"]] = cls.get(r["cls"], 0) + 1
        print("  trigger window:", {k: f"{v} ({v / n:.2f})" for k, v in sorted(cls.items())})
        for key in ("d_rtx", "d_ev1", "d_ev3", "d_rcv3", "hold3", "sched"):
            vals = [r[key] for r in allr]
            vv = [x for x in vals if x is not None]
            print(f"  {key:7s} n={len(vv):4d} p25={q(vv, .25):7.3f} p50={q(vv, .5):7.3f} "
                  f"p75={q(vv, .75):7.3f} p95={q(vv, .95):7.3f}")
        pe = [r for r in allr if r["cls"] == "post_ev3"]
        if pe:
            nn = [r["n_new"] for r in pe]
            na = [r["n_any"] for r in pe]
            print(f"  post_ev3: data pkts sent in (ev3, rtx): new-original p50={q(nn,.5)} p95={q(nn,.95)} "
                  f"mean={sum(nn)/len(nn):.2f}; any-data p50={q(na,.5)} mean={sum(na)/len(na):.2f}; "
                  f"frac with >=1 new original = {sum(1 for x in nn if x>0)/len(nn):.2f}")
        for c in ("pre_ev1", "ev1_ev3", "post_ev3"):
            vv = [r["d_rtx"] for r in allr if r["cls"] == c]
            if vv:
                print(f"  d_rtx|{c:8s} n={len(vv):4d} p50={q(vv, .5):7.3f} p95={q(vv, .95):7.3f}")
        # retransmit round index
        for i in (0, 1, 2):
            vv = [r["d_rtx"] for r in allr if r["i"] == i]
            if vv:
                print(f"  d_rtx|round{i} n={len(vv):4d} p50={q(vv, .5):7.3f} p95={q(vv, .95):7.3f}")
        print(f"  owd_fwd (arrived tracked) p50={q(allo, .5):.3f} p95={q(allo, .95):.3f}")
        lat = [m[2] - m[1] for m in allm if m[2] is not None]
        ntx = [m[3] for m in allm if m[3] is not None]
        print(f"  msg first-arrival latency p50={q(lat, .5):.3f} p95={q(lat, .95):.3f} "
              f"n={len(lat)}; tx-until-arrival dist:",
              {k: ntx.count(k) for k in sorted(set(ntx))})
        # latency conditional on tx count
        for k in sorted(set(ntx)):
            vv = [m[2] - m[1] for m in allm if m[3] == k and m[2] is not None]
            print(f"    ntx={k}: n={len(vv)} lat p50={q(vv, .5):.3f} p95={q(vv, .95):.3f}")


if __name__ == "__main__":
    main()
