import gzip, json, pathlib, re, sys
sys.path.insert(0, "/nonexistent")
def rows(path):
    out = []
    with gzip.open(path, "rt", errors="replace") as f:
        for line in f:
            p = line.rstrip("\n").split(",", 6)
            if len(p) < 7 or p[2] != "17": continue
            out.append((int(p[0]), p[1], int(p[4]), int(p[5]), p[6]))
    return out
def parts(x):
    if ":" in x:
        h, t = x.split(":", 1); return bytes.fromhex(h), bytes.fromhex(t)
    b = bytes.fromhex(x); return b[:-16], b[-16:]
cell = pathlib.Path(sys.argv[1])
for arm in sys.argv[2:]:
    for rep in sorted((cell/arm).glob("rep*"), key=lambda p: int(p.name[3:])):
        port = int(re.search(r"--bind\s+\S+:(\d+)", (rep/"cmd.txt").read_text()).group(1))
        cli = rows(rep/"capb_cli.csv.gz"); srv = rows(rep/"capb_srv.csv.gz")
        txu = {}; arr_pn = set()
        for t, d, sp, dp, x in cli:
            if d == "o" and dp == port and x:
                fr, tg = parts(x)
                if fr and fr[0] in (0, 5) and len(fr) >= 11: txu.setdefault(int.from_bytes(fr[3:11], "big"), []).append(int.from_bytes(tg[:8], "big"))
        srvu = set()
        for t, d, sp, dp, x in srv:
            if d == "i" and dp == port and x:
                fr, tg = parts(x)
                if fr and fr[0] in (0, 5) and len(fr) >= 11: srvu.add(int.from_bytes(fr[3:11], "big"))
        rx = set()
        for l in (rep/"rx_log.jsonl").read_text().splitlines()[1:]:
            try: rx.add(json.loads(l)["u"])
            except Exception: pass
        miss = [u for u in txu if u not in srvu]
        print(rep.name, "units_tx", len(txu), "units_arrived_cap", len(srvu), "rxlog", len(rx), "tx_not_cap_arrived", len(miss), "in_rxlog", sum(1 for u in miss if u in rx))
