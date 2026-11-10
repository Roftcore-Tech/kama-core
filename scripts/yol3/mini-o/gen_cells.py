#!/usr/bin/env python3
"""[YOL3 · Ö] mini-Ö hücreleri (KK-Ö-11, EK-A §6 / KK-Ö-EKA-1): (hücre × kol) başına bir dosya. Koşu YAPMAZ.

    python3 -I scripts/yol3/mini-o/gen_cells.py            # cells/ + SIRA.txt yazar
    python3 -I scripts/yol3/mini-o/gen_cells.py --check    # yazmaz; 5 dosya / 50 iş + bayt eşitliği
    python3 -I scripts/yol3/mini-o/gen_cells.py --komut    # orkestratörün koşu komutu (§8.3)

Seçenekler (varsayılanlar G4 mini-Ö'nündür; değişirse dosyalar yeniden üretilir ve SIRA.txt'nin
sha256'sı koşu kaydına girer):
    --kol K1          gösterim kolu = f0_flows ve bench `--e1-arm` değeri, İKİ uca aynı. Varsayılan
                      K1 (sahip v4 EK-A §2: "Gösterim seti = K1"). A paketi entegrasyon ağacına
                      girerse ör. `--kol K1A` (ikilide o kol yoksa koşu başlamaz: tipli ret).
    --taban K0        r25 kıyas kolu (KK-Ö-11: "gösterim seti ve K0, aynı ikili ve aynı FEC durumu").
    --playout-ms 2000 oynatma tamponu T (f0_flows `--playout-ms`; [1500, 2500]). G4: 2000 ("ölçüm
                      yoksa 2,0 s", v4 III.1); Paz yeniden koşusu: G4 okumasının T önerisi.
    --lane-w 5        W (teyitsiz dosya birimi). 5 = `LaneConfig::default()` (bayrak yazılmaz); başka
                      değer f0_flows'a `--lane-w N` olarak yazılır (KK-Ö-13; ikilide yoksa koşu
                      başlamaz: bilinmeyen bayrak reddi).

Hücreler (v4 IV G4 satırı; KK-Ö-11): gösterim analoğu `a_r12k_d150_l20_foto150k` (Y10, bağlayıcı
dört ölçüt), `b_r25_d300_l20_p65` × {kol, taban} (R3-r25 / K-3), `a_r12k_d300_l30_dosyali` ve
`a_r12k_d300_l0_dosyali` (G-K8 B1/B2/B5 raporlanan). N = 10. Toplam 5 dosya, 50 iş.

Geometri (FWD/REV, zaman çizelgesi, bütçe) kaynak hücrenin AYNISI: kaynak dosya ayrıştırılır, yalnız
CELL, REPS, ARMS ve kol/gösterim bayrakları değişir; kaynağın sha256'sı başlığa yazılır (kaynak
değişirse `--check` fark verir). A hücrelerinde her uçta `--e1-arm <kol> --fec-off on --goster on
--playout-ms <T>` (FEC kapalı: v4 Y1 "gösterim düğümünde runtime.fec = None"; şeritler + tampon:
YOL3-T1, f0_flows `--goster on`). B hücresinde (bench `kama-product`) `--fec-profile 0` ve
`--e1-arm`; bench'te şerit ve oynatma tamponu YOK (beyanlı: r25 satırı motor bayraklarını ölçer).

İkililer (orkestratör derler, D-34): BIN_GOSTER = <yol2/goster>/target/release/examples/f0_flows,
BIN_GOSTER_BENCH = <yol2/goster>/target/release/bench. Etiket: gösterim; kapılı PoC ölçümü Aralık;
ARQ yolu, FEC Aralık'ta; N < 30 betimsel.
"""

import argparse
import hashlib
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
CELLS_DIR = os.path.join(HERE, "cells")
SIRA = os.path.join(HERE, "SIRA.txt")
REL_CELLS = "scripts/yol3/mini-o/cells"

REPS = 10
DEFAULT_W = 5            # client-core lanes.rs DEFAULT_FILE_WINDOW_UNITS (YOL3-T1)
PLAYOUT_MIN_MS, PLAYOUT_MAX_MS = 1500, 2500   # kama-app playout.rs PLAYOUT_DELAY_MIN/MAX
ARM_NAME = re.compile(r"^[A-Za-z0-9_]+$")     # f0_cell.sh ARM_<kol>_SRV/CLI

Y10 = "scripts/yol3/cells/a/a_r12k_d150_l20_foto150k.cell"
L30 = "scripts/ktp2/f0/cells/a/a_r12k_d300_l30_dosyali.cell"
L0 = "scripts/yol3/cells/a/a_r12k_d300_l0_dosyali.cell"
R25 = "scripts/ktp2/f0/cells/b/b_r25_d300_l20_p65.cell"

A_KEYS = ["FWD", "REV", "BUDGET_S", "CAPTURE", "SETTLE_S", "F0A_ARGS"]
B_KEYS = ["RATE_ID", "DELAY_ID", "LOSS_PCT", "RATE_BPS", "DELAY_MS", "JITTER_MS", "MSGS", "PAY",
          "IVAL_MS", "DEADLINE_S", "FWD", "REV", "BUDGET_S", "CAPTURE", "SETTLE_S"]
LINE = re.compile(r"^([A-Z][A-Z0-9_]*)=(.*)$")
# Kaynak A hücresinde bulunmaması gereken (bu üreticinin yazdığı) bayraklar.
OWN_FLAGS = ("--e1-arm", "--fec-off", "--e1-edar", "--goster", "--playout-ms", "--lane-w")


def read_cell(rel):
    path = os.path.join(REPO, rel)
    with open(path, "rb") as f:
        raw = f.read()
    lines = {}
    for line in raw.decode("utf-8").splitlines():
        m = LINE.match(line)
        if m:
            if m.group(1) in lines:
                raise SystemExit(f"{rel}: {m.group(1)} iki kez")
            lines[m.group(1)] = line
    return lines, hashlib.sha256(raw).hexdigest()


def table(kol, taban):
    """[(kaynak, kol, amaç)] öncelik sırasıyla (SIRA.txt sırası)."""
    rows = [(Y10, kol, "gösterim analoğu (Y10): bağlayıcı dört ölçüt, T ve W teyidi")]
    rows.append((R25, kol, "R3-r25 / K-3: gösterim kolu"))
    if taban != kol:
        rows.append((R25, taban, "R3-r25 / K-3: taban kol (aynı ikili, aynı FEC durumu)"))
    rows.append((L30, kol, "G-K8 kötü ağ vekili: B1, B2, B5 raporlanan"))
    rows.append((L0, kol, "G-K8 iyi ağ vekili: B1, B2, B5 raporlanan"))
    return rows


def goster_text(playout_ms, lane_w):
    return (f"şeritler W = {lane_w}{' (LaneConfig::default)' if lane_w == DEFAULT_W else ''} + "
            f"oynatma tamponu T = {playout_ms} ms (--goster on)")


def header(cell, purpose, rel, sha):
    return [
        f"# [YOL3 · Ö · mini-Ö] {cell} — {purpose}. Üretici: scripts/yol3/mini-o/gen_cells.py "
        "(elle düzenlemeyin).",
        f"# Kaynak hücre: {rel} (sha256 {sha}) — geometri AYNEN; fark yalnız CELL, REPS, ARMS, "
        "kol ve gösterim bayrakları.",
    ]


def a_body(cell, arm, rel, sha, lines, purpose, playout_ms, lane_w):
    for k in A_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    m = re.match(r"^F0A_ARGS='(.*)'$", lines["F0A_ARGS"])
    if not m or "'" in m.group(1):
        raise SystemExit(f"{rel}: F0A_ARGS biçimi beklenen gibi değil")
    args = m.group(1).split()
    for flag in OWN_FLAGS:
        if flag in args:
            raise SystemExit(f"{rel}: kaynak hücrede {flag} zaten var")
    extra = [f"--e1-arm {arm}", "--fec-off on", "--goster on", f"--playout-ms {playout_ms}"]
    if lane_w != DEFAULT_W:
        extra.append(f"--lane-w {lane_w}")
    f0a = m.group(1) + " " + " ".join(extra)
    return "\n".join(header(cell, purpose, rel, sha) + [
        f"# Kol {arm} (f0_flows --e1-arm, iki uçta aynı) · FEC off (v4 Y1) · "
        f"{goster_text(playout_ms, lane_w)}.",
        "# İkili: BIN_GOSTER = <yol2/goster>/target/release/examples/f0_flows (orkestratör, D-34). "
        "İki ayrı süreç; zarflar Unix soketinden (netem dışı, beyanlı).",
        f"CELL={cell}",
        lines["FWD"],
        lines["REV"],
        f'REPS="${{F0_REPS:-{REPS}}}"',
        f'ARMS="{arm}"',
        lines["BUDGET_S"],
        lines["CAPTURE"],
        lines["SETTLE_S"],
        f"F0A_ARGS='{f0a}'",
        f'ARM_{arm}_SRV="@BIN_GOSTER@ --role bob $F0A_ARGS"',
        f'ARM_{arm}_CLI="@BIN_GOSTER@ --role alice $F0A_ARGS"',
        "",
    ])


def b_body(cell, arm, rel, sha, lines, purpose):
    for k in B_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    out = header(cell, purpose, rel, sha) + [
        f"# Kol {arm} (bench --e1-arm, iki uçta aynı) · FEC off (--fec-profile 0, v4 Y1) · "
        "bench'te şerit ve oynatma tamponu YOK (beyanlı).",
        "# İkili: BIN_GOSTER_BENCH = <yol2/goster>/target/release/bench (orkestratör, D-34); kol "
        "arms_kama.sh kama_e1_arm.",
        f"CELL={cell}",
    ]
    out += [lines[k] for k in B_KEYS if k not in ("BUDGET_S", "CAPTURE", "SETTLE_S")]
    out += [
        f'REPS="${{F0_REPS:-{REPS}}}"',
        f'ARMS="{arm}"',
        lines["BUDGET_S"],
        lines["CAPTURE"],
        lines["SETTLE_S"],
        'KAMA_SRV_EXTRA="--fec-profile 0"',
        'KAMA_CLI_EXTRA="--fec-profile 0"',
        "# shellcheck disable=SC1091",
        'source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../ktp2/f0/b" && pwd)/hucre_b.sh"',
        "# shellcheck disable=SC1091",
        'source "$F0_DIR/arms_kama.sh"',
        f"kama_e1_arm {arm} {arm} @BIN_GOSTER_BENCH@",
        "b_cap_wrap",
        "",
    ]
    return "\n".join(out)


def plan(kol, taban, playout_ms, lane_w):
    """[(dosya adı, gövde)] öncelik sırasıyla."""
    items = []
    for rel, arm, purpose in table(kol, taban):
        lines, sha = read_cell(rel)
        base = os.path.basename(rel)[: -len(".cell")]
        cell = f"{base}__{arm}"
        if base.startswith("a_"):
            body = a_body(cell, arm, rel, sha, lines, purpose, playout_ms, lane_w)
        else:
            body = b_body(cell, arm, rel, sha, lines, purpose)
        items.append((f"{cell}.cell", body))
    return items


def sira_text(items):
    return "".join(f"{REL_CELLS}/{name}\n" for name, _ in items)


def komut(items, a):
    return f"""\
# [YOL3 · Ö] mini-Ö koşusu (§8.3) — orkestratör koşar (D-18); ajan ve cargo durur (KK-Ö-EKA-5).
# Depo kökünden. İkililer yol2/goster release (orkestratör derler, D-34); SHA koşu kaydına.
# Kayıt: SIRA.txt sha256, ONKAYIT.md sha256, ikili sha256'ları; kol {a.kol} · taban {a.taban} ·
#   T {a.playout_ms} ms · W {a.lane_w} · FEC off. {len(items)} dosya, {len(items) * REPS} iş.
# Süre tahmini [türetilmiş]: -j 11 ile beş dosya paralel; en uzun dosya (d300 l30, 10 koşu × 2,5–5 dk)
#   ≈ 35–60 dk; seri ≈ 2–3 sa.
export BIN_GOSTER=<yol2/goster ağacı>/target/release/examples/f0_flows
export BIN_GOSTER_BENCH=<yol2/goster ağacı>/target/release/bench
systemd-inhibit --what=sleep:idle --why="YOL3 mini-O" setsid \\
  scripts/ktp2/f0/f0_grid.sh -j 11 ~/kama-k4g-work/yol3/mini-o/<tarih> $(cat scripts/yol3/mini-o/SIRA.txt)
# Okuma (koşu bitince; ön kayıt sha256'sı denetlenir, farklıysa okuma yok):
python3 -I scripts/yol3/mini-o/oku.py --kok ~/kama-k4g-work/yol3/mini-o/<tarih> \\
  --onkayit scripts/yol3/mini-o/ONKAYIT.md      # çıktı <kök>/okuma/{{OKUMA.md,okuma.json}}
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kol", default="K1")
    ap.add_argument("--taban", default="K0")
    ap.add_argument("--playout-ms", type=int, default=2000)
    ap.add_argument("--lane-w", type=int, default=DEFAULT_W)
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--komut", action="store_true")
    a = ap.parse_args()
    for name in (a.kol, a.taban):
        if not ARM_NAME.match(name):
            raise SystemExit(f"geçersiz kol adı: {name!r} (yalnız [A-Za-z0-9_])")
    if not PLAYOUT_MIN_MS <= a.playout_ms <= PLAYOUT_MAX_MS:
        raise SystemExit(f"--playout-ms {a.playout_ms} ∉ [{PLAYOUT_MIN_MS}, {PLAYOUT_MAX_MS}]")
    if a.lane_w < 1:
        raise SystemExit("--lane-w ≥ 1 olmalı")
    items = plan(a.kol, a.taban, a.playout_ms, a.lane_w)
    if a.komut:
        print(komut(items, a), end="")
        return 0
    desc = (f"kol={a.kol} taban={a.taban} T={a.playout_ms}ms W={a.lane_w}")
    if a.check:
        errs = []
        want = {name for name, _ in items}
        have = set(os.listdir(CELLS_DIR)) if os.path.isdir(CELLS_DIR) else set()
        for extra in sorted(have - want):
            errs.append(f"fazla dosya: cells/{extra}")
        for name, body in items:
            try:
                with open(os.path.join(CELLS_DIR, name), encoding="utf-8") as f:
                    if f.read() != body:
                        errs.append(f"cells/{name}: üretilenle aynı değil")
            except OSError as e:
                errs.append(f"cells/{name}: {e}")
        try:
            with open(SIRA, encoding="utf-8") as f:
                if f.read() != sira_text(items):
                    errs.append("SIRA.txt: üretilenle aynı değil")
        except OSError as e:
            errs.append(f"SIRA.txt: {e}")
        for e in errs:
            print(f"FARK: {e}", file=sys.stderr)
        print(f"{len(items)} dosya · {len(items) * REPS} iş · {desc} · "
              f"{'temiz' if not errs else f'{len(errs)} fark'}")
        return 1 if errs else 0
    os.makedirs(CELLS_DIR, exist_ok=True)
    want = {name for name, _ in items}
    for old in sorted(set(os.listdir(CELLS_DIR)) - want):
        if old.endswith(".cell"):
            os.remove(os.path.join(CELLS_DIR, old))
    for name, body in items:
        with open(os.path.join(CELLS_DIR, name), "w", encoding="utf-8") as f:
            f.write(body)
    with open(SIRA, "w", encoding="utf-8") as f:
        f.write(sira_text(items))
    print(f"{len(items)} dosya · {len(items) * REPS} iş · {desc} → {REL_CELLS}/ + "
          "scripts/yol3/mini-o/SIRA.txt")
    return 0


if __name__ == "__main__":
    sys.exit(main())
