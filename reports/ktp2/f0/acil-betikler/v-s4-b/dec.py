import gzip, json, pathlib, re, sys
exec(open("w.py").read().split("def q(")[0])
def q(v, p):
    v = sorted(v); return float("nan") if not v else v[max(0, min(len(v)-1, int(round(p*(len(v)-1)))))]
cell = pathlib.Path(sys.argv[1])
for arm in sys.argv[2:]:
    R = []; mism = 0
    for rep in sorted((cell/arm).glob("rep*"), key=lambda p: int(p.name[3:])):
        L = (rep/"out.jsonl").read_text().splitlines()
        meta = json.loads(L[0])["meta"]; t0 = meta["t0_unix_ns"]; off = meta.get("events_epoch_offset_ns", 0)
        ev = [json.loads(l)["event"] for l in L[1:] if '"event"' in l]
        port = int(re.search(r"--bind\s+\S+:(\d+)", (rep/"cmd.txt").read_text()).group(1))
        ftx = {}; pnu = {}
        for t, d, sp, dp, x in rows(rep/"capb_cli.csv.gz"):
            if d == "o" and dp == port and x:
                fr, tg = parts(x)
                if fr and fr[0] in (0, 5) and len(fr) >= 11:
                    u = int.from_bytes(fr[3:11], "big"); ftx.setdefault(u, t); pnu[int.from_bytes(tg[:8], "big")] = u
        fa = {}
        for t, d, sp, dp, x in rows(rep/"capb_srv.csv.gz"):
            if d == "i" and dp == port and x:
                fr, tg = parts(x)
                if fr and fr[0] in (0, 5):
                    pn = int.from_bytes(tg[:8], "big")
                    if pn in pnu: fa.setdefault(pnu[pn], t)
        for e in ev:
            u = e["msg_id"]
            if e.get("delivered_ns") is None or u not in ftx or u not in fa: continue
            sub = t0 + e["sent_ns"] - off; conf = t0 + e["delivered_ns"] - off
            if ftx[u] < sub - 5e6: mism += 1
            R.append(((conf - sub)/1e9, (ftx[u] - sub)/1e9, (fa[u] - ftx[u])/1e9, (conf - fa[u])/1e9, (fa[u]-sub)/1e9))
    tot = [r[0] for r in R]; p90 = q(tot, .9); top = [r for r in R if r[0] >= p90]
    m = lambda i: sum(r[i] for r in top)/len(top)
    print(f"{cell.name} {arm} n={len(R)} tx_before_submit={mism} queue p50={q([r[1] for r in R],.5):.2f} rec p50={q([r[2] for r in R],.5):.2f} fb p50={q([r[3] for r in R],.5):.2f} sub->arr p50={q([r[4] for r in R],.5):.2f} | top10% total={m(0):.1f} q={m(1):.1f} rec={m(2):.1f} fb={m(3):.1f}")
