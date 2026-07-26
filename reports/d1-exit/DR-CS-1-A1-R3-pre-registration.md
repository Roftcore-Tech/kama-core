# DR-CS-1 TUR-A1 — R3 PRE-REGISTRATION (Abkarada DUR-B(2) kararı: A+C hibrit + 3 düzeltme)

> **BAĞLAYICI ÇERÇEVE:** ARQ tüm kollarda aktiftir; DR-CS-1 cold-start LIVENESS DR'ı. **PRODUCTION CODE
> DEĞİŞİKLİĞİ SIFIR.** Bu belge W-2B-R3'ü HERHANGİ BİR R3-KOŞUSU YAPILMADAN ÖNCE dondurur. git-commit-hash +
> author-date = R3 anchor. §5: sonuca-bakıp beklenti/[CAL]/horizon/path-sınıfı değiştirmek YASAK.

**Dayanak:** Abkarada DUR-B(2) kararı (2026-07-26; A+C hibrit, 3-düzeltme) · Harness `4b1c52e`.
**Devralınan-geçerli:** RTT-tol + RX_TEST_PATIENCE (§enstrüman). **HÜKÜMSÜZ:** eski CAP=64-release + T_MAX=30s.

---

## 0. W-2B-R2 → HÜKÜMSÜZ
`SUPERSEDED — NON-INTERFERING BOUNDS REQUIRED.` CAP=64-release ~3.2s'te suppression'ı-bırakıp senaryoyu
partial-starvation'a çeviriyordu; T_MAX=30s < 300B-completion(~61s). A+C ile düzeltildi → **W-2B-R3.**

---

## 1. W-2B-R3 — FULL ACK-STARVATION PROGRESS/COMPLETION CHARACTERIZATION AND COLD-START CAUSALITY

("Recovery-path" tek-başına yetersiz — bazı hücreler recovery-olmadan first-flight-completion gösteriyor;
completion-trigger ile recovery-event AYRIŞTIRILDI.)

### 1.1 Matched kollar (aynı BÜTÜN argümanlar; tek-fark feature)
```
ON:      experimental-cold-start ON  · full matching DATA-ACK/SACK suppression · probe-echo passes
CONTROL: experimental-cold-start OFF · AYNI suppression/workload/RTT/argümanlar
```
**w_units = 8 SABİT + pre-registered** (sonradan-DEĞİŞMEZ). unit_bytes ∈ {65,300,1200} → total_workload
∈ {520,2400,9600}B. tx VE rx birebir `--w-units 8 --unit-bytes <size>` alır (rx-ledger workload/cwnd hesabı için).

### 1.2 Amaç
Doğal-production progress/completion-yolunu ÖLÇMEK (hardcode-yok; yeni/test-only-path → DUR-B) · yeni-path-yok
kanıtı · ON/OFF completion + progress KARŞILAŞTIRMA · cold-start nedensel-marjinal-etki HÜCRE-BAZINDA · transport-
duplicate + app-dedup maliyeti · **workload/cwnd-rejimi birinci-sınıf değişken** (size-etkisi ≠ workload/cwnd-etkisi).

### 1.3 ZORUNLU LEDGER (her koşu; her-iki-build; harness `4b1c52e` emit eder)
```
w2b_feature_state                          (ON|OFF)
configured_rtt / measured_rtt_p50/p95      (per-cell RTT-realization)
w2b_workload_units · w2b_unit_bytes · w2b_total_workload_bytes
w2b_effective_initial_cwnd_bytes           (AMPİRİK = first_flight_burst_units × unit_bytes;
                                            first_flight_burst = 1500ms-stall-gap ÖNCESİ block-0 yeni-source-shard)
w2b_first_flight_burst_units · w2b_workload_to_initial_cwnd_ratio
w2b_completion_trigger_kind                (block-0 TAMAMLAYAN shard'ın kind'ı)
w2b_completion_time
w2b_first_recovery_event_kind / _time      (block-0 completion-ÖNCESİ İLK recovery-event: 0x01/0x02/0x03)
w2b_recovery_events_before_completion_count
w2b_recovery_contributed_to_completion     (bool)
w2b_recovery_path                          (YALNIZ gerçek-recovery-gözlendiyse; aksi "none")
w2b_suppressed_ack_count (SATURATING) · w2b_ack_cap_hard_hit
w2b_transport_duplicate_rx_count · w2b_application_delivery_count · w2b_application_duplicate_delivery_count
```
**completion_trigger_kind izinli sınıfları:** `FirstFlight · ExistingPtoArq · ForwardRecoveryDup ·
OtherExistingProductionKind[kind] · NoCompletionWithinHorizon`. (RecoveredControl = tx-side-notion; rx-completion
DAİMA DATA-shard-kind'ıyla olur — bu ölçüm rx-completion'ı verir; yeni-DATA-wire-kind görülürse DUR-B.)

### 1.4 A+C BOUNDS (donmuş)
- **CAP=64 release: KALDIRILDI.** `suppressed_ack_count` = SATURATING diagnostic (release-tetiği-DEĞİL).
- **ACK_CAP_HARD = 6000** (runaway-güvenlik; release-DEĞİL; aşımda `ack_cap_hard_hit=true` → run **INVALID → DUR-B**).
  Formül: `ceil(GLOBAL_WATCHDOG_MS/SACK_CADENCE_MS)×2 = ceil(150000/50)×2 = 6000`. Normal-hücrede-erişilemez
  (en-uzun-horizon 85s → ~1700 suppressed).
- **T_MAX=30s: EMEKLİ.**
- **CELL_OBSERVATION_HORIZON(size)** (formül-türevi; floor_rate=38 B/s pilot-türevi):
  ```
  H(size) = first_flight_dispatch_bound(size) + RTT_MAX_CELL(1s) + drain_margin(≈10s)
  first_flight_dispatch_bound = total_workload_bytes / floor_rate
    65B  : 520/38  ≈ 13.7s → H = 30s
    300B : 2400/38 ≈ 63.2s → H = 85s
    1200B: 9600/38 ≈ 252.6s → formül ~264s; RUNTIME-CAP = 45s (Abkarada-izinli)
  ```
- **GLOBAL_HARNESS_WATCHDOG(size)** = H(size) + 15s (yalnız runaway-koruması; > cell-horizon).
- **1200B özel:** 45s runtime-cap'te tamamlanmazsa hüküm YALNIZ `NO_COMPLETION_BY_45S` (mutlak-NoRecovery-DEĞİL).

### 1.5 MATRİS (54 koşu)
`RTT{100,250,1000} × size{65,300,1200} × feature{ON,OFF} × 3 = 54.` Her-satır enstrüman-zinciri (acceptance-
RTT-pin + per-cell-RTT-realization + overlap-gate + binary-provenance + arg-paritesi[feature-hariç] + clock-domain).

### 1.6 HÜKÜM — HÜCRE BAZINDA (sonuç sınıfları)
```
DECISIVE                       (OFF NoCompletion/geç ∧ ON completion/erken; cold-start nedensel-belirleyici)
BENEFICIAL                     (ON anlamlı-daha-iyi ama OFF de sonunda tamamlar)
NEUTRAL                        (ON≈OFF)
NO_COMPLETION_WITHIN_HORIZON   (her-iki-kol da horizon-içinde-tamamlamaz; [EXACT horizon])
INVALID — NO EVIDENCE          (enstrüman-zinciri-kırık VEYA ack_cap_hard_hit)
```
**ON≈OFF ise cold-start-lehine-PASS-YAZILMAZ → "natural progress/completion (workload/cwnd-governed)" raporlanır.**

---

## 2. W-3-65 (B1) — KAPANDI (bu belgeden bağımsız)
`W-3-65 = 3/3 PASS` (`a8b50e7`; RecoveredByForwardRecoveryControl, PostTimeoutRecovery-yok). R3-değişikliği
W-3-65'i ETKİLEMEZ. A2 açık-madde: POST-TIMEOUT RECOVERED-CONTROL NOTIFICATION CONTRACT.

---

## 3. A1-KAPANIŞ SEMANTİĞİ (Abkarada)
Sender-side-FULL, cold-start'ın W-2B'de FAYDALI-çıkmasını GEREKTİRMEZ. FULL =
"full-starvation-sınır-koşulu deterministik-karakterize-edildi + doğal-production-path'ler-ayrıştırıldı +
ON/OFF-marjinal-etki-dürüstçe-ölçüldü + uygulama-doğruluğu-korundu." (Tam-matris ON≈OFF-çıkarsa W-2B-hükmü
"under total ACK starvation cold-start is neutral; progress governed by floor-rate first-flight/forward-recovery"
— sınır-bölgesi-haritalandı; W-1'in-decisive-bölgesi + bu-nötr-bölge birlikte güçlü-sonuç.)

## FREEZE-BEYANI
Bu belge 54-koşu YAPILMADAN ÖNCE commit edilir. git-commit-hash + author-date = R3 anchor. Değişiklik gerekirse
→ YENİ pre-registration + DUR-B.
