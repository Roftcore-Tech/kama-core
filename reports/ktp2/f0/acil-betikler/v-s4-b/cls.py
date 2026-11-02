import sys, pathlib
src = open("w.py").read()
src = src.split("for arm in sys.argv[2:]:")[0]
src = src.replace('r = {"d_rtx": (tn - t)/1e9, "e3": None if e3 is None else (e3 - t)/1e9}',
 'j1 = bisect.bisect_left(ack_t, t); e1 = None\n            while j1 < len(ack_t):\n                if ack_max[j1] >= s + 1: e1 = ack_t[j1]; break\n                j1 += 1\n            r = {"d_rtx": (tn - t)/1e9, "e3": None if e3 is None else (e3 - t)/1e9, "cls": ("pre_ev1" if (e1 is None or tn < e1) else ("ev1_ev3" if (e3 is None or tn < e3) else "post_ev3"))}')
exec(src)
cell = pathlib.Path(sys.argv[1])
for arm in sys.argv[2:]:
    E = []
    for rep in sorted((cell/arm).glob("rep*"), key=lambda p: int(p.name[3:])):
        E += rep_events(rep)
    n = len(E); out = []
    for c in ("pre_ev1", "ev1_ev3", "post_ev3"):
        v = [e["d_rtx"] for e in E if e["cls"] == c]
        out.append(f"{c} {len(v)/n:.2f} d_rtx p50={q(v,.5):.2f}")
    print(cell.name, arm, "n", n, " | ".join(out))
