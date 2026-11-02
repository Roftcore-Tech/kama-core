"""[v-m4] Bağımsız yeniden hesap: koşu başına p95(L=rx-sunum), Kama için S=ilk DATA çıkışı-sunum,
N=rx-ilk DATA çıkışı. dec_b.py'den farklı olarak: DATA çerçevesini ip_len filtresiyle DEĞİL çerçeve
türü (0x00) ve başlıktan okur; ip_len sapmalarını, kapsama oranını, S<0 sayısını ve yakalama
düşüşlerini ayrıca raporlar. Salt okuma."""
import sys, os, json, gzip, math, statistics as st

ROOT = sys.argv[1]
OUT = sys.argv[2]


def pct(v, q):
    v = sorted(v)
    if not v:
        return None
    pos = q * (len(v) - 1)
    lo = math.floor(pos)
    hi = min(lo + 1, len(v) - 1)
    return v[lo] + (v[hi] - v[lo]) * (pos - lo)


def load(rep, pay, kama):
    p = os.path.join(rep, "out.jsonl")
    if not os.path.exists(p):
        return None
    lines = open(p).read().splitlines()
    meta = json.loads(lines[0])["meta"]
    t0u = meta["t0_unix_ns"]
    off = meta.get("events_epoch_offset_ns", 0)
    offer = {}
    for l in lines[1:]:
        o = json.loads(l).get("event")
        if o:
            offer[o["msg_id"]] = (t0u + o["sent_ns"] - off) / 1e9
    rx = {}
    rp = os.path.join(rep, "rx_log.jsonl")
    if os.path.exists(rp):
        for l in open(rp).read().splitlines()[1:]:
            o = json.loads(l)
            if o["u"] not in rx:
                rx[o["u"]] = o["t"] / 1e9
    res = {"n": len(offer), "nrx": sum(1 for i in offer if i in rx)}
    L = [rx[i] - offer[i] for i in offer if i in rx]
    res["L95"] = pct(L, .95)
    res["L95_inf"] = None
    # p95, teslim edilmeyen = +sonsuz
    Linf = L + [1e9] * (len(offer) - len(L))
    res["L95_inf"] = pct(Linf, .95)
    if kama:
        fn = os.path.join(rep, "capb_cli.csv.gz")
        ftx = {}
        ntx = {}
        badlen = 0
        drops = None
        exp = 20 + 8 + 16 + 11 + pay
        if os.path.exists(fn):
            with gzip.open(fn, "rt") as f:
                for line in f:
                    if line.startswith("#"):
                        a = line.strip().split(",")
                        if a[0] == "#stats":
                            drops = int(a[2])
                        continue
                    a = line.rstrip("\n").split(",")
                    if len(a) < 7 or a[1] != "o" or a[2] != "17":
                        continue
                    x = a[6]
                    if not x.startswith("00"):
                        continue
                    if int(a[3]) != exp:
                        badlen += 1
                    u = int(x[6:22], 16)
                    t = int(a[0]) / 1e9
                    ntx[u] = ntx.get(u, 0) + 1
                    if u not in ftx or t < ftx[u]:
                        ftx[u] = t
        S = [ftx[i] - offer[i] for i in offer if i in ftx and i in rx]
        N = [rx[i] - ftx[i] for i in offer if i in ftx and i in rx]
        res.update({"S95": pct(S, .95), "N95": pct(N, .95), "cov": len(S) / max(1, res["nrx"]),
                    "Sneg": sum(1 for s in S if s < -0.002), "badlen": badlen, "drops": drops,
                    "S50": pct(S, .5), "N50": pct(N, .5)})
    return res


cells = []
for rid in ("r1k", "r12k"):
    for did in ("d300", "d1000"):
        for loss in (0, 10, 20, 30):
            for p in (65, 1000):
                cells.append((f"{rid}_{did}_l{loss}_p{p}", p))
fo = open(OUT, "w")
for cell, pay in cells:
    for arm in ("taban", "aday", "quinn"):
        d = os.path.join(ROOT, "b_" + cell, arm)
        if not os.path.isdir(d):
            continue
        rows = []
        for rep in sorted(os.listdir(d)):
            r = load(os.path.join(d, rep), pay, arm != "quinn")
            if r and r["nrx"] > 0:
                rows.append(r)
        o = {"cell": cell, "arm": arm, "N": len(rows)}
        for k in ("L95", "L95_inf", "S95", "N95", "S50", "N50", "cov"):
            v = [r[k] for r in rows if r.get(k) is not None]
            o[k] = pct(v, .5) if v else None
        o["deliv"] = sum(r["nrx"] for r in rows) / max(1, sum(r["n"] for r in rows))
        if arm != "quinn":
            o["Sneg"] = sum(r["Sneg"] for r in rows)
            o["badlen"] = sum(r["badlen"] for r in rows)
            o["drops"] = sum((r["drops"] or 0) for r in rows)
            o["covmin"] = min(r["cov"] for r in rows)
        fo.write(json.dumps(o) + "\n")
        fo.flush()
        print(json.dumps(o), flush=True)
