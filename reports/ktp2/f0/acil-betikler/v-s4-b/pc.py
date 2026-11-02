import gzip, json, pathlib, re, sys
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
cell = pathlib.Path(sys.argv[1]); arm = sys.argv[2]
agg = {"orig_pre": 0, "next_lt_1p2": 0, "orig_arrived_and_next_before_ack_possible": 0}
hist = {}
for rep in sorted((cell/arm).glob("rep*"), key=lambda p: int(p.name[3:])):
    meta = json.loads((rep/"out.jsonl").open().readline())["meta"]
    lpc = meta["activation"].get("left_preconverge_ns") or 10**15
    t0 = meta["t0_unix_ns"]; lpc_abs = t0 + lpc
    port = int(re.search(r"--bind\s+\S+:(\d+)", (rep/"cmd.txt").read_text()).group(1))
    cli = rows(rep/"capb_cli.csv.gz"); srv = rows(rep/"capb_srv.csv.gz")
    tx = []
    for t, d, sp, dp, x in cli:
        if d == "o" and dp == port and x:
            fr, tg = parts(x)
            if fr and fr[0] in (0, 5) and len(fr) >= 11: tx.append((t, int.from_bytes(tg[:8], "big"), int.from_bytes(fr[3:11], "big")))
    arr = {}
    for t, d, sp, dp, x in srv:
        if d == "i" and dp == port and x:
            fr, tg = parts(x)
            if fr and fr[0] in (0, 5): arr.setdefault(int.from_bytes(tg[:8], "big"), t)
    byu = {}
    for r in tx: byu.setdefault(r[2], []).append(r)
    for u, l in byu.items():
        if l[0][0] >= lpc_abs or len(l) < 2: 
            if l[0][0] < lpc_abs: agg["orig_pre"] += 1
            continue
        agg["orig_pre"] += 1
        dt = (l[1][0] - l[0][0]) / 1e9
        b = round(dt * 2) / 2
        hist[b] = hist.get(b, 0) + 1
        if dt < 1.2: agg["next_lt_1p2"] += 1
        if l[0][1] in arr and dt < 1.2: agg["orig_arrived_and_next_before_ack_possible"] += 1
print(cell.name, arm, agg)
print(" next-tx delay histogram (0.5s bins, first 12):", sorted(hist.items())[:12])
