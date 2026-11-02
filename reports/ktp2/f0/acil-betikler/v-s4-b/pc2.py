import gzip, json, pathlib, re, sys
exec(open("pc.py").read().split("cell = pathlib.Path")[0])
cell = pathlib.Path(sys.argv[1]); arm = sys.argv[2]
offs = {}; n_units = 0; n_with_1s = 0
for rep in sorted((cell/arm).glob("rep*"), key=lambda p: int(p.name[3:])):
    meta = json.loads((rep/"out.jsonl").open().readline())["meta"]
    lpc = meta["activation"].get("left_preconverge_ns") or 10**15
    lpc_abs = meta["t0_unix_ns"] + lpc
    port = int(re.search(r"--bind\s+\S+:(\d+)", (rep/"cmd.txt").read_text()).group(1))
    tx = []
    for t, d, sp, dp, x in rows(rep/"capb_cli.csv.gz"):
        if d == "o" and dp == port and x:
            fr, tg = parts(x)
            if fr and fr[0] in (0, 5) and len(fr) >= 11: tx.append((t, int.from_bytes(fr[3:11], "big")))
    byu = {}
    for t, u in tx: byu.setdefault(u, []).append(t)
    for u, l in byu.items():
        if l[0] >= lpc_abs: continue
        n_units += 1
        o = [round((t - l[0]) / 1e9, 2) for t in l[1:4]]
        if any(0.9 <= x <= 1.2 for x in o): n_with_1s += 1
        if n_units <= 12: print(rep.name, u, o)
print("units sent in PreConverge:", n_units, "with a re-tx at 0.9-1.2s:", n_with_1s)
