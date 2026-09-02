#!/usr/bin/env python3
"""MS1 OFFLINE-COUNTERFACTUAL (talimat-§9): alternatif-budget-modellerini kayıtlı-trace üstünde REPLAY.
YALNIZ-MODELLEME — implementasyon/onay-DEĞİL; production-kod-değişikliği-YOK.

Girdi: ms1-interval.csv (1s-bin: rate,sealed,enq,emit,canc,denials-kümülatif) + ms1-block-ledger.csv
(close_ms, planned_r). Replay-varsayımları (SINIRLAR — raporda-da):
  (V1) Kanal/feedback SABİT: model-parity'si loss/RTT/BtlBw'yi-DEĞİŞTİRMEZ (counterfactual-yalnız-
       token-fizibilitesi; uçtan-uca-goodput-İDDİASI-YOK).
  (V2) DATA-talebi ölçülen-seriden: data_bytes(t)=Δsealed−Δemit×(UNIT+OH) (retx-dahil).
  (V3) leftover L(t)=max(0, rate(t)·1s − data_bytes(t)) — burst-cap-mikro-dinamiği-1s-bin'de-yıkanır
       (fidelity-kontrolü baseline-replay-vs-ölçülen-emit ile raporlanır).
  (V4) cancel'ler ölçülen-Δcanc-anında FIFO-en-eski-bekleyen-bloklara-uygulanır.
MODELLER: Baseline(leftover-FIFO) · A bounded-share(α=0.25, work-conserving; DATA-yerinden-etme-ölçülür)
· B micro-bucket(β=0.15, burst=4·UNIT) · C per-block-admission-cap(2)+leftover-drain · D floor(1/blok
≤2s)+hard-cap(0.30·Σrate) · E deadline-aware=DEĞERLENDİRİLMEDİ (deadline-semantiği-tanımsız; fuzzy-
deadline-girdisi-yok — talimat-şartı).
Çıktı-metrikleri: realization · first-parity-delay-p50 · DATA-displacement-payı (Σmax(0,grant−L)/Σrate·1s)
· max-bekleyen-parity · starve-blok-oranı (close+10s-içinde-0-parity).
"""
import csv, sys, statistics as st
from collections import defaultdict

UNIT, OH = 1200, 37

def load(path):
    intervals = defaultdict(list)
    for r in csv.DictReader(open(f"{path}/ms1-interval.csv")):
        intervals[(r["cell"], r["arm"], int(r["seed"]))].append(
            (int(r["t_ms"]), int(r["rate"]), int(r["sealed_bytes"]), int(r["enq"]), int(r["emit"]), int(r["canc"])))
    blocks = defaultdict(list)
    for r in csv.DictReader(open(f"{path}/ms1-block-ledger.csv")):
        if int(r["planned_r"]) > 0 and int(r["close_ms"]) >= 0:
            blocks[(r["cell"], r["arm"], int(r["seed"]))].append((int(r["close_ms"]), int(r["planned_r"])))
    return intervals, blocks

def replay(series, closes, model):
    # closes: [(t_ms, planned)] sıralı; kuyruk: [(t_close, kalan_adet)]
    closes = sorted(closes)
    ci = 0
    queue = []
    granted = defaultdict(int)  # t_close → emit-adedi
    first_emit = {}
    displacement = 0.0
    bucket = 0.0
    total_rate_bytes = 0.0
    cum_parity_bytes = 0.0
    maxq = 0
    prev_sealed = None
    prev_emit = 0
    prev_canc = 0
    for (t, rate, sealed, _enq, emit, canc) in series:
        # ölçülen-Δ'lar
        d_sealed = 0 if prev_sealed is None else sealed - prev_sealed
        d_emit_bytes = (emit - prev_emit) * (UNIT + OH)
        data_bytes = max(0, d_sealed - d_emit_bytes)
        prev_sealed, prev_emit = sealed, emit
        rate_bytes = rate * 1.0
        total_rate_bytes += rate_bytes
        L = max(0.0, rate_bytes - data_bytes)
        # yeni-close'lar kuyruğa
        while ci < len(closes) and closes[ci][0] <= t:
            tc, planned = closes[ci]
            if model == "C":
                planned = min(planned, 2)
            queue.append([tc, planned])
            ci += 1
        # cancel'ler (V4): ölçülen-Δcanc FIFO
        d_canc = canc - prev_canc
        prev_canc = canc
        while d_canc > 0 and queue:
            take = min(d_canc, queue[0][1])
            queue[0][1] -= take
            d_canc -= take
            if queue[0][1] == 0:
                queue.pop(0)
        maxq = max(maxq, sum(q[1] for q in queue))
        # model-grant (byte)
        if model in ("Baseline", "C"):
            budget = L
        elif model == "A":
            alpha = 0.25 * rate_bytes
            budget = max(L, min(alpha, sum(q[1] for q in queue) * UNIT))
            displacement += max(0.0, min(budget, sum(q[1] for q in queue) * UNIT) - L)
        elif model == "B":
            bucket = min(bucket + 0.15 * rate_bytes, 4 * UNIT)
            budget = bucket
        elif model == "D":
            budget = L
            # floor: close+2s-geçmiş-ve-0-emitli-bloklara 1-parity-zorla (yerinden-etme-sayılır)
            for q in queue:
                if q[1] > 0 and t - q[0] >= 2000 and granted[q[0]] == 0:
                    budget_extra = UNIT
                    displacement += max(0.0, budget_extra - max(0.0, L - budget + budget))
                    q[1] -= 1
                    granted[q[0]] += 1
                    first_emit.setdefault(q[0], t)
                    cum_parity_bytes += UNIT
            if cum_parity_bytes > 0.30 * total_rate_bytes:
                budget = 0.0
        else:
            raise ValueError(model)
        # FIFO-drain
        while budget >= UNIT and queue:
            q = queue[0]
            if q[1] == 0:
                queue.pop(0)
                continue
            q[1] -= 1
            budget -= UNIT
            if model == "B":
                bucket = budget
            granted[q[0]] += 1
            first_emit.setdefault(q[0], t)
            cum_parity_bytes += UNIT
            if model == "D" and cum_parity_bytes > 0.30 * total_rate_bytes:
                break
            if q[1] == 0:
                queue.pop(0)
    planned_tot = sum(p for _, p in closes)
    emitted_tot = sum(granted.values())
    delays = [first_emit[tc] - tc for tc, _ in closes if tc in first_emit]
    starve = sum(1 for tc, _ in closes if tc not in first_emit or first_emit[tc] - tc > 10_000)
    return {
        "realization": emitted_tot / planned_tot if planned_tot else 0.0,
        "delay_p50": st.median(delays) if delays else -1,
        "displacement_share": displacement / max(total_rate_bytes, 1.0),
        "maxq": maxq,
        "starve_frac": starve / len(closes) if closes else 0.0,
        "measured_check": None,
    }

def main(path):
    intervals, blocks = load(path)
    out = open(f"{path}/ms1-counterfactual.csv", "w")
    out.write("cell,arm,model,realization_med,delay_p50_med,displacement_med,maxq_med,starve_med,fidelity_med\n")
    agg = defaultdict(list)
    for key, series in sorted(intervals.items()):
        cell, arm, seed = key
        if arm not in ("Manual-r4", "Manual-r8"):
            continue
        closes = blocks.get(key, [])
        if not closes:
            continue
        measured_emit = series[-1][4]
        for model in ["Baseline", "A", "B", "C", "D"]:
            r = replay(series, closes, model)
            if model == "Baseline":
                planned_tot = sum(p for _, p in closes)
                model_emit = r["realization"] * planned_tot
                r["measured_check"] = abs(model_emit - measured_emit) / max(measured_emit, 1)
            agg[(cell, arm, model)].append(r)
    for (cell, arm, model), rs in sorted(agg.items()):
        med = lambda k: st.median([x[k] for x in rs])
        fid = st.median([x["measured_check"] for x in rs if x["measured_check"] is not None] or [-1])
        out.write(f"{cell},{arm},{model},{med('realization'):.4f},{med('delay_p50'):.0f},{med('displacement_share'):.4f},{med('maxq'):.0f},{med('starve_frac'):.4f},{fid:.4f}\n")
    print("yazıldı: ms1-counterfactual.csv")

if __name__ == "__main__":
    main(sys.argv[1])
