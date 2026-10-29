#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] 05b netem-7 arşivinden srtt ↔ fiziksel RTT (betimsel, sonradan; salt okur).

Kaynak: reports/ktp2/f0/05b-netem/oturum-2-49864510/run-49864510.tar.xz (ham c.err'ler; açılmadan
okunur). Profiller scripts/netem/kapi_manifest.py:40-55 (aynen; netem7_cell.sh:24-33). Geometri:
100 × 256 B, aralık 50 ms (p6 hariç). Veri paketi tel boyu 42 + 16 + 11 + 256 = 325 B; ACK 73 B.
RTT_fiz = 2·gecikme + ser(325) + ser(73) (kuyruk yok, jitter ortalaması 0) — p4'te netem sıra-koruyan
jitter etkisi fizik.fifo_owd ile ayrıca verilir.

Kullanım: python3 -I arsiv_srtt.py [arşiv.tar.xz]
"""
import pathlib
import re
import statistics
import sys
import tarfile

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.dont_write_bytecode = True  # depoya __pycache__ yazılmasın
import fizik  # noqa: E402

ARSIV = HERE.parents[3] / "reports/ktp2/f0/05b-netem/oturum-2-49864510/run-49864510.tar.xz"
# profil → (hız B/s, tek yön gecikme s, σ s)
PROF = {"p1_loss50": (12500, 0.0, 0.0), "p2_loss80": (25000, 0.0, 0.0),
        "p4_jitter": (12500, 0.2, 0.1), "p5_revpath": (12500, 0.0, 0.0), "p7_bbr": (6250, 0.0, 0.0)}


def main():
    path = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ARSIV
    rows = {}
    with tarfile.open(path, "r:xz") as tf:
        for m in tf:
            mm = re.match(r".*/(p\d_\w+)/(taban|aday)/rep(\d+)/c\.err$", m.name)
            if not mm or mm.group(1) not in PROF:
                continue
            txt = tf.extractfile(m).read().decode(errors="replace")
            s = re.search(r"\bsrtt=(\d+)us", txt)
            n = re.search(r"rtt_samples=(\d+)", txt)
            tp = re.search(r"tail_probe=(\d+)@", txt)
            rows.setdefault((mm.group(1), mm.group(2)), []).append(
                (int(s.group(1)) / 1e6 if s else 0.0, int(n.group(1)) if n else 0, int(tp.group(1)) if tp else 0))
    print("| profil | RTT_fiz (ms) | RTT_fifo (ms) | kol | srtt medyan (ms) | min–maks (ms) | srtt>0 / N | "
          "srtt/RTT_fiz medyan | rtt_samples medyan | tail_probe medyan |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for p, (R, d, sg) in PROF.items():
        rtt = 2 * d + 325 / R + 73 / R
        f, _ = fizik.fifo_owd(d, sg, 0.05, 325 / R) if sg else (d, 0)
        r_, _ = fizik.fifo_owd(d, sg, 0.10, 73 / R) if sg else (d, 0)
        rtt_f = f + 325 / R + r_ + 73 / R
        for a in ("taban", "aday"):
            v = rows.get((p, a), [])
            sr = sorted(x[0] for x in v if x[0] > 0)
            if not sr:
                continue
            print(f"| {p} | {rtt * 1000:.1f} | {rtt_f * 1000:.1f} | {a} | {statistics.median(sr) * 1000:.1f} | "
                  f"{sr[0] * 1000:.1f}–{sr[-1] * 1000:.1f} | {len(sr)}/{len(v)} | {statistics.median(sr) / rtt:.2f} | "
                  f"{statistics.median(x[1] for x in v):.0f} | {statistics.median(x[2] for x in v):.1f} |")


if __name__ == "__main__":
    main()
