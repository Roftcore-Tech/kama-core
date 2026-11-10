#!/usr/bin/env python3
"""[YOL3 · Ö] E1 teşhis hücreleri (CC §4.2; KK-Ö-06): (hücre × kol) başına bir dosya. Koşu YAPMAZ.

    python3 -I scripts/yol3/tesis/gen_cells.py            # cells/ + SIRA.txt yazar
    python3 -I scripts/yol3/tesis/gen_cells.py --check    # yazmaz; 16 dosya / 160 iş + bayt eşitliği
    python3 -I scripts/yol3/tesis/gen_cells.py --komut    # orkestratörün koşu komutu (§8.2)
    python3 -I scripts/yol3/tesis/gen_cells.py --set e2a [--check|--komut]   # E2a kıyası
    python3 -I scripts/yol3/tesis/gen_cells.py --set goster [--check|--komut]  # şeritli gösterim
    python3 -I scripts/yol3/tesis/gen_cells.py --set a [--check|--komut]       # A merdiveni
    python3 -I scripts/yol3/tesis/gen_cells.py --set a6 [--check|--komut]      # A-6 gece ölçümü

Seçenekler (M2 manifestinin satırları; varsayılanlar aşağıda, değişirse dosyalar yeniden üretilir):
    --analog y10|l30   gösterim analoğu: `a_r12k_d150_l20_foto150k` (Y10, varsayılan) ya da
                       `a_r12k_d300_l30_dosyali` (CC §4.2 "Y10 hazırsa, yoksa …")
    --fec off|urun     bütün kollarda FEC durumu: `off` (varsayılan; KK-ORK-01: teşhisin bütün
                       kollarında FEC KAPALI — gösterim düğümü `runtime.fec = None`, v4 Y1, etiket
                       "ARQ yolu"; f0_flows `--fec-off on`, bench `--fec-profile 0`) ya da `urun`
                       (ürün profili, FEC açık)
    --edar-yok         ön test geçmediyse (CC §4.2): her kolda `--e1-edar off` (e1_probe_keep düşer);
                       K2−e K2 ile aynı sete indiği için çıkar (15 dosya, 150 iş)
    --set g2|e2a       `g2` (varsayılan): G2 teşhis seti → cells/ + SIRA.txt (yukarıdaki tablo).
                       `e2a`: E2a kıyası → cells-e2a/ + SIRA-E2A.txt; gösterim analoğu × {K1, K1a,
                       K0a} (3 dosya, 30 iş). K1a = K1 + E2a, K0a = yalnız E2a; E2a = M1 `E2aFlags`
                       tt_rfc (g = 1 ms) + rtt_rule_v11 + iw_idle_hold + pto_probes = 2. Tek
                       değişken K1 ↔ K1a; ikili yol2/o1-e2a (iki kol AYNI ikiliden).
                       `goster`: şeritli gösterim → cells-goster/ + SIRA-GOSTER.txt; gösterim
                       analoğu × {K1, K1a, K0}, her kolda `--goster on` (YOL3-T1 S1 şeritleri
                       W = 5 + PTT oynatma tamponu T = 2,0 s; f0_flows). 3 dosya, 30 iş; ikili
                       yol2/goster. Yalnız A hücresi (bench'te şerit yok).
                       `a`: A paketi merdiveni (Q1 + Q2) → cells-a/ + SIRA-A.txt; gösterim analoğu
                       × {K0, K1, K1a, K1g, K1gr, K1grp, K1A}, `--goster on`, FEC kapalı (7 dosya,
                       70 iş); ikili yol2/goster-a. K1g = K1 + gap_ack · K1gr = K1g +
                       return_prior_gated · K1grp = K1gr + pto_rfc + pto_probes = 2 · K1A = K1 +
                       E2a'nın hepsi + a3_burst_floor + a4_probe_srtt.
                       `a6`: A-6 gece ölçümü (sahip kararı v4 EK-A §9.3, §10.3, §10.4, §2.1;
                       KK-Ö-EKA-2…4) → cells-a6/ + SIRA-A6.txt; 23 dosya, 230 iş; ikililer
                       yol2/goster-a6 (f0_flows, bench) ve yol3/quinn-kol (bench f0-quinn).
                       Kollar (A6_ARMS): K1 (şeritsiz) · K1A6 = K1A + a6_iw_floor (K1 + A-1…A-6) ·
                       K1A (K1 + A − 6) · K1s = K1 + `--goster on` · K1A6s = K1A6 + `--goster on`
                       · Q_bbr_rfc · Q_cubic (QK kolları, gen_q_cells.py tanımı). Hücreler (sıra =
                       iş emri maddesi; SIRA-A6 dizilişi 1, 2, 5, 3, 4 — kısa b hücreleri sonda
                       boş yuvaları doldurur):
                         1  a_r12k_d150_l20_foto150k × 7 kol
                         2  a_r12k_d300_l30_dosyali × 7 kol
                         5  q_limit: 1 ve 2 × {K1A6s, Q_bbr_rfc}, kol adı `<kol>_qlim`; netem
                            dizesine iki yönde `limit N`, N = floor(2 × rate × RTT / 1200 B)
                            (RTT = iki yön gecikme toplamı), hesabı hücre başlığında
                         3  b_r25_d300_l20_p65 × {K1, K1A6, K1A} (bench)
                         4  b_r12k_d1000_l0_p1000 × {K1, K1A6} (bench; R3 koruması)
                       Yazılmayanlar (nedenleri `--komut` çıktısında da): b hücrelerinde s-kolları
                       (bench'te S1 şeridi ve PTT yok; b_body `--goster`'i reddeder); b_r25'te
                       quinn (QUIC ilk uçuşu her yönde ≥ 1200 B UDP yükü, RFC 9000 §14.1 → 25 B/s'de
                       el sıkışma ≥ 99,4 s + 2d > D = 60 s, reports/ktp2/f0/b/RAPOR-B.md hesabı;
                       ayrıca Q_* tanımı f0-quinn akış senaryosudur, bench B'nin quinn kolu ayrı
                       tanım); b_r25'te q_limit (N = floor(2 × 25 × 0,6 / 1200) = 0 < 1 paket).

Geometri (FWD/REV, zaman çizelgesi, bütçe) kaynak F0/YOL3 hücresinin AYNISI: kaynak dosya
ayrıştırılır, yalnız CELL, REPS, ARMS ve kol bayrağı değişir. Kaynak dosyanın sha256'sı başlığa
yazılır (kaynak değişirse `--check` fark verir). Kol → bayrak eşlemesi M2'nin teşhis manifestindeki
tablonun aynısıdır (docs/design/E1-TESHIS-G2.md, KK-M2-09); burada yalnız başlık açıklamasıdır —
bayrakları ikili `--e1-arm` değerinden kurar (bench `kama-product` ve f0_flows, iki uçta aynı).

İkililer (orkestratör derler, D-34): BIN_TESIS = <yol2/o1>/target/release/examples/f0_flows,
BIN_TESIS_BENCH = <yol2/o1>/target/release/bench. `a6`: aynı ikisi yol2/goster-a6 ağacından +
BIN_QK = <yol3/quinn-kol hedefi>/release/bench (quinn kolları; önceki quinn koşusunun değişkeni).
"""

import argparse
import hashlib
import os
import re
import shlex
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", "..", ".."))

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
# [E2a kıyası] K1a = K1'in E1 seti + E2a; K0a = yalnız E2a. E2a seti iki kolda aynı (M1 E2aFlags).
ARM_FLAGS.update({"K1a": ARM_FLAGS["K1"], "K0a": []})
E2A_TEXT = "tt_rfc(g=1ms)+rtt_rule_v11+iw_idle_hold+pto_probes=2"
# [A merdiveni] Q1 (E2aFlags) + Q2 (E1Flags) kolları; E1 tabanı K1, K1A'da Q2'nin iki bayrağı.
ARM_FLAGS.update({"K1g": ARM_FLAGS["K1"], "K1gr": ARM_FLAGS["K1"], "K1grp": ARM_FLAGS["K1"],
                  "K1A": ARM_FLAGS["K1"] + ["a3_burst_floor", "a4_probe_srtt"]})
# Kol → E2a etiketi (ikilinin `e2a_set` etiketiyle aynı sıra).
E2A_BY_ARM = {
    "K1a": E2A_TEXT,
    "K0a": E2A_TEXT,
    "K1g": "gap_ack",
    "K1gr": "gap_ack+return_prior_gated",
    "K1grp": "pto_probes=2+gap_ack+return_prior_gated+pto_rfc",
    "K1A": E2A_TEXT + "+gap_ack+return_prior_gated+pto_rfc",
}
E2A_ARMS = list(E2A_BY_ARM)
# [A-6 gece] K1A6 = K1A + Q2 A-6 `a6_iw_floor` (E1Flags; a3/a4 ile aynı yapı). E2a seti K1A'nınki.
ARM_FLAGS["K1A6"] = ARM_FLAGS["K1A"] + ["a6_iw_floor"]
E2A_BY_ARM["K1A6"] = E2A_BY_ARM["K1A"]

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

# [E2a kıyası] Gösterim hücresi (analog) × {K1, K1a, K0a}; ilk iki satır = tek değişkenli çift.
TABLE_E2A = [
    (1, "ANALOG", ["K1", "K1a", "K0a"], "E2a kıyası: K1 ↔ K1a tek değişken, K0a yalnız E2a"),
]

# [Şeritli gösterim] Gösterim hücresi × {K1, K1a, K0}, `--goster on`; ilk iki satır K1 ↔ K1a.
TABLE_GOSTER = [
    (1, "ANALOG", ["K1", "K1a", "K0"], "şeritli gösterim: K1 ↔ K1a, K0 (--goster on)"),
]

# [A merdiveni] Gösterim hücresi × 7 kol, `--goster on`; K1 taban, oranlar oku.py'de K1'e.
TABLE_A = [
    (1, "ANALOG", ["K0", "K1", "K1a", "K1g", "K1gr", "K1grp", "K1A"],
     "A paketi merdiveni (Q1 + Q2; --goster on)"),
]

# Set → (tablo, hücre dizini, sıra dosyası, ağaç, koşu kökü).
SETS = {
    "g2": (TABLE, "cells", "SIRA.txt", "yol2/o1", "2026-11-11"),
    "e2a": (TABLE_E2A, "cells-e2a", "SIRA-E2A.txt", "yol2/o1-e2a", "2026-11-09-e2a"),
    "goster": (TABLE_GOSTER, "cells-goster", "SIRA-GOSTER.txt", "yol2/goster", "2026-11-09-goster"),
    "a": (TABLE_A, "cells-a", "SIRA-A.txt", "yol2/goster-a", "2026-11-09-a"),
}

# [A-6 gece] Kol → (tür, değer, goster). "kama": f0_flows `--e1-arm <değer>` (+ `--goster on`);
# "quinn": bench `f0-quinn --q-kol <değer>` (yol3/quinn-kol, crates/bench/src/f0q.rs). Ad, ARMS'taki
# ad (f0_cell.sh `ARM_<ad>_SRV`); `--e1-arm` değeri addan ayrıdır (s-kollarında aynı bayrak seti).
A6_ARMS = {
    "K1": ("kama", "K1", False),
    "K1A6": ("kama", "K1A6", False),
    "K1A": ("kama", "K1A", False),
    "K1s": ("kama", "K1", True),
    "K1A6s": ("kama", "K1A6", True),
    "Q_bbr_rfc": ("quinn", "Q_bbr_rfc", False),
    "Q_cubic": ("quinn", "Q_cubic", False),
}
A6_SEVEN = list(A6_ARMS)
# Küçük kuyruk kolu (EK-A §10.3): ayrı kol, ad `<kol>_qlim`, netem dizesine `limit N`.
QLIM = "_qlim"
A6_QLIM = [a + QLIM for a in ("K1A6s", "Q_bbr_rfc")]
# quinn kol açıklamaları (yol3/quinn-kol gen_q_cells.py DESC ile aynı metin).
Q_DESC = {
    "Q_bbr_rfc": "BBR, IW 14 520 B (RFC 9002 §7.2, MSS = initial_mtu 1452)",
    "Q_cubic": "Cubic, IW 12 000 B (quinn varsayılanı)",
}
# netem `limit` paket birimi (EK-A §9.4 "rate × RTT / MTU × 2"; MTU = 1200 B, iş emri).
QLIM_MTU = 1200
A6_C1 = "scripts/yol3/cells/a/a_r12k_d150_l20_foto150k.cell"
A6_C2 = "scripts/ktp2/f0/cells/a/a_r12k_d300_l30_dosyali.cell"
A6_C3 = "scripts/ktp2/f0/cells/b/b_r25_d300_l20_p65.cell"
A6_C4 = "scripts/ktp2/f0/cells/b/b_r12k_d1000_l0_p1000.cell"
# (madde, kaynak, kollar, amaç) — SIRA-A6 bu sırayla: 1, 2, 5, 3, 4 (en kısa b hücresi sonda).
TABLE_A6 = [
    (1, A6_C1, A6_SEVEN, "7 kol, gösterim analoğu (EK-A §9.3, §10.4)"),
    (2, A6_C2, A6_SEVEN, "7 kol, d300 l30 dosyalı (EK-A §9.3, §10.4)"),
    (5, A6_C1, A6_QLIM, "küçük kuyruk q_limit (EK-A §10.3)"),
    (5, A6_C2, A6_QLIM, "küçük kuyruk q_limit (EK-A §10.3)"),
    (3, A6_C3, ["K1", "K1A6", "K1A"], "r25 teslim@60 ve gönderim; R3 (EK-A §9.3, §2.1)"),
    (4, A6_C4, ["K1", "K1A6"], "R3 koruması d1000 l0 p95 (EK-A §2.1)"),
]
A6_TREE = "yol2/goster-a6"
A6_KOK = "2026-11-10-a6"
SETS["a6"] = (TABLE_A6, "cells-a6", "SIRA-A6.txt", A6_TREE, A6_KOK)
# `--goster on` taşıyan setler (f0_flows: S1 şeritleri + oynatma tamponu, iki uca aynı).
GOSTER_SETS = {"goster", "a"}

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


def ork(fec):
    return " (KK-ORK-01)" if fec == "off" else ""


def flags_text(arm, edar_yok):
    fl = [f for f in ARM_FLAGS[arm] if not (edar_yok and f == "e-dar")]
    return "+".join(fl) if fl else "hiçbiri (bayraklar kapalı)"


def e1_src(arm):
    """E1 bayraklarının kaynağı; Q2 bayrağı taşıyan kolda ek (diğer setler bayt bayt aynı)."""
    if "a6_iw_floor" in ARM_FLAGS[arm]:
        return "(M2 manifesti KK-M2-09; a3/a4/a6 Q2 E1Flags)"
    if any(f in ARM_FLAGS[arm] for f in ("a3_burst_floor", "a4_probe_srtt")):
        return "(M2 manifesti KK-M2-09; a3/a4 Q2 E1Flags)"
    return "(M2 manifesti KK-M2-09)"


def e2a_text(arm):
    """E2a kolunda başlık eki; diğer kollarda boş (G2 dosyaları bayt bayt aynı kalır)."""
    if arm not in E2A_BY_ARM:
        return ""
    src = "M1 E2aFlags" if arm in ("K1a", "K0a") else "M1+Q1 E2aFlags"
    return f" · E2a {E2A_BY_ARM[arm]} ({src})"


def a_body(cell, arm, rel, sha, lines, fec, edar_yok, purpose, row, tree, goster=False):
    for k in A_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    m = re.match(r"^F0A_ARGS='(.*)'$", lines["F0A_ARGS"])
    if not m or "'" in m.group(1):
        raise SystemExit(f"{rel}: F0A_ARGS biçimi beklenen gibi değil")
    args = m.group(1).split()
    for flag in ("--e1-arm", "--fec-off", "--e1-edar", "--goster", "--playout-ms"):
        if flag in args:
            raise SystemExit(f"{rel}: kaynak hücrede {flag} zaten var")
    extra = [f"--e1-arm {arm}"]
    if fec == "off":
        extra.append("--fec-off on")
    if edar_yok:
        extra.append("--e1-edar off")
    if goster:
        extra.append("--goster on")
    f0a = m.group(1) + " " + " ".join(extra)
    return "\n".join([
        f"# [YOL3 · Ö · teşhis] {cell} — sıra {row}; {purpose}. Üretici: "
        "scripts/yol3/tesis/gen_cells.py (elle düzenlemeyin).",
        f"# Kaynak hücre: {rel} (sha256 {sha}) — geometri AYNEN; fark yalnız CELL, REPS, ARMS, "
        "kol bayrağı.",
        f"# Kol {arm}: {flags_text(arm, edar_yok)} {e1_src(arm)}{e2a_text(arm)} · "
        f"FEC {fec}{ork(fec)}.",
        f"# İkili: BIN_TESIS = <{tree}>/target/release/examples/f0_flows (orkestratör, D-34). "
        "Zarflar Unix soketinden (netem dışı, beyanlı).",
    ] + ([
        "# Gösterim (YOL3-T1, --goster on): S1 şeritleri LaneConfig::default (W = 5) + PTT oynatma "
        "tamponu T = 2,0 s; oku.py PTT varış ve oynatma p95'ini ayrı yazar.",
    ] if goster else []) + [
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


def b_body(cell, arm, rel, sha, lines, fec, edar_yok, purpose, row, tree, goster=False,
           tag="[YOL3 · Ö · teşhis]", gen="scripts/yol3/tesis/gen_cells.py", sira="sıra"):
    if goster:
        raise SystemExit(f"{rel}: --goster yalnız A hücresinde (bench'te S1 şeridi yok)")
    for k in B_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    extra = []
    if fec == "off":
        extra.append("--fec-profile 0")
    if edar_yok:
        extra.append("--e1-edar off")
    out = [
        f"# {tag} {cell} — {sira} {row}; {purpose}. Üretici: "
        f"{gen} (elle düzenlemeyin).",
        f"# Kaynak hücre: {rel} (sha256 {sha}) — geometri AYNEN; fark yalnız CELL, REPS, ARMS, "
        "kol bayrağı.",
        f"# Kol {arm}: {flags_text(arm, edar_yok)} {e1_src(arm)}{e2a_text(arm)} · "
        f"FEC {fec}{ork(fec)}.",
        f"# İkili: BIN_TESIS_BENCH = <{tree}>/target/release/bench (orkestratör, D-34); kol "
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


# ── [A-6 gece] q_limit hesabı, Kama ve quinn kol gövdeleri ─────────────────────────────────────

NETEM_DELAY = re.compile(r"\bdelay (\d+)ms\b")
NETEM_RATE = re.compile(r"\brate (\d+)(bit|kbit|mbit)\b")
RATE_BITS = {"bit": 1, "kbit": 1000, "mbit": 1_000_000}


def tr_num(x, nd, strip=False):
    """Türkçe ondalık (virgül) ve binlik boşluk: 12 500 · 0,300; strip → sondaki sıfırlar düşer."""
    s = f"{x:,.{nd}f}"
    if strip and "." in s:
        s = s.rstrip("0").rstrip(".")
    return s.replace(",", " ").replace(".", ",")


def netem_str(lines, key, rel):
    m = re.match(rf'^{key}="(.*)"$', lines[key])
    if not m or '"' in m.group(1) or "$" in m.group(1):
        raise SystemExit(f"{rel}: {key} biçimi beklenen gibi değil")
    return m.group(1)


def qlim_calc(lines, rel):
    """EK-A §10.3: N = floor(2 × rate × RTT / 1200 B) paket; RTT = ileri + dönüş gecikmesi (jitter
    katılmaz), rate = netem `rate` (iki yön eşit olmalı). Tam sayı aritmetiği (bit, ms)."""
    fwd = netem_str(lines, "FWD", rel)
    rev = fwd if lines["REV"] == 'REV="$FWD"' else netem_str(lines, "REV", rel)
    vals = []
    for name, spec in (("FWD", fwd), ("REV", rev)):
        d, r = NETEM_DELAY.search(spec), NETEM_RATE.search(spec)
        if not d or not r or "limit" in spec.split():
            raise SystemExit(f"{rel}: {name} q_limit için delay/rate yok ya da limit zaten var")
        vals.append((int(d.group(1)), int(r.group(1)) * RATE_BITS[r.group(2)], r.group(0)))
    (df, bits, rate_txt), (dr, bits_r, _) = vals
    if bits != bits_r:
        raise SystemExit(f"{rel}: iki yön hızı farklı ({bits} ↔ {bits_r} bit/s)")
    rtt_ms = df + dr
    den = 8 * 1000 * QLIM_MTU                    # bit→B (8) · ms→s (1000) · paket (1200 B)
    return {"n": (2 * bits * rtt_ms) // den, "exact": 2 * bits * rtt_ms / den,
            "rate_Bps": bits / 8, "rate_txt": rate_txt, "rtt_ms": rtt_ms, "d_fwd": df,
            "d_rev": dr, "line_pk": bits * df / den, "fwd": fwd, "rev": rev}


def qlim_header(q):
    n = q["n"]
    return [
        f"# q_limit (EK-A §10.3): limit N = floor(2 × rate × RTT / {QLIM_MTU} B) = floor(2 × "
        f"{tr_num(q['rate_Bps'], 0)} B/s × {tr_num(q['rtt_ms'] / 1000, 3)} s / {QLIM_MTU} B) = "
        f"floor({tr_num(q['exact'], 4, True)}) = {n} paket; rate = netem `{q['rate_txt']}`, RTT = ileri "
        f"{q['d_fwd']} ms + dönüş {q['d_rev']} ms gecikme (jitter katılmaz); iki yöne `limit {n}`.",
        "# Mevcut 1000 paket profili (netem varsayılanı) ayrı kollarda korunur; bu kol yalnız kuyruk "
        "boyuyla ayrılır.",
        "# Not: netem `limit` gecikme hattındaki paketleri de sayar (tfifo): hatta ≈ rate × tek yön "
        f"gecikme / {QLIM_MTU} B ≈ {tr_num(q['line_pk'], 1)} paket, kuyruğa kalan ≈ "
        f"{tr_num(n - q['line_pk'], 1)} paket (gerçek paket boyu kola göre değişir).",
    ]


def a6_geom(lines, q):
    """FWD/REV satırları: q_limit kolunda iki yöne `limit N` eklenir, değilse kaynak AYNEN."""
    if q is None:
        return [lines["FWD"], lines["REV"]]
    fwd = f'FWD="{q["fwd"]} limit {q["n"]}"'
    rev = lines["REV"] if lines["REV"] == 'REV="$FWD"' else f'REV="{q["rev"]} limit {q["n"]}"'
    return [fwd, rev]


def a6_base_arm(arm):
    return arm[: -len(QLIM)] if arm.endswith(QLIM) else arm


def a6_f0a(lines, rel, refuse):
    for k in A_KEYS:
        if k not in lines:
            raise SystemExit(f"{rel}: {k} yok")
    m = re.match(r"^F0A_ARGS='(.*)'$", lines["F0A_ARGS"])
    if not m or "'" in m.group(1):
        raise SystemExit(f"{rel}: F0A_ARGS biçimi beklenen gibi değil")
    for flag in refuse:
        if flag in m.group(1).split():
            raise SystemExit(f"{rel}: kaynak hücrede {flag} zaten var")
    return m.group(1)


def a6_head(cell, arm, rel, sha, purpose, row, q):
    return [
        f"# [YOL3 · A-6 gece] {cell} — madde {row}; {purpose}. Üretici: "
        "scripts/yol3/tesis/gen_cells.py --set a6 (elle düzenlemeyin).",
        f"# Kaynak hücre: {rel} (sha256 {sha}) — geometri AYNEN; fark yalnız CELL, REPS, ARMS, "
        "kol bayrağı" + (" ve netem `limit` (q_limit)." if q else "."),
    ]


def a6_tail(cell, arm, lines, q, f0a, bin_ph, sub=""):
    return [
        f"CELL={cell}",
        *a6_geom(lines, q),
        f'REPS="${{F0_REPS:-{REPS}}}"',
        f'ARMS="{arm}"',
        lines["BUDGET_S"],
        lines["CAPTURE"],
        lines["SETTLE_S"],
        f"F0A_ARGS='{f0a}'",
        f'ARM_{arm}_SRV="{bin_ph}{sub} --role bob $F0A_ARGS{{x}}"',
        f'ARM_{arm}_CLI="{bin_ph}{sub} --role alice $F0A_ARGS{{x}}"',
        "",
    ]


def a6_kama_body(cell, arm, rel, sha, lines, purpose, row, q):
    _, e1, goster = A6_ARMS[a6_base_arm(arm)]
    src = a6_f0a(lines, rel, ("--e1-arm", "--fec-off", "--e1-edar", "--goster", "--playout-ms"))
    extra = [f"--e1-arm {e1}", "--fec-off on"] + (["--goster on"] if goster else [])
    f0a = src + " " + " ".join(extra)
    head = a6_head(cell, arm, rel, sha, purpose, row, q) + [
        f"# Kol {arm}: f0_flows --e1-arm {e1}{' --goster on' if goster else ''} — "
        f"{flags_text(e1, False)} {e1_src(e1)}{e2a_text(e1)} · FEC off (KK-ORK-01).",
        f"# İkili: BIN_TESIS = <{A6_TREE}>/target/release/examples/f0_flows (orkestratör, D-34). "
        "Zarflar Unix soketinden (netem dışı, beyanlı).",
        "# Gösterim (YOL3-T1, --goster on): S1 şeritleri LaneConfig::default (W = 5) + PTT oynatma "
        "tamponu T = 2,0 s; oku_a6.py ilk demet varışı, bütün demetler varış p95 ve ilk duyulabilir "
        "sesi ayrı yazar." if goster else
        "# Şeritsiz (--goster yok): tek sıralı kuyruk, oynatma tamponu yok; oku_a6.py ilk "
        "duyulabilir sesi yalnız s-kollarında yazar.",
    ] + (qlim_header(q) if q else [])
    return "\n".join(head + a6_tail(cell, arm, lines, q, f0a, "@BIN_TESIS@")).replace("{x}", "")


def a6_q_body(cell, arm, rel, sha, lines, purpose, row, q):
    _, val, _ = A6_ARMS[a6_base_arm(arm)]
    f0a = a6_f0a(lines, rel, ("--e1-arm", "--fec-off", "--e1-edar", "--goster", "--playout-ms",
                              "--q-kol"))
    head = a6_head(cell, arm, rel, sha, purpose, row, q) + [
        f"# Kol {arm}: bench f0-quinn --q-kol {val} — {Q_DESC[val]} · quinn 0.11.11/0.11.18, "
        "mesaj başına uni-stream, öncelik yok, oynatma tamponu yok (gövde: yol3/quinn-kol "
        "gen_q_cells.py @ d93aef10).",
        "# İkili: BIN_QK = <yol3/quinn-kol hedefi>/release/bench (orkestratör, D-34). El sıkışma "
        "netem içinde, t0 = bağlantı sonrası (Kama: zarflar Unix soketinden, netem dışı).",
    ] + (qlim_header(q) if q else [])
    body = "\n".join(head + a6_tail(cell, arm, lines, q, f0a, "@BIN_QK@", " f0-quinn"))
    return body.replace("{x}", f" --q-kol {val}")


def plan_a6():
    """[(madde, dosya adı, gövde)] TABLE_A6 sırasıyla (SIRA-A6)."""
    items = []
    for row, rel, arms, purpose in TABLE_A6:
        lines, sha = read_cell(rel)
        base = os.path.basename(rel)[: -len(".cell")]
        for arm in arms:
            cell = f"{base}__{arm}"
            if base.startswith("b_"):
                if arm.endswith(QLIM) or A6_ARMS[arm][0] != "kama" or A6_ARMS[arm][2]:
                    raise SystemExit(f"{cell}: b hücresinde yalnız şeritsiz Kama kolu")
                body = b_body(cell, arm, rel, sha, lines, "off", False, purpose, row, A6_TREE,
                              tag="[YOL3 · A-6 gece]", gen="scripts/yol3/tesis/gen_cells.py --set a6",
                              sira="madde")
            else:
                q = qlim_calc(lines, rel) if arm.endswith(QLIM) else None
                if q is not None and q["n"] < 1:
                    raise SystemExit(f"{cell}: q_limit N = {q['n']} < 1 paket — kol koşulmaz")
                fn = a6_kama_body if A6_ARMS[a6_base_arm(arm)][0] == "kama" else a6_q_body
                body = fn(cell, arm, rel, sha, lines, purpose, row, q)
            items.append((row, f"{cell}.cell", body))
    return items


def plan(analog, fec, edar_yok, which="g2"):
    """[(sıra, dosya adı, gövde)] öncelik sırasıyla."""
    if which == "a6":
        return plan_a6()
    table, _, _, tree, _ = SETS[which]
    items = []
    for row, src, arms, purpose in table:
        rel = ANALOG[analog] if src == "ANALOG" else src
        lines, sha = read_cell(rel)
        base = os.path.basename(rel)[: -len(".cell")]
        for arm in arms:
            if edar_yok and arm == "K2me":
                continue  # (e-dar) çıkınca K2−e = K2 (CC §4.2 ön testler; §10)
            cell = f"{base}__{arm}"
            body_fn = a_body if base.startswith("a_") else b_body
            body = body_fn(cell, arm, rel, sha, lines, fec, edar_yok, purpose, row, tree,
                           which in GOSTER_SETS)
            items.append((row, f"{cell}.cell", body))
    return items


def sira_text(items, which="g2"):
    cells = SETS[which][1]
    return "".join(f"scripts/yol3/tesis/{cells}/{name}\n" for _, name, _ in items)


def komut_a6(items):
    _, cells, sira, tree, kok = SETS["a6"]
    by_row = {}
    for row, _, _ in items:
        by_row[row] = by_row.get(row, 0) + 1
    q = {}
    for rel in (A6_C1, A6_C2, A6_C3):
        q[os.path.basename(rel)[: -len(".cell")]] = qlim_calc(read_cell(rel)[0], rel)
    ql = "\n".join(f"#   {c}: N = floor({tr_num(v['exact'], 4, True)}) = {v['n']} paket"
                   + ("" if v["n"] >= 1 else " < 1 → kol YAZILMADI") for c, v in q.items())
    rows = " · ".join(f"madde {r}: {n} dosya" for r, n in sorted(by_row.items()))
    return f"""\
# [YOL3 · A-6 gece] ölçüm — orkestratör koşar (D-18; KK-Ö-EKA-5: ölçüm sırasında ajan ve cargo
# durur). Depo kökü = {tree} ağacı. Hücreler {cells}/, sıra {sira} (madde 1, 2, 5, 3, 4).
# {len(items)} dosya × {REPS} tekrar = {len(items) * REPS} iş ({rows}).
# İkililer (orkestratör derler, D-34; worktree başına ayrı hedef, KK-Ö-EKA-D):
#   {tree}:  build --release --locked -p kama-integration --example f0_flows  → BIN_TESIS
#            build --release --locked -p bench --bin bench                     → BIN_TESIS_BENCH
#   yol3/quinn-kol (d93aef10): build --release --locked -p bench --bins          → BIN_QK
export BIN_TESIS=<{tree} hedefi>/release/examples/f0_flows
export BIN_TESIS_BENCH=<{tree} hedefi>/release/bench
export BIN_QK=<yol3/quinn-kol hedefi>/release/bench
systemd-inhibit --what=sleep:idle --why="YOL3 A-6 gece" setsid \\
  scripts/ktp2/f0/f0_grid.sh -j 11 ~/kama-k4g-work/yol3/tesis/{kok} $(cat scripts/yol3/tesis/{sira})
# Okuma (koşu bitince; R3 kıyası için K0 tabanı isteğe bağlı ayrı kökten, karma sürüm etiketiyle):
python3 -I scripts/yol3/tesis/oku_a6.py --kok ~/kama-k4g-work/yol3/tesis/{kok} \\
  [--r3-taban ~/kama-k4g-work/yol3/tesis/2026-11-09-g2]     # çıktı <kök>/okuma-a6/
# q_limit (EK-A §10.3; N = floor(2 × rate × RTT / {QLIM_MTU} B), RTT = iki yön gecikme toplamı):
{ql}
# Yazılmayan hücreler:
#   b hücrelerinde s-kolları (K1s, K1A6s): bench kama-product'ta S1 şeridi ve PTT yok (b_body
#     --goster'i reddeder); b senaryosu PTT taşımaz.
#   b_r25'te quinn (Q_bbr_rfc, Q_cubic): QUIC ilk uçuşu her yönde ≥ 1200 B UDP yükü (RFC 9000
#     §14.1) → 25 B/s'de el sıkışma ≥ 99,4 s + 2d > D = 60 s (RAPOR-B hesabı); Q_*
#     tanımı f0-quinn akış senaryosu, bench B'nin quinn kolu ayrı tanım.
#   b_r25'te q_limit: N < 1 paket (yukarıda).
#   b_r12k_d1000_l0_p1000'de quinn: iş emri bu hücrede yalnız K1, K1A6 ister.
"""


def komut(items, fec, analog, which="g2"):
    if which == "a6":
        return komut_a6(items)
    n1 = sum(1 for row, _, _ in items if row == 1)
    _, _, sira, tree, kok = SETS[which]
    return f"""\
# [YOL3 · Ö] Teşhis koşusu (§8.2) — orkestratör koşar (D-18); ajan ve cargo durur (v2 §11.5).
# Depo kökünden. İkililer {tree} release (orkestratör derler, D-34).
# Koşu kaydı: manifest docs/design/E1-TESHIS-G2.md sha256 37bcabe3…; FEC {fec}{ork(fec)};
#   analog {analog}; {sira} sha256 ve ikili sha256'ları (cell.env) kayda.
export BIN_TESIS=<{tree} ağacı>/target/release/examples/f0_flows
export BIN_TESIS_BENCH=<{tree} ağacı>/target/release/bench
# Tamamı ({len(items)} dosya, {len(items) * REPS} iş; ilk {n1} satır = 1. sıra, {n1 * REPS} iş):
systemd-inhibit --what=sleep:idle --why="YOL3 teshis" setsid \\
  scripts/ktp2/f0/f0_grid.sh -j 11 ~/kama-k4g-work/yol3/tesis/{kok} $(cat scripts/yol3/tesis/{sira})
# Yalnız 1. sıra önce (ayrı parti; aynı kök, DONE'u olan hücre ikinci partide atlanır):
#   ... f0_grid.sh -j 11 ~/kama-k4g-work/yol3/tesis/{kok} $(head -n {n1} scripts/yol3/tesis/{sira})
# Okuma (koşu bitince; manifest sha256 denetlenir, farklıysa okuma yok):
python3 -I scripts/yol3/tesis/oku.py --kok ~/kama-k4g-work/yol3/tesis/{kok} \\
  --manifest docs/design/E1-TESHIS-G2.md      # çıktı <kök>/okuma/{{OKUMA.md,okuma.json}}
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--analog", choices=sorted(ANALOG), default="y10")
    ap.add_argument("--fec", choices=["off", "urun"], default="off")  # KK-ORK-01
    ap.add_argument("--edar-yok", action="store_true")
    ap.add_argument("--set", dest="which", choices=sorted(SETS), default="g2")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--komut", action="store_true")
    a = ap.parse_args()
    items = plan(a.analog, a.fec, a.edar_yok, a.which)
    cells_dir = os.path.join(HERE, SETS[a.which][1])
    sira_path = os.path.join(HERE, SETS[a.which][2])
    if a.komut:
        print(komut(items, a.fec, a.analog, a.which), end="")
        return 0
    if a.check:
        errs = []
        want = {name for _, name, _ in items}
        have = set(os.listdir(cells_dir)) if os.path.isdir(cells_dir) else set()
        for extra in sorted(have - want):
            errs.append(f"fazla dosya: {SETS[a.which][1]}/{extra}")
        for _, name, body in items:
            try:
                with open(os.path.join(cells_dir, name), encoding="utf-8") as f:
                    if f.read() != body:
                        errs.append(f"{SETS[a.which][1]}/{name}: üretilenle aynı değil")
            except OSError as e:
                errs.append(f"{SETS[a.which][1]}/{name}: {e}")
        try:
            with open(sira_path, encoding="utf-8") as f:
                if f.read() != sira_text(items, a.which):
                    errs.append(f"{SETS[a.which][2]}: üretilenle aynı değil")
        except OSError as e:
            errs.append(f"{SETS[a.which][2]}: {e}")
        for e in errs:
            print(f"FARK: {e}", file=sys.stderr)
        n1 = sum(1 for row, _, _ in items if row == 1)
        print(f"{len(items)} dosya · {len(items) * REPS} iş · 1. sıra {n1} dosya / {n1 * REPS} iş"
              f" · set={a.which} analog={a.analog} fec={a.fec} edar_yok={a.edar_yok} · "
              f"{'temiz' if not errs else f'{len(errs)} fark'}")
        return 1 if errs else 0
    os.makedirs(cells_dir, exist_ok=True)
    want = {name for _, name, _ in items}
    for old in os.listdir(cells_dir):
        if old.endswith(".cell") and old not in want:
            os.remove(os.path.join(cells_dir, old))
    for _, name, body in items:
        with open(os.path.join(cells_dir, name), "w", encoding="utf-8") as f:
            f.write(body)
    with open(sira_path, "w", encoding="utf-8") as f:
        f.write(sira_text(items, a.which))
    print(f"{len(items)} dosya, {len(items) * REPS} iş → {os.path.relpath(cells_dir, REPO)}; "
          f"{os.path.relpath(sira_path, REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
