"""Verifier v-s4-b: wait between packet-threshold evidence (ACK covering seq+3 at client) and the
retransmission of a forward-lost transmission; what else was sent in that window.
python3 -I w.py <cell_dir> <arm>..."""
import gzip, json, pathlib, re, sys, bisect

def rows(path):
    out = []
    with gzip.open(path, "rt", errors="replace") as f:
        for line in f:
            p = line.rstrip("\n").split(",", 6)
            if len(p) < 7 or p[2] != "17": continue
            out.append((int(p[0]), p[1], int(p[4]), int(p[5]), p[6]))
    out.sort(key=lambda r: r[0]); return out

def parts(x):
    if ":" in x:
        h, t = x.split(":", 1); return bytes.fromhex(h), bytes.fromhex(t)
    b = bytes.fromhex(x); return b[:-16], b[-16:]

def ackset(fr):
    if len(fr) < 9 or fr[0] not in (0x10, 0x14): return None
    lg = int.from_bytes(fr[2:4], "big"); b0 = int.from_bytes(fr[6:8], "big"); n = fr[8]
    s = set()
    if b0 == 0: return s
    lo = (lg - b0 + 1) & 0xFFFF
    s.update(((lo + k) & 0xFFFF) for k in range(b0))
    cur = lo; off = 9
    for _ in range(n):
        if off + 4 > len(fr): break
        gap = int.from_bytes(fr[off:off+2], "big"); ln = int.from_bytes(fr[off+2:off+4], "big"); off += 4
        cur = (cur - gap) & 0xFFFF
        s.update(((cur - k) & 0xFFFF) for k in range(ln)); cur = (cur - ln) & 0xFFFF
    return s

def q(v, p):
    v = sorted(v)
    return float("nan") if not v else v[max(0, min(len(v)-1, int(round(p*(len(v)-1)))))]

def rep_events(rep):
    port = int(re.search(r"--bind\s+\S+:(\d+)", (rep/"cmd.txt").read_text()).group(1))
    cli = rows(rep/"capb_cli.csv.gz"); srv = rows(rep/"capb_srv.csv.gz")
    tx = []  # t, pn, seq_full, unit
    hi = None; base = 0
    for t, d, sp, dp, x in cli:
        if d != "o" or dp != port or not x: continue
        fr, tg = parts(x)
        if not fr or fr[0] not in (0, 5) or len(fr) < 11 or len(tg) < 8: continue
        s16 = int.from_bytes(fr[1:3], "big")
        if hi is not None and s16 < (hi & 0xFFFF) and (hi & 0xFFFF) - s16 > 30000: base += 65536
        s = base + s16; hi = s
        tx.append((t, int.from_bytes(tg[:8], "big"), s, int.from_bytes(fr[3:11], "big")))
    arrived = set()
    for t, d, sp, dp, x in srv:
        if d == "i" and dp == port and x:
            fr, tg = parts(x)
            if fr and fr[0] in (0, 5) and len(tg) >= 8: arrived.add(int.from_bytes(tg[:8], "big"))
    pn2seq = {pn & 0xFFFF: s for t, pn, s, u in tx}   # last-wins; PN16 wrap negligible (<65k pkts)
    ack_t, ack_max = [], []
    m = -1
    for t, d, sp, dp, x in cli:
        if d != "i" or sp != port or not x: continue
        fr, tg = parts(x); a = ackset(fr)
        if a is None: continue
        ss = [pn2seq[p] for p in a if p in pn2seq]
        if ss: m = max(m, max(ss))
        ack_t.append(t); ack_max.append(m)
    seen = set(); kind = []
    for t, pn, s, u in tx:
        kind.append("new" if u not in seen else "rep"); seen.add(u)
    tx_t = [r[0] for r in tx]
    byu = {}
    for i, r in enumerate(tx): byu.setdefault(r[3], []).append(i)
    ev = []
    for u, idx in byu.items():
        for a, b in zip(idx, idx[1:]):
            t, pn, s, _ = tx[a]
            if pn in arrived: continue
            tn = tx[b][0]
            # first ack at/after t with running max >= s+3
            j = bisect.bisect_left(ack_t, t); e3 = None
            while j < len(ack_t):
                if ack_max[j] >= s + 3: e3 = ack_t[j]; break
                j += 1
            r = {"d_rtx": (tn - t)/1e9, "e3": None if e3 is None else (e3 - t)/1e9}
            if e3 is not None and tn >= e3:
                lo = bisect.bisect_right(tx_t, e3); hi_ = bisect.bisect_left(tx_t, tn)
                r["wait"] = (tn - e3)/1e9
                r["new"] = sum(1 for k in range(lo, hi_) if kind[k] == "new")
                r["rep_other"] = sum(1 for k in range(lo, hi_) if kind[k] == "rep" and tx[k][3] != u)
            ev.append(r)
    return ev

for arm in sys.argv[2:]:
    cell = pathlib.Path(sys.argv[1]); E = []
    for rep in sorted((cell/arm).glob("rep*"), key=lambda p: int(p.name[3:])):
        try: E += rep_events(rep)
        except Exception as ex: print("#", rep, ex, file=sys.stderr)
    P = [e for e in E if "wait" in e]
    w = [e["wait"] for e in P]; nn = [e["new"] for e in P]; ro = [e["rep_other"] for e in P]
    print(f"{cell.name} {arm}: lost_followup={len(E)} post_ev3={len(P)} ({len(P)/max(1,len(E)):.3f}) "
          f"wait p50={q(w,.5):.2f} p95={q(w,.95):.2f} new p50={q(nn,.5)} p95={q(nn,.95)} frac_new>=1={sum(x>0 for x in nn)/max(1,len(nn)):.2f} "
          f"rep_other p50={q(ro,.5)} mean={sum(ro)/max(1,len(ro)):.2f} mean_new={sum(nn)/max(1,len(nn)):.2f} "
          f"d_rtx p50={q([e['d_rtx'] for e in E],.5):.2f} p95={q([e['d_rtx'] for e in E],.95):.2f} e3 p50={q([e['e3'] for e in E if e['e3'] is not None],.5):.2f}")
