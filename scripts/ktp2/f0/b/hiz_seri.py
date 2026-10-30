#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] Gönderim hızı zaman serisi (yakalama, salt okur).

Her koşu için `w` saniyelik pencerelerde: istemciden çıkan UDP yükü baytı (Kama: orijinal/kopya ayrı),
sunulan birim sayısına göre gönderen kuyruğu (sunulan − ilk iletilen). Kol ve hücre başına koşular
arası medyan. quinn/tcp için yalnız toplam bayt (şifreli / segment).

Kullanım: python3 -I hiz_seri.py <runs/tam/b/out> <çıktı.csv> [w_s] [hücre…]
"""
import csv
import pathlib
import statistics
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.dont_write_bytecode = True
import analiz_b as A  # noqa: E402
import fizik  # noqa: E402


def run_series(rep, w):
    cli = A.read_cap(rep / "capb_cli.csv.gz")
    if cli is None:
        return None
    meta, ev = A.read_events(rep)
    port = A.server_port(rep)
    t0 = (meta.get("t0_unix_ns") or 0) / 1e9
    off = meta.get("events_epoch_offset_ns", 0)
    kama = (meta.get("transport") or "").startswith("kama")
    pk = []
    for t, d, pr, ipl, sp, dp, x in cli:
        if d != "o" or dp != port:
            continue
        if pr == 17:
            k = A.kama_decode(x, ipl - 28) if kama and x else None
            pk.append((t / 1e9, ipl - 28, k))
        elif pr == 6:
            pk.append((t / 1e9, ipl - 40, None))
    if not pk:
        return None
    if not t0:
        t0 = pk[0][0]
    offers = sorted(t0 + (e["sent_ns"] - off) / 1e9 for e in ev if e.get("sent_ns") is not None)
    bins = {}
    seen = set()
    first_tx = []
    for t, n, k in pk:
        b = int((t - t0) // w)
        rec = bins.setdefault(b, {"top": 0, "orj": 0, "kop": 0})
        rec["top"] += n
        if k and k["type"] == "data":
            if k["unit"] in seen:
                rec["kop"] += n
            else:
                seen.add(k["unit"])
                rec["orj"] += n
                first_tx.append(t)
    out = {}
    for b, rec in bins.items():
        te = t0 + (b + 1) * w
        out[b] = dict(rec)
        if kama:
            out[b]["kuyruk"] = sum(1 for o in offers if o <= te) - sum(1 for f in first_tx if f <= te)
    return out


def main():
    root = pathlib.Path(sys.argv[1])
    w = float(sys.argv[3]) if len(sys.argv) > 3 else 5.0
    want = sys.argv[4:] or sorted(p.name for p in root.iterdir() if p.is_dir() and p.name.startswith("b_r12k"))
    geoms = fizik.by_cell()
    rows = []
    for c in want:
        g = geoms[c]
        for arm in g["arms"]:
            per = {}
            n = 0
            for rep in sorted((root / c / arm).glob("rep*")):
                s = run_series(rep, w)
                if s is None:
                    continue
                n += 1
                for b, v in s.items():
                    per.setdefault(b, []).append(v)
            for b in sorted(per):
                v = per[b]
                rows.append({"hucre": c, "kol": arm, "t_s": b * w, "n": len(v),
                             "top_Bps": statistics.median(x["top"] for x in v) / w,
                             "orj_Bps": statistics.median(x["orj"] for x in v) / w,
                             "kop_Bps": statistics.median(x["kop"] for x in v) / w,
                             "gonderen_kuyruk": statistics.median(x.get("kuyruk", 0) for x in v)})
    with open(sys.argv[2], "w", newline="") as f:
        wr = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        wr.writeheader()
        wr.writerows(rows)
    print(len(rows), "satır")


if __name__ == "__main__":
    main()
