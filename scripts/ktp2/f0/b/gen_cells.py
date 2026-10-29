#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] cells/b/*.cell üreticisi — geometri ve sınırlar fizik.py'den.

Üretilen dosyalar commit edilir; elle düzenlenmez (değişiklik fizik.py'de yapılır, bu betik yeniden
koşar). Ayrıca cells/b/LISTE-<öncelik>.txt ve cells/b/DUMAN.txt yazılır (f0_grid.sh'ye verilecek
sıra).

Kullanım: python3 -I gen_cells.py
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # -I betik dizinini sys.path'e koymaz; kendi modülümüz açıkça
sys.dont_write_bytecode = True  # depoya __pycache__ yazılmasın
import fizik  # noqa: E402

CELLS = HERE.parent / "cells" / "b"

# Duman seti (F0_REPS=1): her (hız, yük) sınıfı + r25 + bir stres hücresi; boru hattı, yakalama ve
# ayrıştırma denetimi için. Sıra: kısa koşular önce.
DUMAN = [
    "b_r12k_d300_l30_p65",
    "b_r12k_d1000_l10_p1000",
    "b_r1k_d300_l0_p65",
    "b_r1k_d1000_l20_p1000",
    "b_r25_d1000_l20_p65",
    "b_r12k_d300_l80_p65",
]

TEMPLATE = """# [KTP2-F0 / Ajan B] {cell} — ÜRETİLDİ: scripts/ktp2/f0/b/gen_cells.py (fizik.py). ELLE DÜZENLEME.
# öncelik={prio} tür={kind} N={reps} · ρ_ileri={rho:.3f} (tek iletim Kama veri paketi / hat) ·
# veri paketi {s_data} B tel · RTT_fiz {rtt:.3f} s (0 aralıklı ACK) / {rtt16:.3f} s (16 aralıklı) ·
# RTT_fifo {rttf:.3f} s (netem sıra-koruyan jitter) · tek yön alt sınır {ow:.3f} s · ideal ARQ p95 tek yön {p95:.2f} s
CELL={cell}
RATE_ID={rate_id}
DELAY_ID={delay_id}
LOSS_PCT={loss}
RATE_BPS={rate}
DELAY_MS={delay}
JITTER_MS={jit}
MSGS={msgs}
PAY={pay}
IVAL_MS={ival}
DEADLINE_S={dl}
FWD="{netem}"
REV="$FWD"
REPS="${{F0_REPS:-{reps}}}"
ARMS="taban aday quinn tcp"
BUDGET_S={budget}
CAPTURE=0
SETTLE_S=1.5
# shellcheck disable=SC1091
source "$(cd "$(dirname "${{BASH_SOURCE[0]}}")/../../b" && pwd)/hucre_b.sh"
b_hucre_kur
"""


def main():
    CELLS.mkdir(parents=True, exist_ok=True)
    for old in CELLS.glob("b_*.cell"):
        old.unlink()
    by_prio = {}
    names = set()
    for c in fizik.grid():
        txt = TEMPLATE.format(
            cell=c["cell"], prio=c["prio"], kind=c["kind"], reps=c["reps"], rho=c["rho_fwd"],
            s_data=c["s_data_l2"], rtt=c["rtt_phys_s"], rtt16=c["rtt_phys_r16_s"],
            rttf=c["rtt_fifo_s"], ow=c["oneway_lb_s"], p95=c["ideal_arq_oneway_p95_s"], rate_id=c["rate_id"],
            delay_id=c["delay_id"], loss=c["loss"], rate=c["rate_Bps"], delay=c["delay_ms"],
            jit=c["jitter_ms"], msgs=c["msgs"], pay=c["payload"], ival=c["ival_ms"],
            dl=c["deadline_s"], netem=c["netem"], budget=fizik.budget_s(c))
        (CELLS / f"{c['cell']}.cell").write_text(txt)
        by_prio.setdefault(c["prio"], []).append(c["cell"])
        names.add(c["cell"])
    for p, lst in sorted(by_prio.items()):
        (CELLS / f"LISTE-{p}.txt").write_text("".join(f"scripts/ktp2/f0/cells/b/{n}.cell\n" for n in lst))
    missing = [n for n in DUMAN if n not in names]
    if missing:
        raise SystemExit(f"duman hücresi ızgarada yok: {missing}")
    (CELLS / "DUMAN.txt").write_text("".join(f"scripts/ktp2/f0/cells/b/{n}.cell\n" for n in DUMAN))
    print(f"{len(names)} hücre yazıldı → {CELLS}")


if __name__ == "__main__":
    main()
