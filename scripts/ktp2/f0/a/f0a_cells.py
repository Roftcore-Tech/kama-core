#!/usr/bin/env python3
"""[KTP2-F0 · A] Akış bağımsızlığı hücre dosyalarının üreticisi (yalnız dosya yazar, koşu YAPMAZ).

    python3 -I scripts/ktp2/f0/a/f0a_cells.py [--out scripts/ktp2/f0/cells/a]

Her hücre `scripts/ktp2/f0/f0_cell.sh` biçimindedir: iki süreç (bob = sunucu ns-B, alice = istemci
ns-A), aynı argümanlar, yalnız `--role` farklı. İkililer ortam değişkenlerinden:
`BIN_F0A_TABAN`, `BIN_F0A_ADAY` (her kolun `target/release/examples/f0_flows`'u).

Profil dizeleri KATALOG.md §2'deki sırayla (`delay … distribution normal loss N% rate X`).
Parametrelerin gerekçesi `reports/ktp2/f0/a/RAPOR-A.md` §3'te.
"""

import argparse
import os

PROFILES = {
    "r12k_d300_l30": "delay 300ms 60ms distribution normal loss 30% rate 100kbit",
    "r1k_d300_l30": "delay 300ms 60ms distribution normal loss 30% rate 8kbit",
    "r12k_d1000_l30": "delay 1000ms 200ms distribution normal loss 30% rate 100kbit",
    # STRES (KATALOG dışı; İSTEK): hız sınırı yok — teslim kanalı taşmasının eşiğini 30 s
    # durmada aşabilecek alım hızı için. Karar yönlendirmez.
    "rinf_d300_l30": "delay 300ms 60ms distribution normal loss 30%",
}

# Dosya boyu = 990 B'lık parça sayısının katı (MAX_FILE_CHUNK_BYTES = 990; son parça da 1013 B
# birim → birim boyu sınıf eşlemesi çakışmaz). r12k: 530 parça (≈512 KiB); r1k: 66 parça (≈64 KiB).
FILE_BYTES = {
    "r12k_d300_l30": 530 * 990,
    "r1k_d300_l30": 66 * 990,
    "r12k_d1000_l30": 530 * 990,
    "rinf_d300_l30": 8474 * 990,
}

# Ortak zaman çizelgesi (t0 = alice Connected): 3 ısınma TEXT (0/0,5/1 s) · dosya 5 s · 40 TEXT
# 6 s'den 2,5 s arayla · 10 sinyal 7 s'den 10 s arayla · 2 PTT basışı 15 s ve 60 s (25 demet ×
# 200 ms, 40 B) · durma 20 s'den 30 s · karşı örnek 5 TEXT + 10 demet.
COMMON = [
    "--warmup-texts 3", "--warmup-every-ms 500",
    "--file-at-ms 5000",
    "--texts 40", "--text-at-ms 6000", "--text-every-ms 2500", "--text-bytes 64",
    "--sigs 10", "--sig-at-ms 7000", "--sig-every-ms 10000", "--sig-bytes 120",
    "--ptt-bundles 25", "--ptt-bundle-ms 200", "--ptt-bytes 40",
    "--stall-at-ms 20000", "--stall-ms 30000",
    "--karsi on", "--karsi-texts 5", "--karsi-every-ms 500", "--karsi-bundles 10",
    "--karsi-min-unacked 3", "--karsi-text-bytes 48",
    "--connect-timeout-s 30", "--probe-ms 200", "--live-diag on", "--receipt-grace-s 30",
]

# (hücre soneki, senaryo, profil, REPS, pencere s, bütçe s, ek argümanlar, açıklama, sınıf)
CELLS = [
    ("dosyasiz", "dosyasiz", "r12k_d300_l30", 30, 300, 600, [], "dosyasız taban", "birincil"),
    ("dosyali", "dosyali", "r12k_d300_l30", 30, 300, 600, [], "dosyalı, durmasız", "birincil"),
    ("dosyali_dur", "dosyali_dur", "r12k_d300_l30", 30, 300, 600, [],
     "K-1: dosyalı + 30 s tüketici durması", "birincil"),
    ("dosyasiz", "dosyasiz", "r1k_d300_l30", 30, 300, 900, [], "dosyasız taban", "birincil"),
    ("dosyali", "dosyali", "r1k_d300_l30", 30, 300, 900, [], "dosyalı, durmasız", "birincil"),
    ("dosyali_dur", "dosyali_dur", "r1k_d300_l30", 30, 300, 900, [],
     "K-1: dosyalı + 30 s tüketici durması", "birincil"),
    # Teslim kanalı taşması (orkestratör sorusu, 2026-10-29). Koşul: durma süresi × alım hızı >
    # 1024 birim. duman-2: yoğun PTT gönderen hızıyla sınırlı kaldı (pacer ≈ 76–334 B/s, FIFO
    # ≈ 2900) → taşma ölçülemedi; hücre bu gözlem için N=5 kalır. Taşma için: r12k'da dosya
    # alım hızı ≈ 6–7 birim/s (duman-2) → 240 s durma ≈ 1500 birim; hız sınırsız profilde 30 s.
    ("tasma_ptt", "dosyasiz_dur", "r12k_d300_l30", 5, 90, 480,
     ["--ptt-at-ms 15000", "--ptt-bundles 3000", "--ptt-bundle-ms 20"],
     "yoğun PTT + durma (gönderen hızı sınırlı)", "betimsel"),
    ("tasma_dosya", "dosyali_dur", "r12k_d300_l30", 10, 300, 600,
     ["--file-bytes 2097810", "--stall-at-ms 10000", "--stall-ms 240000"],
     "teslim kanalı taşması: 2 MB dosya + 240 s durma", "betimsel"),
    ("tasma_dosya", "dosyali_dur", "rinf_d300_l30", 10, 60, 480, [],
     "teslim kanalı taşması: hız sınırsız (STRES, KATALOG dışı) + 30 s durma", "betimsel"),
    ("dosyasiz_dur", "dosyasiz_dur", "r12k_d300_l30", 10, 300, 600, [],
     "durmanın tek başına etkisi", "betimsel"),
    ("dosyasiz", "dosyasiz", "r12k_d1000_l30", 10, 300, 900, [], "dosyasız taban (RTT 2 s)",
     "isteğe bağlı"),
    ("dosyali_dur", "dosyali_dur", "r12k_d1000_l30", 10, 300, 900, [],
     "K-1 (RTT 2 s)", "isteğe bağlı"),
]

TEMPLATE = """\
# [KTP2-F0 · A] {desc} — {klass}. Üretici: scripts/ktp2/f0/a/f0a_cells.py (elle düzenlemeyin).
# İkililer: BIN_F0A_TABAN / BIN_F0A_ADAY = <kol ağacı>/target/release/examples/f0_flows.
# Zarflar Unix soketinden (netem dışı, beyanlı); veri UDP + yön başına netem.
CELL={cell}
FWD="{netem}"
REV="{netem}"
REPS={reps}
ARMS="taban aday"
BUDGET_S={budget}
CAPTURE=0
SETTLE_S=1
F0A_ARGS='{args}'
ARM_taban_SRV="@BIN_F0A_TABAN@ --role bob $F0A_ARGS"
ARM_taban_CLI="@BIN_F0A_TABAN@ --role alice $F0A_ARGS"
ARM_aday_SRV="@BIN_F0A_ADAY@ --role bob $F0A_ARGS"
ARM_aday_CLI="@BIN_F0A_ADAY@ --role alice $F0A_ARGS"
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    here = os.path.dirname(os.path.abspath(__file__))
    ap.add_argument("--out", default=os.path.normpath(os.path.join(here, "..", "cells", "a")))
    ap.add_argument("--smoke", action="store_true",
                    help="REPS=1 duman kopyaları (<out>/duman/, hücre adı +_duman)")
    a = ap.parse_args()
    out_dir = os.path.join(a.out, "duman") if a.smoke else a.out
    os.makedirs(out_dir, exist_ok=True)
    for suffix, scenario, profile, reps, window, budget, extra, desc, klass in CELLS:
        cell = f"a_{profile}_{suffix}"
        if a.smoke:
            cell += "_duman"
            reps = 1
            klass = "duman (REPS=1)"
        args = [
            "--dir @D@", "--alice-addr @CLI_IP@:@PORT@", "--bob-addr @SRV_IP@:@PORT@",
            f"--scenario {scenario}", f"--profile {profile}", "--rep @REP@",
            f"--file-bytes {FILE_BYTES[profile]}", f"--window-s {window}",
        ] + COMMON + extra
        # Sonradan verilen bayrak öncekinin yerine geçmez (örnek çift bayrağı reddeder): ek
        # argümanların ortak listede karşılığı varsa ortaktakini çıkar.
        keys = {e.split()[0] for e in extra}
        args = [x for x in args if x.split()[0] not in keys or x in extra]
        if not any(x.startswith("--ptt-at-ms") for x in args):
            args.append("--ptt-at-ms 15000,60000")
        body = TEMPLATE.format(desc=desc, klass=klass, cell=cell, netem=PROFILES[profile],
                               reps=reps, budget=budget, args=" ".join(args))
        path = os.path.join(out_dir, f"{cell}.cell")
        with open(path, "w", encoding="utf-8") as f:
            f.write(body)
        print(path)


if __name__ == "__main__":
    main()
