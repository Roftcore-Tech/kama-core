#!/usr/bin/env python3
"""[KTP2-F0 · Ajan C] Hücre üreteci — `scripts/ktp2/f0/cells/c/*.cell` (tam set) ve
`scripts/ktp2/f0/cells/c/duman/*.cell` (REPS=1 duman alt kümesi).

Hücre dosyaları bu betikten ÜRETİLİR; elle düzenlenmez. Değişiklik = bu betik + yeniden üretim +
commit (ikisi birlikte). Koşu başlatmaz.

Kol şablonları (her kol iki ikiliyle: `_taban` = $BIN_TABAN, `_aday` = $BIN_ADAY):
  arq     ARQ-yalnız            --fec-profile 0                      (FEC alt sistemi yok)
  uyar    mevcut uyarlamalı FEC  (FEC bayrağı yok = ürün profili 0x05, Controller)
  rs2     sabit RS K=2 E=64     --fec-profile 1 --fec-mode static --fec-r r
  rs4     sabit RS K=4 E=64     --fec-profile 2 --fec-mode static --fec-r r
  tekrar  tekrar (vekil)        --fec-profile 0, her demet iki ayrı birim, T/2 arayla
Teşhis kolları (yalnız taban, ayrı `cx_` hücreleri, REPS=10, betimsel):
  uyar64  uyarlamalı, E=64      --fec-profile 2 --fec-mode adaptive
  pol     son tarih politikası  --fec-mode policy --fec-deadline-ms (800 − T)

Ortak: --transport kama-product, --payload = kodlayıcı(1,2 kbit/s·T) + 13 (AMP) + 6 (zarf 22−16),
--warmup-msgs 16, konuşma ≈ 20 s, --ts-out, --manifest; sunucu --rx-log (İSTEK-C1, Ajan D).
`--no-rxlog` bayrağı İSTEK-C1 gelmeden duman için sunucu bayrağını çıkarır (sonuç [vekil] olur).

Kullanım: python3 -I scripts/ktp2/f0/c/gen_cells.py [--no-rxlog]
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CELLS = os.path.normpath(os.path.join(HERE, "..", "cells", "c"))
sys.path.insert(0, HERE)
import model_c as M  # noqa: E402  (aynı dizin; tek doğruluk kaynağı: tel sabitleri, r tablosu)

NETEM_RATE = {"r1k": "8kbit", "r12k": "100kbit"}
NETEM_DELAY = {"d300": "delay 300ms 60ms distribution normal",
               "d1000": "delay 1000ms 200ms distribution normal"}
MAX_DEADLINE_S = {"d300": 6, "d1000": 10}
WARMUP = 16
TALK_S = 20.0
# Koşu süresi tahmini (s/kol-koşusu): yerleşme 1 + ısınma + konuşma + kuyruk + kapanış (RAPOR-C §6).
EST_RUN_S = {"d300": 30.0, "d1000": 38.0}
EST_RUN_S_STRESS = 45.0

PRIMARY = ["arq", "uyar", "rs2", "rs4", "tekrar"]
DIAG_KINDS = ("uyar64", "pol")
BINS = ["taban", "aday"]

# (öncelik, ad-ekleri, REPS). Öncelik 1 = birincil; 3 = betimsel.
GRID = [
    # öncelik 1 — RTT 600 ms, %20, 120 ms önce
    (1, ("r12k", "d300", 20, 120), 30),
    (1, ("r12k", "d300", 10, 120), 30),
    (1, ("r12k", "d300", 30, 120), 30),
    (1, ("r12k", "d300", 20, 60), 30),
    (1, ("r12k", "d300", 20, 200), 30),
    (1, ("r12k", "d1000", 20, 120), 30),
    (1, ("r1k", "d300", 20, 200), 30),
    # öncelik 2
    (2, ("r12k", "d1000", 10, 120), 30),
    (2, ("r12k", "d1000", 30, 120), 30),
    (2, ("r1k", "d300", 10, 200), 30),
    (2, ("r1k", "d300", 30, 200), 30),
    # öncelik 3 — betimsel (N=10)
    (3, ("r12k", "d300", 50, 120), 10),
    (3, ("r1k", "d300", 20, 120), 10),
    (3, ("r12k", "d300", 10, 60), 10),
    (3, ("r12k", "d300", 30, 60), 10),
    (3, ("r12k", "d300", 10, 200), 10),
    (3, ("r12k", "d300", 30, 200), 10),
    (3, ("r12k", "d1000", 20, 60), 10),
    (3, ("r12k", "d1000", 20, 200), 10),
]
# Teşhis hücreleri: uyarlamalı FEC'in sıfır paritesinin kök nedenini sayaçla ayırmak için.
DIAG = [
    (2, ("r12k", "d300", 20, 120), 10),
    (2, ("r12k", "d300", 30, 120), 10),
]
DUMAN = [("r12k", "d300", 20, 120), ("r12k", "d1000", 20, 120), ("r1k", "d300", 20, 200),
         ("r12k", "d300", 50, 120)]


def cell_name(c, prefix="c"):
    rate, d, loss, t = c
    return f"{prefix}_{rate}_{d}_l{loss}_b{t}"


def netem(c):
    rate, d, loss, _ = c
    parts = [NETEM_DELAY[d]]
    if loss:
        parts.append(f"loss {loss}%")
    parts.append(f"rate {NETEM_RATE[rate]}")
    return " ".join(parts)


def frames(t):
    n = int(round(TALK_S * 1000 / t))
    return (n // 8) * 8 + (8 if n % 8 else 0)  # K ∈ {2,4,8} ile hizalı (kısmi blok yok)


def fec_flags(kind, c):
    rate, _, loss, t = c
    if kind in ("arq", "tekrar"):
        return "--fec-profile 0"
    if kind == "uyar":
        return ""  # ürün profili: --fec-profile 5 --fec-mode adaptive (bayraksız varsayılan)
    if kind == "uyar64":
        return "--fec-profile 2 --fec-mode adaptive"
    if kind == "pol":
        return f"--fec-mode policy --fec-deadline-ms {800 - t}"
    if kind in ("rs2", "rs4"):
        r = M.r_select(kind, loss, t, M.RATES[rate])
        return f"--fec-profile {M.RS_ARMS[kind]['profile']} --fec-mode static --fec-r {r}"
    raise ValueError(kind)


def arm_lines(kind, binv, c, name, rxlog):
    _, d, _, t = c
    payload = M.bench_payload(t)
    copies = 2 if kind == "tekrar" else 1
    interval = t // copies
    msgs = frames(t) * copies
    fec = fec_flags(kind, c)
    b = "@BIN_TABAN@" if binv == "taban" else "@BIN_ADAY@"
    rx = " --rx-log @D@/rx.jsonl" if rxlog else ""
    srv = f"{b} serve --bind @SRV@ --transport kama-product --payload {payload} {fec}{rx}"
    cli = (f"{b} client --server @SRV@ --transport kama-product --msgs {msgs} --payload {payload} "
           f"--submit-interval-ms {interval} --warmup-msgs {WARMUP} --max-deadline-s {MAX_DEADLINE_S[d]} "
           f"{fec} --profile {name} --repeat @REP@ --ts-out @D@/ts.csv --manifest @D@/manifest.json "
           f"--fwd-impair '$FWD' --rev-impair '$REV' --out @D@/out.jsonl")
    srv = " ".join(srv.split())
    cli = " ".join(cli.split())
    arm = f"{kind}_{binv}"
    return arm, srv, cli


def write_cell(path, c, name, kinds, bins, reps, prio, rxlog):
    _, d, loss, t = c
    arms = []
    lines = []
    for kind in kinds:
        for binv in bins:
            if kind in DIAG_KINDS and binv != "taban":
                continue  # teşhis kolları yalnız tabanda
            arm, srv, cli = arm_lines(kind, binv, c, name, rxlog)
            arms.append(arm)
            # $FWD/$REV kaynak anında genişler (çift tırnak); @…@ f0_cell.sh'te doldurulur.
            lines.append(f'ARM_{arm}_SRV="{srv}"')
            lines.append(f'ARM_{arm}_CLI="{cli}"')
    est = (EST_RUN_S_STRESS if loss >= 50 else EST_RUN_S[d]) * len(arms) * reps
    rs_info = ", ".join(f"{k} r={M.r_select(k, loss, t, M.RATES[c[0]])}" for k in kinds if k in ("rs2", "rs4"))
    hdr = [
        "# [KTP2-F0 · Ajan C] ÜRETİLDİ — scripts/ktp2/f0/c/gen_cells.py (elle düzenlemeyin).",
        f"# öncelik={prio}  demet={t} ms  kayıp=%{loss}  gecikme={d}  hız={c[0]}  "
        f"payload={M.bench_payload(t)} B  çerçeve={frames(t)}  ısınma={WARMUP}",
        f"# {rs_info}" if rs_info else "# (sabit RS kolu yok)",
        f"# tahmini süre: {len(arms)} kol × {reps} tekrar × ~{est / len(arms) / reps:.0f} s = {est / 3600:.2f} koşu-saati",
        f"# rx-log: {'AÇIK (İSTEK-C1)' if rxlog else 'KAPALI (vekil ölçü: gönderen ACK zamanı)'}",
    ]
    body = [
        f"CELL={name}",
        f'FWD="{netem(c)}"',
        'REV="$FWD"',
        f"REPS={reps}",
        f'ARMS="{" ".join(arms)}"',
        "BUDGET_S=120",
        "CAPTURE=0",
        "SETTLE_S=1",
    ]
    with open(path, "w") as f:
        f.write("\n".join(hdr + body + lines) + "\n")
    return est, len(arms) * reps


def main():
    rxlog = "--no-rxlog" not in sys.argv[1:]
    os.makedirs(CELLS, exist_ok=True)
    os.makedirs(os.path.join(CELLS, "duman"), exist_ok=True)
    index = ["# Ajan C hücre dizini (ÜRETİLDİ: gen_cells.py)", "",
             "| öncelik | hücre | kol-koşusu | tahmini koşu-saati |", "|---|---|---|---|"]
    tot = {1: 0.0, 2: 0.0, 3: 0.0}
    for prio, c, reps in GRID:
        name = cell_name(c)
        est, n = write_cell(os.path.join(CELLS, name + ".cell"), c, name, PRIMARY, BINS, reps, prio, rxlog)
        tot[prio] += est
        index.append(f"| {prio} | {name} | {n} | {est / 3600:.2f} |")
    for prio, c, reps in DIAG:
        name = cell_name(c, "cx")
        est, n = write_cell(os.path.join(CELLS, name + ".cell"), c, name, ["uyar64", "pol", "uyar"],
                            ["taban"], reps, prio, rxlog)
        tot[prio] += est
        index.append(f"| {prio} | {name} | {n} | {est / 3600:.2f} |")
    dum_est = 0.0
    for c in DUMAN:
        name = cell_name(c) + "_duman"
        kinds = PRIMARY + (["uyar64", "pol"] if c == DUMAN[0] else [])
        est, n = write_cell(os.path.join(CELLS, "duman", name + ".cell"), c, name, kinds, BINS, 1, 0, rxlog)
        dum_est += est
        index.append(f"| duman | duman/{name} | {n} | {est / 3600:.2f} |")
    index += ["", f"Toplam: öncelik-1 {tot[1] / 3600:.1f} h · öncelik-2 {tot[2] / 3600:.1f} h · "
              f"öncelik-3 {tot[3] / 3600:.1f} h · tam set {sum(tot.values()) / 3600:.1f} h · "
              f"duman {dum_est / 3600:.2f} h"]
    with open(os.path.join(CELLS, "INDEX.md"), "w") as f:
        f.write("\n".join(index) + "\n")
    print("\n".join(index))
    return 0


if __name__ == "__main__":
    sys.exit(main())
