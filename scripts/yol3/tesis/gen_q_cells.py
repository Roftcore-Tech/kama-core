#!/usr/bin/env python3
"""[YOL3 · QK] G2 gösterim sahnesinin quinn kolları: (kol) başına bir hücre dosyası. Koşu YAPMAZ.

    python3 -I scripts/yol3/tesis/gen_q_cells.py            # cells-q/ + SIRA-Q.txt yazar
    python3 -I scripts/yol3/tesis/gen_q_cells.py --check    # yazmaz; 3 dosya / 30 iş + bayt eşitliği
    python3 -I scripts/yol3/tesis/gen_q_cells.py --komut    # orkestratörün koşu komutu

Sahne: gösterim analoğu `a_r12k_d150_l20_foto150k` (Y10; gen_cells.py `ANALOG["y10"]`). Geometri
(FWD/REV, BUDGET_S, CAPTURE, SETTLE_S) ve `F0A_ARGS` kaynak hücreden AYNEN alınır; Kama kollarının
`--e1-arm`/`--fec-off` ekleri quinn'de anlamsızdır ve verilmez (ikili bunları reddeder). Fark yalnız
CELL, REPS, ARMS ve kol bayrağı `--q-kol`. Kaynak dosyanın sha256'sı başlığa yazılır.

Kollar (bench `f0-quinn --q-kol`; tanım crates/bench/src/f0q.rs modül belgesi):
    Q_bbr       BBR, ilk pencere 240 000 B (quinn BBR varsayılanı, mevcut bench tanımı)
    Q_bbr_rfc   BBR, ilk pencere RFC 9002 §7.2 min(10·MSS, max(2·MSS, 14720)), MSS 1452 → 14 520 B
    Q_cubic     Cubic, ilk pencere 12 000 B (quinn varsayılanı)
Ortak: quinn 0.11.11 / quinn-proto 0.11.18, mesaj başına uni-stream, öncelik yok, idle 300 s,
initial_rtt 333 ms, initial_mtu 1452 (PMTUD, GSO kapalı), TLS 1.3. El sıkışma netem İÇİNDE; t0 =
bağlantı sonrası (Kama kollarında zarflar Unix soketinden, netem dışı; t0 aynı kuralla).

Hücre adları `<sahne>__<kol>`: G2 Kama hücreleriyle çakışmaz; `--komut` yeni bir köke Kama analog
hücrelerini bağlar (oku.py Kama ve quinn kollarını aynı sahne tablosunda okur). İkili: BIN_QK =
<yol3/quinn-kol ağacının hedef dizini>/release/bench (orkestratör derler, D-34).
"""

import argparse
import os
import sys

sys.dont_write_bytecode = True  # gen_cells içe aktarılır; depoya __pycache__ yazılmaz
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gen_cells  # noqa: E402  (read_cell, ANALOG, REPS, REPO: tek kaynak)

CELLS_DIR = os.path.join(HERE, "cells-q")
SIRA = os.path.join(HERE, "SIRA-Q.txt")
REPS = gen_cells.REPS
Q_ARMS = ["Q_bbr", "Q_bbr_rfc", "Q_cubic"]
DESC = {
    "Q_bbr": "BBR, IW 240 000 B (quinn BBR varsayılanı 200×1200; mevcut bench tanımı)",
    "Q_bbr_rfc": "BBR, IW 14 520 B (RFC 9002 §7.2, MSS = initial_mtu 1452)",
    "Q_cubic": "Cubic, IW 12 000 B (quinn varsayılanı)",
}
COPY_KEYS = ["FWD", "REV", "BUDGET_S", "CAPTURE", "SETTLE_S", "F0A_ARGS"]
ANALOG_BASE = os.path.basename(gen_cells.ANALOG["y10"])[: -len(".cell")]


def body(arm, rel, sha, lines):
    for k in COPY_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    f0a = lines["F0A_ARGS"]
    if not f0a.startswith("F0A_ARGS='") or not f0a.endswith("'") or f0a.count("'") != 2:
        raise SystemExit(f"{rel}: F0A_ARGS biçimi beklenen gibi değil")
    for flag in ("--e1-arm", "--fec-off", "--e1-edar", "--q-kol"):
        if flag in f0a.split():
            raise SystemExit(f"{rel}: kaynak hücrede {flag} var")
    base = os.path.basename(rel)[: -len(".cell")]
    cell = f"{base}__{arm}"
    return cell, "\n".join([
        f"# [YOL3 · QK] {cell} — quinn kolu, G2 gösterim sahnesi. Üretici: "
        "scripts/yol3/tesis/gen_q_cells.py (elle düzenlemeyin).",
        f"# Kaynak hücre: {rel} (sha256 {sha}) — geometri ve F0A_ARGS AYNEN; fark yalnız CELL, "
        "REPS, ARMS, kol bayrağı.",
        f"# Kol {arm}: {DESC[arm]} · quinn 0.11.11/0.11.18, mesaj başına uni-stream, öncelik yok.",
        "# İkili: BIN_QK = <yol3/quinn-kol hedefi>/release/bench (orkestratör, D-34). El sıkışma "
        "netem içinde, t0 = bağlantı sonrası (Kama: zarflar Unix soketinden, netem dışı).",
        f"CELL={cell}",
        lines["FWD"],
        lines["REV"],
        f'REPS="${{F0_REPS:-{REPS}}}"',
        f'ARMS="{arm}"',
        lines["BUDGET_S"],
        lines["CAPTURE"],
        lines["SETTLE_S"],
        f0a,
        f'ARM_{arm}_SRV="@BIN_QK@ f0-quinn --role bob $F0A_ARGS --q-kol {arm}"',
        f'ARM_{arm}_CLI="@BIN_QK@ f0-quinn --role alice $F0A_ARGS --q-kol {arm}"',
        "",
    ])


def plan():
    rel = gen_cells.ANALOG["y10"]
    lines, sha = gen_cells.read_cell(rel)
    return [(f"{cell}.cell", text) for cell, text in (body(arm, rel, sha, lines) for arm in Q_ARMS)]


def sira_text(items):
    return "".join(f"scripts/yol3/tesis/cells-q/{name}\n" for name, _ in items)


def komut(items):
    return f"""\
# [YOL3 · QK] quinn kolu koşusu — orkestratör koşar (D-18); depo kökü = yol3/quinn-kol ağacı.
# İkili (orkestratör derler, D-34):
#   KAMA_AGENT=ORK CARGO_TARGET_DIR=<hedef> scripts/ci/cargo-lock.sh build --release --locked -p bench --bins
export BIN_QK=<hedef>/release/bench
# Yeni kök; G2 teşhis kökündeki Kama analog hücreleri (yalnız dizinler) BAĞLANIR, kopyalanmaz ve
# G2 köküne yazılmaz. oku.py Kama ve quinn kollarını aynı sahne tablosunda okur.
Q=~/kama-k4g-work/yol3/quinn/<tarih>; mkdir -p "$Q"
for c in ~/kama-k4g-work/yol3/tesis/2026-11-09-g2/{ANALOG_BASE}__K*; do [ -d "$c" ] && ln -s "$c" "$Q/"; done
# {len(items)} hücre × {REPS} tekrar = {len(items) * REPS} iş:
systemd-inhibit --what=sleep:idle --why="YOL3 quinn kolu" setsid \\
  scripts/ktp2/f0/f0_grid.sh -j 3 "$Q" $(cat scripts/yol3/tesis/SIRA-Q.txt)
python3 -I scripts/yol3/tesis/oku.py --kok "$Q" --manifest docs/design/E1-TESHIS-G2.md
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--komut", action="store_true")
    a = ap.parse_args()
    items = plan()
    if a.komut:
        print(komut(items), end="")
        return 0
    if a.check:
        errs = []
        want = {name for name, _ in items}
        have = set(os.listdir(CELLS_DIR)) if os.path.isdir(CELLS_DIR) else set()
        for extra in sorted(have - want):
            errs.append(f"fazla dosya: cells-q/{extra}")
        for name, text in items:
            try:
                with open(os.path.join(CELLS_DIR, name), encoding="utf-8") as f:
                    if f.read() != text:
                        errs.append(f"cells-q/{name}: üretilenle aynı değil")
            except OSError as e:
                errs.append(f"cells-q/{name}: {e}")
        try:
            with open(SIRA, encoding="utf-8") as f:
                if f.read() != sira_text(items):
                    errs.append("SIRA-Q.txt: üretilenle aynı değil")
        except OSError as e:
            errs.append(f"SIRA-Q.txt: {e}")
        for e in errs:
            print(f"FARK: {e}", file=sys.stderr)
        print(f"{len(items)} dosya · {len(items) * REPS} iş · "
              f"{'temiz' if not errs else f'{len(errs)} fark'}")
        return 1 if errs else 0
    os.makedirs(CELLS_DIR, exist_ok=True)
    want = {name for name, _ in items}
    for old in os.listdir(CELLS_DIR):
        if old.endswith(".cell") and old not in want:
            os.remove(os.path.join(CELLS_DIR, old))
    for name, text in items:
        with open(os.path.join(CELLS_DIR, name), "w", encoding="utf-8") as f:
            f.write(text)
    with open(SIRA, "w", encoding="utf-8") as f:
        f.write(sira_text(items))
    print(f"{len(items)} dosya, {len(items) * REPS} iş → "
          f"{os.path.relpath(CELLS_DIR, gen_cells.REPO)}; {os.path.relpath(SIRA, gen_cells.REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
