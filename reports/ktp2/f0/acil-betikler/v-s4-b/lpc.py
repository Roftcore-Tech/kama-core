import json, pathlib, statistics as st, sys
base = pathlib.Path("/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out")
for cell in sys.argv[1:]:
    for arm in ("taban", "aday"):
        v = []; none = 0
        for rep in (base/cell/arm).glob("rep*"):
            try:
                m = json.loads((rep/"out.jsonl").open().readline())["meta"]
                x = m.get("activation", {}).get("left_preconverge_ns")
                if x: v.append(x/1e9)
                else: none += 1
            except Exception as e: none += 1
        v.sort()
        print(f"{cell} {arm} n={len(v)} none={none} p50={st.median(v):.2f} max={max(v):.2f}")
