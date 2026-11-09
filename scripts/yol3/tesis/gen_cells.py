#!/usr/bin/env python3
"""[YOL3 · Ö] E1 teşhis hücreleri (CC §4.2; KK-Ö-06): (hücre × kol) başına bir dosya. Koşu YAPMAZ.

    python3 -I scripts/yol3/tesis/gen_cells.py            # cells/ + SIRA.txt yazar
    python3 -I scripts/yol3/tesis/gen_cells.py --check    # yazmaz; 16 dosya / 160 iş + bayt eşitliği
    python3 -I scripts/yol3/tesis/gen_cells.py --komut    # orkestratörün koşu komutu (§8.2)

Seçenekler (M2 manifestinin satırları; varsayılanlar aşağıda, değişirse dosyalar yeniden üretilir):
    --analog y10|l30   gösterim analoğu: `a_r12k_d150_l20_foto150k` (Y10, varsayılan) ya da
                       `a_r12k_d300_l30_dosyali` (CC §4.2 "Y10 hazırsa, yoksa …")
    --fec urun|off     bütün kollarda FEC durumu: `urun` (varsayılan; K0 = a858f3fc + düzenek, ürün
                       profili) ya da `off` (`fec = None`: f0_flows `--fec-off on`, bench
                       `--fec-profile 0`)
    --edar-yok         ön test geçmediyse (CC §4.2): her kolda `--e1-edar off` (e1_probe_keep düşer);
                       K2−e K2 ile aynı sete indiği için çıkar (15 dosya, 150 iş)

Geometri (FWD/REV, zaman çizelgesi, bütçe) kaynak F0/YOL3 hücresinin AYNISI: kaynak dosya
ayrıştırılır, yalnız CELL, REPS, ARMS ve kol bayrağı değişir. Kaynak dosyanın sha256'sı başlığa
yazılır (kaynak değişirse `--check` fark verir). Kol → bayrak eşlemesi M2'nin teşhis manifestindeki
tablonun aynısıdır (docs/design/E1-TESHIS-G2.md, KK-M2-09); burada yalnız başlık açıklamasıdır —
bayrakları ikili `--e1-arm` değerinden kurar (bench `kama-product` ve f0_flows, iki uçta aynı).

İkililer (orkestratör derler, D-34): BIN_TESIS = <yol2/o1>/target/release/examples/f0_flows,
BIN_TESIS_BENCH = <yol2/o1>/target/release/bench.
"""

import argparse
import hashlib
import os
import re
import shlex
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
CELLS_DIR = os.path.join(HERE, "cells")
SIRA = os.path.join(HERE, "SIRA.txt")

REPS = 10

# Kol → bayraklar (M2 manifesti KK-M2-09; CC §4.2). Sıra: (c) e1_startup_cwnd · (d)
# e1_applimited_plateau · DrainRtprop e1_drainrt_gate · (e-dar) e1_probe_keep · retx_first.
ARM_FLAGS = {
    "K0": [],
    "K0r": ["retx_first"],
    "K1": ["c", "d", "DrainRtprop", "e-dar"],
    "K2": ["c", "d", "DrainRtprop", "e-dar", "retx_first"],
    "K2mc": ["d", "DrainRtprop", "e-dar", "retx_first"],
    "K2me": ["c", "d", "DrainRtprop", "retx_first"],
}
ALL_ARMS = ["K0", "K0r", "K1", "K2", "K2mc", "K2me"]

ANALOG = {
    "y10": "scripts/yol3/cells/a/a_r12k_d150_l20_foto150k.cell",
    "l30": "scripts/ktp2/f0/cells/a/a_r12k_d300_l30_dosyali.cell",
}

# (sıra, kaynak hücre dosyası | "ANALOG", kollar, okuma amacı) — CC §4.2 hücre tablosu.
TABLE = [
    (1, "ANALOG", ALL_ARMS, "seçim: PTT p95, dosya bitişi"),
    (1, "scripts/ktp2/f0/cells/a/a_r12k_d300_l30_dosyasiz.cell", ["K0", "K2", "K2mc"],
     "PTT-yalnız oturumda yeni veri emisyonu"),
    (2, "scripts/ktp2/f0/cells/b/b_r12k_d1000_l30_p1000.cell", ["K0", "K2", "K2mc"],
     "manşet: E1 var/yok (hız)"),
    (3, "scripts/ktp2/f0/cells/b/b_r12k_d1000_l0_p1000.cell", ["K0", "K2"], "koruma: T0"),
    (3, "scripts/ktp2/f0/cells/b/b_r25_d300_l20_p65.cell", ["K0", "K2"], "koruma: K-3"),
]

A_KEYS = ["FWD", "REV", "BUDGET_S", "CAPTURE", "SETTLE_S", "F0A_ARGS"]
B_KEYS = ["RATE_ID", "DELAY_ID", "LOSS_PCT", "RATE_BPS", "DELAY_MS", "JITTER_MS", "MSGS", "PAY",
          "IVAL_MS", "DEADLINE_S", "FWD", "REV", "BUDGET_S", "CAPTURE", "SETTLE_S"]
LINE = re.compile(r"^([A-Z][A-Z0-9_]*)=(.*)$")


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


def flags_text(arm, edar_yok):
    fl = [f for f in ARM_FLAGS[arm] if not (edar_yok and f == "e-dar")]
    return "+".join(fl) if fl else "hiçbiri (bayraklar kapalı)"


def a_body(cell, arm, rel, sha, lines, fec, edar_yok, purpose, row):
    for k in A_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    m = re.match(r"^F0A_ARGS='(.*)'$", lines["F0A_ARGS"])
    if not m or "'" in m.group(1):
        raise SystemExit(f"{rel}: F0A_ARGS biçimi beklenen gibi değil")
    args = m.group(1).split()
    for flag in ("--e1-arm", "--fec-off", "--e1-edar"):
        if flag in args:
            raise SystemExit(f"{rel}: kaynak hücrede {flag} zaten var")
    extra = [f"--e1-arm {arm}"]
    if fec == "off":
        extra.append("--fec-off on")
    if edar_yok:
        extra.append("--e1-edar off")
    f0a = m.group(1) + " " + " ".join(extra)
    return "\n".join([
        f"# [YOL3 · Ö · teşhis] {cell} — sıra {row}; {purpose}. Üretici: "
        "scripts/yol3/tesis/gen_cells.py (elle düzenlemeyin).",
        f"# Kaynak hücre: {rel} (sha256 {sha}) — geometri AYNEN; fark yalnız CELL, REPS, ARMS, "
        "kol bayrağı.",
        f"# Kol {arm}: {flags_text(arm, edar_yok)} (M2 manifesti KK-M2-09) · FEC {fec}.",
        "# İkili: BIN_TESIS = <yol2/o1>/target/release/examples/f0_flows (orkestratör, D-34). "
        "Zarflar Unix soketinden (netem dışı, beyanlı).",
        f"CELL={cell}",
        lines["FWD"],
        lines["REV"],
        f'REPS="${{F0_REPS:-{REPS}}}"',
        f'ARMS="{arm}"',
        lines["BUDGET_S"],
        lines["CAPTURE"],
        lines["SETTLE_S"],
        f"F0A_ARGS='{f0a}'",
        f'ARM_{arm}_SRV="@BIN_TESIS@ --role bob $F0A_ARGS"',
        f'ARM_{arm}_CLI="@BIN_TESIS@ --role alice $F0A_ARGS"',
        "",
    ])


def b_body(cell, arm, rel, sha, lines, fec, edar_yok, purpose, row):
    for k in B_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    extra = []
    if fec == "off":
        extra.append("--fec-profile 0")
    if edar_yok:
        extra.append("--e1-edar off")
    out = [
        f"# [YOL3 · Ö · teşhis] {cell} — sıra {row}; {purpose}. Üretici: "
        "scripts/yol3/tesis/gen_cells.py (elle düzenlemeyin).",
        f"# Kaynak hücre: {rel} (sha256 {sha}) — geometri AYNEN; fark yalnız CELL, REPS, ARMS, "
        "kol bayrağı.",
        f"# Kol {arm}: {flags_text(arm, edar_yok)} (M2 manifesti KK-M2-09) · FEC {fec}.",
        "# İkili: BIN_TESIS_BENCH = <yol2/o1>/target/release/bench (orkestratör, D-34); kol "
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
    ]
    if extra:
        x = " ".join(extra)
        out += [f'KAMA_SRV_EXTRA="{x}"', f'KAMA_CLI_EXTRA="{x}"']
    out += [
        "# shellcheck disable=SC1091",
        'source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../ktp2/f0/b" && pwd)/hucre_b.sh"',
        "# shellcheck disable=SC1091",
        'source "$F0_DIR/arms_kama.sh"',
        f"kama_e1_arm {arm} {arm}",
        "b_cap_wrap",
        "",
    ]
    return "\n".join(out)


def plan(analog, fec, edar_yok):
    """[(sıra, dosya adı, gövde)] öncelik sırasıyla."""
    items = []
    for row, src, arms, purpose in TABLE:
        rel = ANALOG[analog] if src == "ANALOG" else src
        lines, sha = read_cell(rel)
        base = os.path.basename(rel)[: -len(".cell")]
        for arm in arms:
            if edar_yok and arm == "K2me":
                continue  # (e-dar) çıkınca K2−e = K2 (CC §4.2 ön testler; §10)
            cell = f"{base}__{arm}"
            body_fn = a_body if base.startswith("a_") else b_body
            body = body_fn(cell, arm, rel, sha, lines, fec, edar_yok, purpose, row)
            items.append((row, f"{cell}.cell", body))
    return items


def sira_text(items):
    return "".join(f"scripts/yol3/tesis/cells/{name}\n" for _, name, _ in items)


def komut(items):
    n1 = sum(1 for row, _, _ in items if row == 1)
    return f"""\
# [YOL3 · Ö] Teşhis koşusu (§8.2) — orkestratör koşar (D-18); ajan ve cargo durur (v2 §11.5).
# Depo kökünden. İkililer yol2/o1 release (orkestratör derler, D-34).
export BIN_TESIS=<yol2/o1 ağacı>/target/release/examples/f0_flows
export BIN_TESIS_BENCH=<yol2/o1 ağacı>/target/release/bench
# Tamamı ({len(items)} dosya, {len(items) * REPS} iş; ilk {n1} satır = 1. sıra, {n1 * REPS} iş):
systemd-inhibit --what=sleep:idle --why="YOL3 teshis" setsid \\
  scripts/ktp2/f0/f0_grid.sh -j 11 ~/kama-k4g-work/yol3/tesis/2026-11-11 $(cat scripts/yol3/tesis/SIRA.txt)
# Yalnız 1. sıra önce (ayrı parti; aynı kök, DONE'u olan hücre ikinci partide atlanır):
#   ... f0_grid.sh -j 11 ~/kama-k4g-work/yol3/tesis/2026-11-11 $(head -n {n1} scripts/yol3/tesis/SIRA.txt)
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--analog", choices=sorted(ANALOG), default="y10")
    ap.add_argument("--fec", choices=["urun", "off"], default="urun")
    ap.add_argument("--edar-yok", action="store_true")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--komut", action="store_true")
    a = ap.parse_args()
    items = plan(a.analog, a.fec, a.edar_yok)
    if a.komut:
        print(komut(items), end="")
        return 0
    if a.check:
        errs = []
        want = {name for _, name, _ in items}
        have = set(os.listdir(CELLS_DIR)) if os.path.isdir(CELLS_DIR) else set()
        for extra in sorted(have - want):
            errs.append(f"fazla dosya: cells/{extra}")
        for _, name, body in items:
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
        n1 = sum(1 for row, _, _ in items if row == 1)
        print(f"{len(items)} dosya · {len(items) * REPS} iş · 1. sıra {n1} dosya / {n1 * REPS} iş"
              f" · analog={a.analog} fec={a.fec} edar_yok={a.edar_yok} · "
              f"{'temiz' if not errs else f'{len(errs)} fark'}")
        return 1 if errs else 0
    os.makedirs(CELLS_DIR, exist_ok=True)
    want = {name for _, name, _ in items}
    for old in os.listdir(CELLS_DIR):
        if old.endswith(".cell") and old not in want:
            os.remove(os.path.join(CELLS_DIR, old))
    for _, name, body in items:
        with open(os.path.join(CELLS_DIR, name), "w", encoding="utf-8") as f:
            f.write(body)
    with open(SIRA, "w", encoding="utf-8") as f:
        f.write(sira_text(items))
    print(f"{len(items)} dosya, {len(items) * REPS} iş → {os.path.relpath(CELLS_DIR, REPO)}; "
          f"{os.path.relpath(SIRA, REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
