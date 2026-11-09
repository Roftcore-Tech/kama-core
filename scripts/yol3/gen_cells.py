#!/usr/bin/env python3
"""[YOL3 · Ö] Yeni A hücreleri (KK-Ö-07): yalnız dosya yazar, koşu YAPMAZ.

    python3 -I scripts/yol3/gen_cells.py [--out scripts/yol3/cells/a] [--check]

İki hücre (v4 Y10; v4 I.3 DEMO erken uyarısı):

* `a_r12k_d150_l20_foto150k` — gösterim profili analoğu. FWD = REV = `delay 150ms loss 20%
  rate 100kbit`: JİTTER YOK (gösterim `tc`'siyle aynı, `scripts/netem/goster-tc.sh`; sahip
  2026-11-09: jitter ayrı bir profil kararı). KATALOG §2'nin σ = D/5 kuralı YALNIZ bu hücreye
  uygulanmaz. `--scenario dosyali`, `--file-bytes 150480` (152 × 990 B; "150 KB"nin 990 B parça
  katı), `--ptt-bytes 80`.
* `a_r12k_d300_l0_dosyali` — l30 kardeşinin (`scripts/ktp2/f0/cells/a/a_r12k_d300_l30_dosyali.cell`)
  aynı geometrisi, kayıpsız: `delay 300ms 60ms distribution normal rate 100kbit` (KATALOG §2: l0'da
  `loss` parçası yazılmaz), dosya 524 700 B (530 × 990 B).

Geri kalan her şey F0 A üreticisinin ortak zaman çizelgesi (`COMMON`) ve şablonu (`TEMPLATE`)
AYNEN: kaynak `scripts/ktp2/f0/a/f0a_cells.py` (origin/ktp2/f0-taban 17973f7c). F0 üreticisi ve
F0 A hücre dosyaları DEĞİŞMEZ; bu dosya onları import etmez (kopya, kaynak atfıyla), böylece F0
ağacındaki bir değişiklik bu hücreleri sessizce değiştirmez. `--check` kopyanın kaynakla aynı
kaldığını ve yazılı dosyaların üretilenle bayt bayt aynı olduğunu doğrular (yazmaz).
"""

import argparse
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
F0A_GEN = os.path.join(REPO, "scripts", "ktp2", "f0", "a", "f0a_cells.py")

# ── F0 A üreticisinden AYNEN kopya (scripts/ktp2/f0/a/f0a_cells.py `COMMON`) ──────────────────
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

# ── F0 A üreticisinden AYNEN kopya (`TEMPLATE`); yalnız ilk satırın üretici atfı farklı ───────
TEMPLATE = """\
# [YOL3 · Ö] {desc} — {klass}. Üretici: scripts/yol3/gen_cells.py (elle düzenlemeyin).
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

# Parça boyu (F0 üreticisi: MAX_FILE_CHUNK_BYTES = 990).
CHUNK = 990

PROFILES = {
    # Gösterim profili analoğu (v4 Y10; KK-Ö-07): jitter yok.
    "r12k_d150_l20": "delay 150ms loss 20% rate 100kbit",
    # KATALOG §2 dizgisi, l0 (loss parçası yok); F0 r12k_d300_l30 ile aynı delay/σ/rate.
    "r12k_d300_l0": "delay 300ms 60ms distribution normal rate 100kbit",
}

# (hücre adı, senaryo, profil, dosya baytı, REPS, pencere s, bütçe s, ek argümanlar, açıklama, sınıf)
# Pencere/bütçe r12k_d300 F0 kardeşleriyle aynı (300 s / 600 s). REPS = 10 (mini-Ö N, v4 IV G4).
CELLS = [
    ("a_r12k_d150_l20_foto150k", "dosyali", "r12k_d150_l20", 152 * CHUNK, 10, 300, 600,
     ["--ptt-bytes 80"], "gösterim analoğu (Y10): 150 KB fotoğraf, 80 B demet, jitter yok",
     "mini-Ö / teşhis"),
    ("a_r12k_d300_l0_dosyali", "dosyali", "r12k_d300_l0", 530 * CHUNK, 10, 300, 600,
     [], "dosyalı, durmasız, kayıpsız (l30 kardeşinin geometrisi)", "mini-Ö"),
]


def cell_body(name, scenario, profile, file_bytes, reps, window, budget, extra, desc, klass):
    args = [
        "--dir @D@", "--alice-addr @CLI_IP@:@PORT@", "--bob-addr @SRV_IP@:@PORT@",
        f"--scenario {scenario}", f"--profile {profile}", "--rep @REP@",
        f"--file-bytes {file_bytes}", f"--window-s {window}",
    ] + COMMON + extra
    # F0 üreticisiyle aynı kural: örnek çift bayrağı reddeder; ek argümanın ortak listede
    # karşılığı varsa ortaktaki çıkar.
    keys = {e.split()[0] for e in extra}
    args = [x for x in args if x.split()[0] not in keys or x in extra]
    if not any(x.startswith("--ptt-at-ms") for x in args):
        args.append("--ptt-at-ms 15000,60000")
    return TEMPLATE.format(desc=desc, klass=klass, cell=name, netem=PROFILES[profile],
                           reps=reps, budget=budget, args=" ".join(args))


def source_block(text, name):
    """F0 üreticisinden `NAME = [...]` ya da `NAME = \"\"\"...\"\"\"` bloğunun metni."""
    m = re.search(rf'^{name} = (\[.*?^\]|""".*?^""")', text, re.S | re.M)
    return m.group(1) if m else None


def check_copy():
    """Kopyalar F0 kaynağıyla aynı mı (TEMPLATE'te yalnız ilk satır farklı olabilir)."""
    errs = []
    try:
        with open(F0A_GEN, encoding="utf-8") as f:
            src = f.read()
    except OSError as e:
        return [f"F0 A üreticisi okunamadı: {e}"]
    with open(os.path.abspath(__file__), encoding="utf-8") as f:
        mine = f.read()
    for name in ("COMMON", "TEMPLATE"):
        a, b = source_block(src, name), source_block(mine, name)
        if a is None or b is None:
            errs.append(f"{name} bloğu bulunamadı")
            continue
        if name == "TEMPLATE":
            a, b = a.split("\n", 2)[2], b.split("\n", 2)[2]
        if a != b:
            errs.append(f"{name} F0 kaynağından ayrışmış ({F0A_GEN})")
    return errs


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", default=os.path.join(HERE, "cells", "a"))
    ap.add_argument("--check", action="store_true",
                    help="yazma; kopyayı ve diskteki dosyaları doğrula (çıkış 1 = fark)")
    a = ap.parse_args()
    errs = check_copy() if a.check else []
    if not a.check:
        os.makedirs(a.out, exist_ok=True)
    for spec in CELLS:
        body = cell_body(*spec)
        path = os.path.join(a.out, f"{spec[0]}.cell")
        if a.check:
            try:
                with open(path, encoding="utf-8") as f:
                    if f.read() != body:
                        errs.append(f"{path}: üretilenle aynı değil")
            except OSError as e:
                errs.append(f"{path}: {e}")
        else:
            with open(path, "w", encoding="utf-8") as f:
                f.write(body)
            print(path)
    if a.check:
        for e in errs:
            print(f"FARK: {e}", file=sys.stderr)
        print(f"{len(CELLS)} hücre · {'temiz' if not errs else f'{len(errs)} fark'}")
        return 1 if errs else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
