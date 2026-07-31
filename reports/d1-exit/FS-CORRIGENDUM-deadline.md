# FS-CORRIGENDUM — FS DEADLINE RAW-DATA RECOMPUTE (DR-FEC-FS-1 v2 §9)

**Statü:** İlk-sonuç-görevi (§27-4). Tarihsel `MarginalFecEssential` iddiası ("305ms deadline; r>=d meet
~%100") ham blok-timestamp'lerinden YENİDEN hesaplandı. "~%100" YASAK → exact hits/N. Veri:
`reports/d1-exit/data/wallclock/rtt250-*-hyb-*-rxledger.csv` (warm-steady blok: deficit_applied>0; 30
blok/hücre = 10-seed × 3-warm-blok). Metrik: `deadline_hit = (complete_ms − first_ms) ≤ 305 ms`
(bir-yön-gecikme-iptalli göreli-tamamlanma; deadline 305ms Phase-C'den winner-görülmeden dondurulmuştu).

## 1. EXACT HITS/N (rtt250, hybrid)

| hücre | arm | N | deadline-hit | hit% | comp p50 | comp min–max |
|---|---|---|---|---|---|---|
| d1 / r0 | r<d (ARQ+0) | 30 | 0 | **0.0%** | 333 | 328–408 |
| d1 / r1 | r≥d (ARQ+1) | 30 | 30 | **100.0%** | 288 | 281–293 |
| d2 / r1 | r<d* (ARQ+1) | 30 | 0 | **0.0%** | 356 | 314–390 |
| d2 / r2 | r≥d (ARQ+2) | 30 | 28 | **93.3%** | 300 | 291–307 |
| d4 / r2 | r<d* (ARQ+2) | 30 | 0 | **0.0%** | 393 | 378–464 |
| d4 / r4 | r≥d (ARQ+4) | 30 | 4 | **13.3%** | 315 | 293–332 |

`* r<d baseline`: **d2/r0 ve d4/r0 (gerçek sıfır-parity A0) rtt250'de KOŞULMADI** — mevcut r<d arm en-yüksek-
hâlâ-yetersiz r (d2→r1, d4→r2). Yani d2/d4'te r<d = "1-eksik" arm, "sıfır-parity" değil. (Veri-eksiği;
FS-PREREG C-P5 bunu düzeltecek.)

## 2. HÜKÜM DÜZELTMESİ

- **MarginalFecBeneficial (deadline-siz completion-hızlanması) — GEÇERLİ (korunur).** r≥d, r<d'den p50-olarak
  **d1:−45ms · d2:−56ms · d4:−78ms** daha hızlı (tarihsel ~46/52/77 ile tutarlı). FEC-path 30/30. Değişmedi.
- **MarginalFecEssential (deadline=305ms tutturma) — KISMEN GERİ ÇEKİLDİ.** Tarihsel "deficit 1/2/4 → r≥d
  meet ~%100" **YANLIŞ genelleme**. Exact:
  - **deficit=1: DOĞRULANDI** — r<d 0% → r≥d **100%**. Temiz MarginalFecEssential.
  - **deficit=2: ZAYIF-DOĞRULANDI** — r<d 0% → r≥d **93.3%** (2/30 blok 306–307ms'te deadline'ı kıl-payı kaçırdı).
  - **deficit=4: GEÇERSİZ** — r<d 0% → r≥d **yalnız 13.3%** (p50=315>305). Parity-eklemek d4'te deadline'ı
    KURTARMIYOR; r≥d arm da çoğunlukla kaçırıyor. **d4'te MarginalFecEssential YOK.**

**Düzeltilmiş ifade:** *MarginalFecEssential yalnız düşük-eksik (deficit≤2) rejiminde geçerli (rtt250,
305ms); deficit=4'te r≥d arm da deadline'ı kaçırır → essential-fayda deficit-ekseninde monoton-değil,
düşük-deficit'e sınırlı.*

## 3. NEDEN (mekanizma-tutarlı)
d4'te yeterli-parity (r=4) blok-kapanış-sonrası çıktığından, 4-shard'lık parity-uçuşu tek-RTT-pencereye
sığmıyor; p50-completion 315ms > 305ms. Bu §2'deki emisyon-modeli-sınırının (parity@blok-kapanış +
ACK-clocked-source) doğrudan-sonucu — FS-1'deki F1–F4 erken/artımlı-emisyon aileleri tam-da bunu hedefler.

## 4. PROVENANCE
```text
recompute-script: inline python (bu-tur; deterministik; complete_ms−first_ms≤305 sayımı)
raw: reports/d1-exit/data/wallclock/rtt250-{pin1-d1-r0/r1, pin2-d2-r1/r2, pin3-d4-r2/r4}-hyb-{1..10}-rxledger.csv
supersedes-partial: FS-wallclock-fec-utility.md §49-51 ("MarginalFecEssential deficit 1/2/4 ~%100")
                    → deficit≤2-sınırlı; d4-GERİ-ÇEKİLDİ. §Beneficial değişmedi.
```
