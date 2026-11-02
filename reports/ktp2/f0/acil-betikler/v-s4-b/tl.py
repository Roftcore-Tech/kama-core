import gzip, json, pathlib, re, sys
exec(open("w.py").read().split("def q(")[0])
rep = pathlib.Path(sys.argv[1]); lim = float(sys.argv[2])
meta = json.loads((rep/"out.jsonl").open().readline())["meta"]; t0 = meta["t0_unix_ns"]
print("left_preconv", meta["activation"]["left_preconverge_ns"]/1e9)
port = int(re.search(r"--bind\s+\S+:(\d+)", (rep/"cmd.txt").read_text()).group(1))
cli = rows(rep/"capb_cli.csv.gz"); srv = rows(rep/"capb_srv.csv.gz")
ev = []
pn2 = {}
for t, d, sp, dp, x in cli:
    if not x: continue
    fr, tg = parts(x)
    if d == "o" and dp == port and fr and fr[0] in (0, 5) and len(fr) >= 11:
        pn = int.from_bytes(tg[:8], "big"); s = int.from_bytes(fr[1:3], "big"); u = int.from_bytes(fr[3:11], "big")
        pn2[pn & 0xFFFF] = (s, u); ev.append(((t - t0)/1e9, f"C-TX seq={s} u={u} pn={pn}"))
    elif d == "i" and sp == port:
        a = ackset(fr)
        if a is not None: ev.append(((t - t0)/1e9, f"C-ACK seqs={sorted(pn2[p][0] for p in a if p in pn2)}"))
for t, d, sp, dp, x in srv:
    if not x: continue
    fr, tg = parts(x)
    if d == "i" and dp == port and fr and fr[0] in (0, 5):
        ev.append(((t - t0)/1e9, f"   S-RX pn={int.from_bytes(tg[:8],'big')}"))
for t, s in sorted(ev):
    if t > lim: break
    print(f"{t:8.3f} {s}")
