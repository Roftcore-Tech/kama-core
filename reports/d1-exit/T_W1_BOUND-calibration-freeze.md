# T_W1_BOUND — FORMAL CALIBRATION + FREEZE (FC6 önkoşul-artefaktı)

**ÇERÇEVE:** Bu deney ARQ ile FEC'i karşılaştırmaz; ARQ bütün kollarda aktiftir; ölçülen şey ARQ üzerine
eklenen proaktif FEC'in marjinal faydasıdır. (Bu artefakt cold-start LIVENESS bound'udur; FEC-marjı-değil.)

**Amaç:** W-1 (first-source-loss cold-start) kabul-tavanını, **temiz-koşu-verisini GÖRMEDEN**, birinci-
ilkelerden türet + DONDUR. Disiplin: "sonuçları gördükten sonra bound-seçme YOK" (talimat-§prohibitions).
Bu dosya, W-1-rerun analizinden **ÖNCE** commit edilir; sonraki analiz bu donmuş-değeri kullanır.

## 1. Senaryo (W-1)
- netem: RTT100 (delay 50ms / delay 50ms), çift-yön açık.
- Erasure: `--deficit 1 --deficit-from-block 0` → HER blok'un ilk-source-esi'si (esi 0) ingress'te silinir.
- Politika: `--policy leftover --fixed-r 0` → PROAKTIF-FEC-YOK (r=0). Blok-0 kurtarımı YALNIZ ARQ
  (RECOVERED-kontrol → retransmit). Bu, ARQ-substratının cold-start-clock'la doğru-saatlenmesini test eder.
- Ölçülen: blok-0 completion wall-clock'u (cold-start ON) vs (OFF).

## 2. Bound'un ANLAMI
T_W1_BOUND bir **LIVENESS KABUL-TAVANI**dır (performans-hedefi DEĞİL): "cold-start ON iken blok-0,
T_W1_BOUND içinde tamamlanMALI." OFF-kolu bu tavanı (ve 90s-ufkunu) aşarak cold-start-deadlock'unu gösterir.
Kabul-kriteri: `T_ON(block0) < T_W1_BOUND < T_OFF(block0 | 90s-ufuk)`.

## 3. Birinci-ilke türetme (RTT-üst-sınırı ≤ 1000ms; W-1 RTT100'de koşar ama tavan RTT-envelope'u kapsar)
Cold-start ON'da blok-0 kurtarım-zaman-çizelgesi (bileşen üst-sınırları):

| Bileşen | Üst-sınır | Gerekçe |
|---|---|---|
| İlk-DATA-emit → ladder-arm | ~0 | arm ilk-DATA-wire-emit'te (M2; INV-CS-7) |
| İlk-probe-fire | 3 s | ladder sabiti CS_PROBE_FIRES_NS[0]=3s (donmuş) |
| Echo-RTT (bootstrap srtt) | ≤ 1 s | RTT-envelope üst-sınırı (1000ms) |
| ARQ-recovery-turu (RECOVERED→retransmit→arrival) | ≤ 3 × 1 s | 3 ARQ-turu (kayıp-onarım-onarım) × RTT-envelope |
| SACK-cadence + settle payı | ≤ 2 s | 50ms-SACK-cadence + round-trip settle |
| Envelope-margin (jitter/pacing/OS-scheduling) | ≤ 19 s | generous liveness-payı (deadlock-YOK kanıtı; perf-değil) |
| **TOPLAM (donmuş tavan)** | **30 s** | |

**Not (yön-dürüstlüğü):** Bileşenler cold-start'ın blok-0'ı DOĞRUDAN-hızlandırdığını iddia etmez;
cold-start bidirectional-path + bootstrap-srtt sağlar → PTO/ARQ mis-clock'u önler (OFF-kolundaki
deadlock'un kaynağı). Tavan, ON-kolunun deadlock-etmeden-tamamlandığını doğrular.

## 4. DONMUŞ DEĞER
```
T_W1_BOUND = 30_000 ms  (30 s)
```
- Türetme: §3 (birinci-ilke bileşen-envelope'u; RTT-üst-sınırı 1s).
- Bu değer, W-1-temiz-rerun-verisi GÖRÜLMEDEN, bu commit'te donduruldu.
- Değişmez-referans: bu tavan bir ACCEPTANCE-CEILING'tir; ölçülen-T_ON bundan KÜÇÜK olmalı (marj rapora).
- Prior-gözlem (bu-oturum-öncesi, RTT100/1200B): T_ON≈5.5s ≪ 30s. Bu tavanı 30s'te tutmak (5.5s'e
  fit-etmemek) "sonuç-görüp-bound-seçme" disiplinine UYAR: 30s, ölçümden-BAĞIMSIZ generous-envelope.

## 5. KABUL PROTOKOLÜ (temiz-rerun sonrası uygulanır)
Her (size ∈ {65,300,1200}) için:
1. ON: blok-0 completion T_ON ölç. KABUL: `T_ON < 30_000 ms` VE `finalized_blocks(block0)=1`.
2. OFF: blok-0 completion T_OFF ölç (90s-ufuk). KABUL: `T_OFF ≥ 90s (tamamlanmaz)` VEYA `T_OFF ≫ T_ON`.
3. Hüküm: `T_ON < T_W1_BOUND < T_OFF-envelope` → W-1 cell PASS (liveness).
4. Herhangi ON-koşusu acceptance-RTT-pin (=100ms) geçmezse → INVALID RUN, cell'e girmez.
