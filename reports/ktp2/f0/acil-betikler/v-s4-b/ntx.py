"""v-s4-b: per unit, transmissions until first capture-arrival at server and first-arrival latency
from first wire tx. Also hold3/rcv3 for forward-lost transmissions (engine seq)."""
import gzip, pathlib, re, sys, bisect
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
    lo = (lg - b0 + 1) & 0xFFFF; s.update(((lo + k) & 0xFFFF) for k in range(b0)); cur = lo; off = 9
    for _ in range(n):
        if off + 4 > len(fr): break
        gap = int.from_bytes(fr[off:off+2], "big"); ln = int.from_bytes(fr[off+2:off+4], "big"); off += 4
        cur = (cur - gap) & 0xFFFF; s.update(((cur - k) & 0xFFFF) for k in range(ln)); cur = (cur - ln) & 0xFFFF
    return s
def q(v, p):
    v = sorted(v); return float("nan") if not v else v[max(0, min(len(v)-1, int(round(p*(len(v)-1)))))]
def rep_an(rep):
    port = int(re.search(r"--bind\s+\S+:(\d+)", (rep/"cmd.txt").read_text()).group(1))
    cli = rows(rep/"capb_cli.csv.gz"); srv = rows(rep/"capb_srv.csv.gz")
    tx = []; hi = None; base = 0
    for t, d, sp, dp, x in cli:
        if d != "o" or dp != port or not x: continue
        fr, tg = parts(x)
        if not fr or fr[0] not in (0, 5) or len(fr) < 11 or len(tg) < 8: continue
        s16 = int.from_bytes(fr[1:3], "big")
        if hi is not None and s16 < (hi & 0xFFFF) and (hi & 0xFFFF) - s16 > 30000: base += 65536
        s = base + s16; hi = s
        tx.append((t, int.from_bytes(tg[:8], "big"), s, int.from_bytes(fr[3:11], "big")))
    arr = {}
    for t, d, sp, dp, x in srv:
        if d == "i" and dp == port and x:
            fr, tg = parts(x)
            if fr and fr[0] in (0, 5) and len(tg) >= 8: arr.setdefault(int.from_bytes(tg[:8], "big"), t)
    pn2seq = {pn & 0xFFFF: s for t, pn, s, u in tx}
    seq_arr = sorted((s, arr[pn]) for t, pn, s, u in tx if pn in arr)  # seq -> arrival
    # server ACK emissions: running max seq
    bt, bm = [], []; m = -1
    for t, d, sp, dp, x in srv:
        if d != "o" or sp != port or not x: continue
        fr, tg = parts(x); a = ackset(fr)
        if a is None: continue
        ss = [pn2seq[p] for p in a if p in pn2seq]
        if ss: m = max(m, max(ss))
        bt.append(t); bm.append(m)
    byu = {}
    for r in tx: byu.setdefault(r[3], []).append(r)
    msgs = []; hold = []; rcv3 = []
    seqs_sorted = [x[0] for x in seq_arr]
    for u, lst in byu.items():
        n = None
        for i, r in enumerate(lst):
            if r[1] in arr: n = i + 1; msgs.append((n, (arr[r[1]] - lst[0][0])/1e9)); break
        for r in lst[:-1]:
            if r[1] in arr: continue
            s = r[2]
            k = bisect.bisect_left(seqs_sorted, s + 3)
            if k >= len(seq_arr): continue
            s3, ta = seq_arr[k]   # first arrived seq >= s+3 (lowest seq), its arrival
            # earliest arrival among arrived seq>=s+3
            ta = min(x[1] for x in seq_arr[k:k+50])
            rcv3.append((ta - r[0])/1e9)
            j = bisect.bisect_left(bt, ta)
            while j < len(bt) and bm[j] < s + 3: j += 1
            if j < len(bt): hold.append((bt[j] - ta)/1e9)
    return msgs, hold, rcv3
cell = pathlib.Path(sys.argv[1])
for arm in sys.argv[2:]:
    M, H, R = [], [], []
    for rep in sorted((cell/arm).glob("rep*"), key=lambda p: int(p.name[3:])):
        try:
            a, b, c = rep_an(rep); M += a; H += b; R += c
        except Exception as ex: print("#", rep, ex, file=sys.stderr)
    out = []
    for k in (1, 2, 3):
        v = [l for n, l in M if n == k]; out.append(f"ntx{k} n={len(v)} p50={q(v,.5):.2f}")
    v1 = q([l for n, l in M if n == 1], .5); v2 = q([l for n, l in M if n == 2], .5)
    print(f"{cell.name} {arm}: " + " | ".join(out) + f" | cost1={v2-v1:.2f} | rcv3 p50={q(R,.5):.2f} hold p50={q(H,.5):.3f} p95={q(H,.95):.2f}")
