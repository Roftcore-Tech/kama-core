#!/usr/bin/env python3
"""[YOL3 · Ö] mini-Ö hücreleri (KK-Ö-11, EK-A §6 / KK-Ö-EKA-1): (hücre × kol) başına bir dosya. Koşu YAPMAZ.

    python3 -I scripts/yol3/mini-o/gen_cells.py            # cells/ + SIRA.txt yazar
    python3 -I scripts/yol3/mini-o/gen_cells.py --check    # yazmaz; 5 dosya / 50 iş + bayt eşitliği
    python3 -I scripts/yol3/mini-o/gen_cells.py --komut    # orkestratörün koşu komutu (§8.3)
    python3 -I scripts/yol3/mini-o/gen_cells.py --set v2 [--check|--komut]    # mini-Ö v2
    python3 -I scripts/yol3/mini-o/gen_cells.py --set abl [--check|--komut]   # ablasyon (hazır)

`--set v1` (varsayılan) bu belgenin aşağısındaki v1 düzenidir; çıktısı (cells/, SIRA.txt) v2/abl
eklenince bayt bayt aynı kalır (`--check`). v2 ve abl ONKAYIT-V2.md'nin hücreleridir:
    v2   cells-v2/ + SIRA-V2.txt. Üç A hücresi (v1 geometrisi aynen) × K1A6s (gösterim seti; f0_flows
         `--goster on`, `--e1-arm` YOK → E1/E2a bayrakları kama_node::goster'dan, başlık
         `yol3.flag_src = goster.rs`), N = 10 · b_r25_d300_l20_p65 × {K1A6, K0}, N = 20 ·
         b_r12k_d1000_l0_p1000 × {K1A6, K0}, N = 10 (bench `--e1-arm`, iki uçta aynı; bench'te şerit
         ve PTT yok). 7 dosya, 90 iş. `--playout-ms`/`--lane-w` gösterim değerinden (2000 / 5)
         farklıysa f0_flows'a açık bayrak olarak yazılır (o zaman T/W goster.rs'ten gelmez).
    abl  cells-abl/ + SIRA-ABL-r25.txt + SIRA-ABL-d1000.txt (SIRA-V2'de DEĞİL; yalnız ONKAYIT-V2 §6
         tetiklerse koşulur). R3 hücresi başına bırak-birini-çıkar: K1A6mA1 … K1A6mA5 (bench aynı
         adlı kol) + K1A6mA6 (bench `--e1-arm K1A` = K1A6 − a6_iw_floor) + K0 tabanı; N tetikleyen
         v2 hücresininki (r25 20, d1000 10). Kendi kökünde koşar (K0 tabanı aynı kök).

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
DEFAULT_PLAYOUT_MS = 2000  # kama-node goster.rs GOSTER_PLAYOUT_DELAY (v2: bayrak yazılmaz)
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


# ── v2 ve ablasyon (ONKAYIT-V2.md) ─────────────────────────────────────────────────────────────

D1000 = "scripts/ktp2/f0/cells/b/b_r12k_d1000_l0_p1000.cell"
CELLS_V2_DIR = os.path.join(HERE, "cells-v2")
CELLS_ABL_DIR = os.path.join(HERE, "cells-abl")
REL_CELLS_V2 = "scripts/yol3/mini-o/cells-v2"
REL_CELLS_ABL = "scripts/yol3/mini-o/cells-abl"
SIRA_V2 = os.path.join(HERE, "SIRA-V2.txt")
SIRA_ABL = {"r25": os.path.join(HERE, "SIRA-ABL-r25.txt"),
            "d1000": os.path.join(HERE, "SIRA-ABL-d1000.txt")}
# Gösterim seti (sahip kararı 2026-11-10 madde 3; gece K1A6s). Etiketler f0_flows başlığının
# `yol3.e1_set` / `yol3.e2a_set` değerleriyle aynı (oku.py v2 bunları başlıkta denetler).
V2_A_KOL = "K1A6s"          # A hücresi kol etiketi: f0_flows --goster on (bayraklar goster.rs'ten)
V2_B_KOL = "K1A6"           # b hücresi: bench --e1-arm K1A6 (kama-node GOSTER_KOL)
V2_TABAN = "K0"
GOSTER_E1_LABEL = "c+d+DrainRtprop+e-dar+a3_burst_floor+a4_probe_srtt+a6_iw_floor"
GOSTER_E2A_LABEL = ("tt_rfc(g=1ms)+rtt_rule_v11+iw_idle_hold+pto_probes=2+gap_ack+"
                    "return_prior_gated+pto_rfc")
V2_REPS = {"a": 10, "r25": 20, "d1000": 10}
# Ablasyon kolları: (hücre kol adı, bench --e1-arm, çıkan A bayrağı).
ABL_ARMS = [("K1A6mA1", "K1A6mA1", "A-1 gap_ack"),
            ("K1A6mA2", "K1A6mA2", "A-2 return_prior_gated"),
            ("K1A6mA3", "K1A6mA3", "A-3 a3_burst_floor"),
            ("K1A6mA4", "K1A6mA4", "A-4 a4_probe_srtt"),
            ("K1A6mA5", "K1A6mA5", "A-5 pto_rfc (pto_probes = 2 kalır)"),
            ("K1A6mA6", "K1A", "A-6 a6_iw_floor (= K1A)")]
ABL_CELLS = {"r25": R25, "d1000": D1000}
V2_LABEL = ("gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel; "
            "sığ tamponlu ağ gösterim kapsamı dışı")


def v2_table():
    """[(kaynak, kol etiketi, bench kolu | None, REPS, amaç)] öncelik sırasıyla (SIRA-V2 sırası)."""
    a, r, d = V2_REPS["a"], V2_REPS["r25"], V2_REPS["d1000"]
    return [
        (Y10, V2_A_KOL, None, a, "gösterim analoğu (Y10): bağlayıcı dört ölçüt, T ve W teyidi"),
        (R25, V2_B_KOL, V2_B_KOL, r, "R3-r25: gösterim seti (N = 20, sahip madde 4)"),
        (R25, V2_TABAN, V2_TABAN, r, "R3-r25: taban K0 (aynı ikili, aynı kök, N = 20)"),
        (D1000, V2_B_KOL, V2_B_KOL, d, "R3-d1000 l0: gösterim seti (sahip madde 3)"),
        (D1000, V2_TABAN, V2_TABAN, d, "R3-d1000 l0: taban K0 (aynı ikili, aynı kök)"),
        (L30, V2_A_KOL, None, a, "G-K8 kötü ağ vekili: B1, B2, B5 raporlanan"),
        (L0, V2_A_KOL, None, a, "G-K8 iyi ağ vekili: B1, B2, B5 raporlanan"),
    ]


def abl_table(which):
    """[(kaynak, kol etiketi, bench kolu, REPS, amaç)]: K0 tabanı + altı ablasyon kolu."""
    rel = ABL_CELLS[which]
    n = V2_REPS[which]
    rows = [(rel, V2_TABAN, V2_TABAN, n, f"ablasyon {which}: taban K0 (aynı kök)")]
    for arm, bench, cikan in ABL_ARMS:
        rows.append((rel, arm, bench, n, f"ablasyon {which}: gösterim seti − {cikan}"))
    return rows


def header_v(cell, purpose, rel, sha, uretici):
    return [
        f"# [YOL3 · Ö · mini-Ö v2] {cell} — {purpose}. Üretici: scripts/yol3/mini-o/gen_cells.py "
        f"{uretici} (elle düzenlemeyin). Ön kayıt: scripts/yol3/mini-o/ONKAYIT-V2.md.",
        f"# Kaynak hücre: {rel} (sha256 {sha}) — geometri AYNEN; fark yalnız CELL, REPS, ARMS, "
        "kol ve gösterim bayrakları.",
    ]


def a_body_v2(cell, arm, rel, sha, lines, purpose, reps, playout_ms, lane_w):
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
    extra = ["--goster on"]
    if playout_ms != DEFAULT_PLAYOUT_MS:
        extra.append(f"--playout-ms {playout_ms}")
    if lane_w != DEFAULT_W:
        extra.append(f"--lane-w {lane_w}")
    f0a = m.group(1) + " " + " ".join(extra)
    src = ("goster.rs" if playout_ms == DEFAULT_PLAYOUT_MS and lane_w == DEFAULT_W
           else "goster.rs (E1/E2a/FEC) + açık bayrak (T/W)")
    return "\n".join(header_v(cell, purpose, rel, sha, "--set v2") + [
        f"# Kol {arm} = gösterim seti (sahip kararı 2026-11-10 madde 3; gece K1A6s): f0_flows "
        "--goster on, --e1-arm YOK → E1/E2a bayrakları kama_node::goster'dan (başlık "
        "yol3.flag_src = goster.rs, yol3.e1_arm = K1A6).",
        f"# E1 {GOSTER_E1_LABEL} · E2a {GOSTER_E2A_LABEL} · FEC off · "
        f"{goster_text(playout_ms, lane_w)} · kaynak {src}.",
        "# İkili: BIN_GOSTER = <yol2/goster>/target/release/examples/f0_flows (orkestratör, D-34; "
        "goster.rs'i E2a ile kuran yeni ikili). İki ayrı süreç; zarflar Unix soketinden (netem "
        "dışı, beyanlı).",
        f"# Etiket: {V2_LABEL}.",
        f"CELL={cell}",
        lines["FWD"],
        lines["REV"],
        f'REPS="${{F0_REPS:-{reps}}}"',
        f'ARMS="{arm}"',
        lines["BUDGET_S"],
        lines["CAPTURE"],
        lines["SETTLE_S"],
        f"F0A_ARGS='{f0a}'",
        f'ARM_{arm}_SRV="@BIN_GOSTER@ --role bob $F0A_ARGS"',
        f'ARM_{arm}_CLI="@BIN_GOSTER@ --role alice $F0A_ARGS"',
        "",
    ])


def b_body_v(cell, arm, bench, rel, sha, lines, purpose, reps, uretici):
    for k in B_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    if bench == V2_TABAN:
        kol = f"# Kol {arm} = taban (bench --e1-arm K0: E1/E2a bayrakları kapalı, ürün değeri)"
    elif bench == V2_B_KOL:
        kol = (f"# Kol {arm} = gösterim seti (bench --e1-arm {bench}, iki uçta aynı): E1 "
               f"{GOSTER_E1_LABEL} · E2a {GOSTER_E2A_LABEL}")
    else:
        kol = (f"# Kol {arm} = ablasyon (bench --e1-arm {bench}, iki uçta aynı): gösterim seti "
               "K1A6'dan TEK bayrak çıkmış (bench e1arm.rs)")
    out = header_v(cell, purpose, rel, sha, uretici) + [
        kol + " · FEC off (--fec-profile 0, v4 Y1).",
        "# bench'te S1 şeridi ve PTT oynatma tamponu YOK (beyanlı): bu satır yalnız motor "
        "bayraklarını (E1 + E2a) ölçer; gösterim setinin şerit/tampon kısmı A hücrelerinde.",
        "# İkili: BIN_GOSTER_BENCH = <yol2/goster>/target/release/bench (orkestratör, D-34); kol "
        "arms_kama.sh kama_e1_arm.",
        f"# Etiket: {V2_LABEL}.",
        f"CELL={cell}",
    ]
    out += [lines[k] for k in B_KEYS if k not in ("BUDGET_S", "CAPTURE", "SETTLE_S")]
    out += [
        f'REPS="${{F0_REPS:-{reps}}}"',
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
        f"kama_e1_arm {arm} {bench} @BIN_GOSTER_BENCH@",
        "b_cap_wrap",
        "",
    ]
    return "\n".join(out)


def plan_v2(playout_ms, lane_w):
    items = []
    for rel, arm, bench, reps, purpose in v2_table():
        lines, sha = read_cell(rel)
        base = os.path.basename(rel)[: -len(".cell")]
        cell = f"{base}__{arm}"
        if base.startswith("a_"):
            body = a_body_v2(cell, arm, rel, sha, lines, purpose, reps, playout_ms, lane_w)
        else:
            body = b_body_v(cell, arm, bench, rel, sha, lines, purpose, reps, "--set v2")
        items.append((f"{cell}.cell", body, reps))
    return items


def plan_abl():
    """{hangi: [(dosya adı, gövde, REPS)]}."""
    out = {}
    for which in ("r25", "d1000"):
        items = []
        for rel, arm, bench, reps, purpose in abl_table(which):
            lines, sha = read_cell(rel)
            base = os.path.basename(rel)[: -len(".cell")]
            cell = f"{base}__{arm}"
            items.append((f"{cell}.cell",
                          b_body_v(cell, arm, bench, rel, sha, lines, purpose, reps, "--set abl"),
                          reps))
        out[which] = items
    return out


def sira_v(rel_dir, items):
    return "".join(f"{rel_dir}/{name}\n" for name, _, _ in items)


def komut_v2(items, playout_ms, lane_w):
    jobs = sum(r for _, _, r in items)
    return f"""\
# [YOL3 · Ö] mini-Ö v2 koşusu (ONKAYIT-V2.md) — orkestratör koşar (D-18); ajan ve cargo durur.
# Depo kökünden. İkililer yol2/goster release (orkestratör derler, D-34); SHA koşu kaydına.
# Kayıt: SIRA-V2.txt sha256, ONKAYIT-V2.md sha256, ikili sha256'ları; gösterim seti K1A6s (A) /
#   K1A6 (b) · taban K0 · T {playout_ms} ms · W {lane_w} · FEC off. {len(items)} dosya, {jobs} iş.
# Süre tahmini [türetilmiş]: -j 11 ile yedi dosya paralel; en uzun dosyalar r25 (20 koşu × 2–2,5 dk)
#   ve d300 l30 (10 koşu × 2,5–6 dk) ≈ 45–60 dk duvar süresi.
export BIN_GOSTER=<yol2/goster ağacı>/target/release/examples/f0_flows
export BIN_GOSTER_BENCH=<yol2/goster ağacı>/target/release/bench
systemd-inhibit --what=sleep:idle --why="YOL3 mini-O v2" setsid \\
  scripts/ktp2/f0/f0_grid.sh -j 11 <ölçüm-kökü>/yol3/mini-o-v2/<tarih> $(cat scripts/yol3/mini-o/SIRA-V2.txt)
# Okuma (koşu bitince; ön kayıt sha256'sı denetlenir, farklıysa okuma yok):
python3 -I scripts/yol3/mini-o/oku.py --surum v2 --kok <ölçüm-kökü>/yol3/mini-o-v2/<tarih> \\
  --onkayit scripts/yol3/mini-o/ONKAYIT-V2.md   # çıktı <kök>/okuma/{{OKUMA.md,okuma.json}}
"""


def komut_abl(abl):
    out = ["# [YOL3 · Ö] mini-Ö v2 ablasyonu (ONKAYIT-V2.md §6) — YALNIZ v2 okuması tetik verirse.",
           "# Her R3 hücresi kendi kökünde (K0 tabanı aynı kök ve aynı ikili). İkililer v2'ninki.",
           "export BIN_GOSTER_BENCH=<yol2/goster ağacı>/target/release/bench"]
    for which, items in abl.items():
        jobs = sum(r for _, _, r in items)
        out += [f"# {which}: {len(items)} dosya, {jobs} iş.",
                f'systemd-inhibit --what=sleep:idle --why="YOL3 mini-O abl {which}" setsid \\',
                f"  scripts/ktp2/f0/f0_grid.sh -j 11 <ölçüm-kökü>/yol3/mini-o-abl-{which}/<tarih> "
                f"$(cat scripts/yol3/mini-o/SIRA-ABL-{which}.txt)",
                f"python3 -I scripts/yol3/mini-o/oku.py --surum abl --hucre {which} --kok "
                f"<ölçüm-kökü>/yol3/mini-o-abl-{which}/<tarih> --onkayit "
                "scripts/yol3/mini-o/ONKAYIT-V2.md"]
    return "\n".join(out) + "\n"


def write_or_check(cells_dir, rel_dir, items, siras, check):
    """items: [(ad, gövde, REPS)]; siras: [(SIRA yolu, metin)]. Döner: fark listesi."""
    errs = []
    want = {name for name, _, _ in items}
    if check:
        have = set(os.listdir(cells_dir)) if os.path.isdir(cells_dir) else set()
        for extra in sorted(have - want):
            errs.append(f"fazla dosya: {rel_dir}/{extra}")
        for name, body, _ in items:
            try:
                with open(os.path.join(cells_dir, name), encoding="utf-8") as f:
                    if f.read() != body:
                        errs.append(f"{rel_dir}/{name}: üretilenle aynı değil")
            except OSError as e:
                errs.append(f"{rel_dir}/{name}: {e}")
        for path, text in siras:
            try:
                with open(path, encoding="utf-8") as f:
                    if f.read() != text:
                        errs.append(f"{os.path.basename(path)}: üretilenle aynı değil")
            except OSError as e:
                errs.append(f"{os.path.basename(path)}: {e}")
        return errs
    os.makedirs(cells_dir, exist_ok=True)
    for old in sorted(set(os.listdir(cells_dir)) - want):
        if old.endswith(".cell"):
            os.remove(os.path.join(cells_dir, old))
    for name, body, _ in items:
        with open(os.path.join(cells_dir, name), "w", encoding="utf-8") as f:
            f.write(body)
    for path, text in siras:
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)
    return errs


def main_v(a):
    if a.set == "v2":
        if a.kol != "K1" or a.taban != "K0":
            raise SystemExit("--set v2: kol ve taban sabit (ONKAYIT-V2 §1); --kol/--taban verilmez")
        items = plan_v2(a.playout_ms, a.lane_w)
        if a.komut:
            print(komut_v2(items, a.playout_ms, a.lane_w), end="")
            return 0
        siras = [(SIRA_V2, sira_v(REL_CELLS_V2, items))]
        groups = [(CELLS_V2_DIR, REL_CELLS_V2, items, siras)]
        desc = f"v2: A {V2_A_KOL} · b {V2_B_KOL} · taban {V2_TABAN} · T={a.playout_ms}ms W={a.lane_w}"
    else:
        if (a.kol, a.taban, a.playout_ms, a.lane_w) != ("K1", "K0", DEFAULT_PLAYOUT_MS, DEFAULT_W):
            raise SystemExit("--set abl: seçenek almaz (ONKAYIT-V2 §6)")
        abl = plan_abl()
        if a.komut:
            print(komut_abl(abl), end="")
            return 0
        items = abl["r25"] + abl["d1000"]
        siras = [(SIRA_ABL[w], sira_v(REL_CELLS_ABL, abl[w])) for w in ("r25", "d1000")]
        groups = [(CELLS_ABL_DIR, REL_CELLS_ABL, items, siras)]
        desc = "abl: r25 × 7 kol (N = 20) + d1000 × 7 kol (N = 10)"
    errs = []
    for cells_dir, rel_dir, its, sr in groups:
        errs += write_or_check(cells_dir, rel_dir, its, sr, a.check)
    jobs = sum(r for _, _, r in items)
    if a.check:
        for e in errs:
            print(f"FARK: {e}", file=sys.stderr)
        print(f"{len(items)} dosya · {jobs} iş · {desc} · "
              f"{'temiz' if not errs else f'{len(errs)} fark'}")
        return 1 if errs else 0
    print(f"{len(items)} dosya · {jobs} iş · {desc} → "
          + ", ".join(os.path.relpath(p, REPO) for p, _ in groups[0][3]))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kol", default="K1")
    ap.add_argument("--taban", default="K0")
    ap.add_argument("--playout-ms", type=int, default=2000)
    ap.add_argument("--lane-w", type=int, default=DEFAULT_W)
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--komut", action="store_true")
    ap.add_argument("--set", choices=("v1", "v2", "abl"), default="v1")
    a = ap.parse_args()
    for name in (a.kol, a.taban):
        if not ARM_NAME.match(name):
            raise SystemExit(f"geçersiz kol adı: {name!r} (yalnız [A-Za-z0-9_])")
    if not PLAYOUT_MIN_MS <= a.playout_ms <= PLAYOUT_MAX_MS:
        raise SystemExit(f"--playout-ms {a.playout_ms} ∉ [{PLAYOUT_MIN_MS}, {PLAYOUT_MAX_MS}]")
    if a.lane_w < 1:
        raise SystemExit("--lane-w ≥ 1 olmalı")
    if a.set != "v1":
        return main_v(a)
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
