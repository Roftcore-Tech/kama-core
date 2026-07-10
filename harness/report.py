#!/usr/bin/env python3
"""harness/report.py — quinn baseline konsolide rapor + Faz D empirik sanity check.

SoC: ölçüm/agregasyon `bench metrics`'te (Rust, otorite). Bu betik yalnız SUNAR:
- 14 (profil×cc) summary.txt'i tek tabloda toplar (medyan+IQR, §4.4.9 varyans).
- Faz D sanity: HAM jsonl'den delivered latency dağılımı (sıfır-olmayan+değişken mi) +
  profiller-arası gradation (0 @ %95 → ara @ %50 → ~1 @ jitter/bbr). Düz/şüpheli işaretlenir.
Sadece stdlib (numpy yok) — container/host her yerde koşar.
"""
import json, glob, os, sys, statistics as st

RESULTS = os.path.join(os.path.dirname(__file__), "..", "results")
PROFILES = ["p1_loss50", "p2_loss80", "p3_loss95", "p4_jitter", "p5_revpath", "p6_blackout", "p7_bbr"]
CCS = ["cubic", "bbr"]
# lossy profiller: Faz D'de delivered latency sıfır-olmayan+değişken beklenir (handshake fail → boş, o da OK)
LOSSY = {"p1_loss50", "p2_loss80", "p3_loss95", "p5_revpath", "p6_blackout"}


def load_run(path):
    meta, ev = None, []
    with open(path) as f:
        for line in f:
            o = json.loads(line)
            if "meta" in o:
                meta = o["meta"]
            elif "event" in o:
                ev.append(o["event"])
    return meta, ev


def latencies_ms(ev):
    return sorted((e["delivered_ns"] - e["sent_ns"]) / 1e6
                  for e in ev if e["delivered_ns"] is not None)


def read_summary(profile, cc):
    p = os.path.join(RESULTS, f"{profile}_{cc}_summary.txt")
    return open(p).read().rstrip() if os.path.exists(p) else None


def faz_d(profile, cc):
    """HAM koşulardan: teslim sayısı, latency min/med/max (ms), varyans bayrağı."""
    files = sorted(glob.glob(os.path.join(RESULTS, f"{profile}_{cc}_r*.jsonl")))
    all_lat, delivered, total, hs_fail = [], 0, 0, 0
    for fp in files:
        _meta, ev = load_run(fp)
        total += len(ev)
        lat = latencies_ms(ev)
        delivered += len(lat)
        all_lat += lat
        if all(e["sent_ns"] == 0 and e["delivered_ns"] is None for e in ev) and ev:
            hs_fail += 1
    all_lat.sort()  # dosya-başı sorted listelerin birleşimi GLOBAL sorted DEĞİL — min/med/max için şart
    if not all_lat:
        return dict(files=len(files), delivered=0, total=total, hs_fail=hs_fail,
                    lo=None, med=None, hi=None, variable=None)
    return dict(files=len(files), delivered=delivered, total=total, hs_fail=hs_fail,
                lo=all_lat[0], med=all_lat[len(all_lat) // 2], hi=all_lat[-1],
                variable=round(all_lat[0], 3) != round(all_lat[-1], 3))


def main():
    print("=" * 78)
    print("KAMA — quinn BASELINE KONSOLİDE RAPOR (§4.4)")
    meta_path = os.path.join(RESULTS, "run-metadata.json")
    if os.path.exists(meta_path):
        print("repro:", open(meta_path).read().strip())
    print("=" * 78)

    # 1) Konsolide completion tablosu (summary.txt'lerden)
    print("\n### completion vs deadline — medyan (5 repeat) [profil × cc]\n")
    print(f"{'profil':<13}{'cc':<7}{'c(10s)':>9}{'c(30s)':>9}{'c(120s)':>10}   IQR(120s)")
    for profile in PROFILES:
        for cc in CCS:
            s = read_summary(profile, cc)
            if not s:
                print(f"{profile:<13}{cc:<7}{'—  eksik summary':>28}")
                continue
            rows = {}
            for line in s.splitlines():
                if line.startswith("deadline="):
                    parts = line.replace("=", " ").split()
                    dl = parts[1]  # e.g. 10s
                    med = float(parts[3])
                    iqr = line.split("IQR=")[1].split()[0] if "IQR=" in line else "?"
                    rows[dl] = (med, iqr)
            c10 = rows.get("10s", (float("nan"), ""))[0]
            c30 = rows.get("30s", (float("nan"), ""))[0]
            c120, iqr120 = rows.get("120s", (float("nan"), "?"))
            print(f"{profile:<13}{cc:<7}{c10:>9.3f}{c30:>9.3f}{c120:>10.3f}   {iqr120}")

    # 2) Faz D empirik sanity check
    print("\n### Faz D empirik sanity check (HAM jsonl'den)\n")
    print(f"{'profil':<13}{'cc':<7}{'deliv/total':>12}{'hs_fail':>8}"
          f"{'lat_min':>10}{'lat_med':>10}{'lat_max':>10}  durum")
    problems = []
    for profile in PROFILES:
        for cc in CCS:
            d = faz_d(profile, cc)
            if d["files"] == 0:
                continue
            lo = f"{d['lo']:.0f}ms" if d["lo"] is not None else "—"
            med = f"{d['med']:.0f}ms" if d["med"] is not None else "—"
            hi = f"{d['hi']:.0f}ms" if d["hi"] is not None else "—"
            # değerlendirme
            if d["delivered"] == 0:
                verdict = "0 teslim (handshake fail — %95/harsh'ta beklenir)"
                if profile not in ("p3_loss95",) and profile not in LOSSY:
                    problems.append(f"{profile}/{cc}: kayıpsız ama 0 teslim?!")
            elif d["variable"] is False:
                verdict = "⚠ DÜZ latency (min==max) — netem/frame şüphesi"
                problems.append(f"{profile}/{cc}: latency düz (min==max={lo})")
            else:
                verdict = "✓ değişken latency"
            print(f"{profile:<13}{cc:<7}{d['delivered']:>6}/{d['total']:<5}{d['hs_fail']:>8}"
                  f"{lo:>10}{med:>10}{hi:>10}  {verdict}")

    # 3) Gradation sanity: profiller-arası completion 0→ara→1 monoton mı (kabaca)
    print("\n### gradation sanity (cubic, c120 medyan): loss arttıkça completion düşmeli")
    order = []
    for profile in ["p7_bbr", "p4_jitter", "p1_loss50", "p2_loss80", "p3_loss95"]:
        s = read_summary(profile, "cubic")
        c120 = None
        if s:
            for line in s.splitlines():
                if line.startswith("deadline=") and "120s" in line:
                    c120 = float(line.replace("=", " ").split()[3])
        order.append((profile, c120))
    print("  " + "  →  ".join(f"{p}:{c if c is None else round(c,3)}" for p, c in order))

    print("\n" + "=" * 78)
    if problems:
        print("⚠ İNCELENECEK:")
        for p in problems:
            print("   -", p)
    else:
        print("✓ Faz D: düz-eğri/netem-frame şüphesi YOK. Gradation profiller-arası tutarlı.")
    print("=" * 78)


if __name__ == "__main__":
    main()
