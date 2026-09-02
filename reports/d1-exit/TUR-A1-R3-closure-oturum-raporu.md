# TUR A1 — R3 CONFIRMATORY + SENDER-SIDE CLOSURE — OTURUM SONUÇ RAPORU

**Abkarada A+C kararı · 3 düzeltme · W-2B-R3 54-koşu confirmatory · pilot-overclaim tersine-dönüşü ·
adversarial-verify · A1 SENDER-SIDE FULL**

> **BAĞLAYICI ÇERÇEVE:** Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün deney kollarında aktiftir; aynı
> ACK/NACK/PTO/retransmit semantiğini kullanır. DR-CS-1 bir **cold-start LIVENESS/bootstrap** DR'ıdır; ölçülen
> şey cold-start'ın nedensel-marjinal etkisidir. "ARQ FEC'i yendi / FEC ARQ'dan iyi" türü ifadeler YASAKTIR.

**Tarih:** 2026-07-26 · **Branch:** `d1-sprint5c` · **PR:** #44 (DRAFT) · **Commit aralığı:** `4b1c52e..1d50c6e`
**Üretim-etkisi: SIFIR** (harness/test-only + doc; `crates/*/src` mekanizma-yolu DEĞİŞMEDİ; flag-OFF byte-özdeş).

---

## 0. YÖNETİCİ ÖZETİ

Bu tur, önceki turun **ikinci DUR-B'sine (W-2B-R2 [CAL]-bounds çelişkisi)** Abkarada'nın verdiği **A+C hibrit
kararını + üç düzeltmesini** uygulamak ve **W-2B-R3 confirmatory 54-koşu matrisini** koşmakla geçti. Sonuç,
önceki turun **pilot aşırı-hükmünü ("cold-start nötr") TERSİNE ÇEVİRDİ**: tam ON/OFF matched matris cold-start'ın
**300B'de DECISIVE** olduğunu ortaya çıkardı. TUR-A1 sender-side **FULL** hükmüyle kapandı.

### Ana sonuç tablosu
| Kalem | Sonuç |
|---|---|
| Abkarada A+C kararı + 3 düzeltme | ✅ uygulandı (harness `4b1c52e`) |
| R2 SUPERSEDED + R3 pre-reg freeze | ✅ anchor `70ea2de` (koşulardan-ÖNCE) |
| **W-2B-R3 54-koşu** | ✅ TÜM-VALID: **65B-NEUTRAL · 300B-DECISIVE · 1200B-NO_COMPLETION@45s** |
| **Pilot-overclaim** | ✅ confirmatory-matrisle **ÇÜRÜTÜLDÜ** (300B decisive; nötr-değil) |
| Adversarial-verify | ✅ **0/6 refuted** (1200B control-vs-data nüansı doğrulandı) |
| **DR-CS-1 SENDER-SIDE ACCEPTANCE** | ✅ **FULL** |

---

## 1. GİRDİ — ABKARADA A+C KARARI + 3 DÜZELTME

Önceki tur iki gerçek pre-reg/gerçeklik çelişkisi bulup DUR-B'ye taşımıştı. İkinci DUR-B'ye (W-2B-R2 [CAL]-bounds)
Abkarada kararı: **A+C hibrit** + üç düzeltme + "bunlar olmadan 54 koşuya geçilmemeli":
1. **Overclaim:** "cold-start nötr" hükmü pilot-only (RTT100/w8); confirmatory-54-matris-pending.
2. **recovery_path SPLIT:** completion_trigger_kind ≠ recovery_path (first-flight-completion recovery-değil).
3. **workload/cwnd birinci-sınıf değişken:** size-etkisi ≠ workload/cwnd-rejim-etkisi (W-1 dersi).
+ **A+C bounds:** CAP=64-release RED (→W-2C'ye çevirir); T_MAX-emekli; formül-türevi per-cell-horizon;
ACK_CAP_HARD saf-güvenlik (abort→INVALID, release-değil).

---

## 2. ÜÇ DÜZELTME + A+C BOUNDS (harness `4b1c52e`; production-code SIFIR)

### 2.1 Düzeltme-2: TRIGGER/RECOVERY AYRIMI
`recovery_path` (block-0-tamamlayan-shard'ın-wire-kind'ı) tek-başına AŞIRI-GENELLEŞTİRMEYDİ. Ayrıldı:
```
completion_trigger_kind   (FirstFlight/ExistingPtoArq/ForwardRecoveryDup/OtherKind/NoCompletionWithinHorizon)
first_recovery_event_kind / _time        (block-0 completion-öncesi İLK re-send: 0x01/0x02/0x03)
recovery_events_before_completion_count
recovery_contributed_to_completion       (bool)
recovery_path = "none"  UNLESS gerçek-recovery-gözlendi
```

### 2.2 Düzeltme-3: WORKLOAD/EMPIRICAL-DISPATCH LEDGER (Abkarada-düzeltme-2 isimlendirme dahil)
```
workload_units · unit_bytes · total_workload_bytes
empirical_initial_dispatch_bytes = first_flight_burst_units × unit_bytes  (AMPİRİK: 1500ms-stall-gap öncesi
                                   block-0 yeni-source-shard sayısı; PROTOKOL-cwnd DEĞİL — W-1'in "cwnd"iyle
                                   KARIŞTIRILMAZ; sender-diff GEREKMEZ)
workload_to_effective_dispatch_ratio
```
**Kritik gereksinim:** rx VE tx birebir `--w-units/--unit-bytes` alır (aksi rx-default w=80/unit=1200 → yanlış-oran).
**Not (frozen-evidence):** R3-marker'ları eski-etiketi `w2b_effective_initial_cwnd_bytes` taşır = AYNI-değer,
yeniden-yorumla `empirical_initial_dispatch_bytes` (Abkarada-düzeltme-2). Harness-adı düzeltildi; re-run-gerekmez.

### 2.3 A+C BOUNDS
- **CAP=64-release KALDIRILDI** (senaryoyu ~3.2s'te partial-starvation'a çeviriyordu). `suppressed_ack` =
  SATURATING-diagnostic.
- **ACK_CAP_HARD=6000** (formül `ceil(150000/50)×2`; runaway-güvenlik; aşımda `ack_cap_hard_hit=true` → INVALID;
  release-DEĞİL). **Hiçbir hücrede tetiklenmedi.**
- **T_MAX=30s EMEKLİ.** **CELL_OBSERVATION_HORIZON(size)** formül-türevi (floor_rate=38B/s pilot): 65B=30s,
  300B=85s, 1200B=45s(runtime-cap→NO_COMPLETION_BY_45S). GLOBAL_WATCHDOG=H+15s.

---

## 3. R2 SUPERSEDED + R3 FREEZE (koşulardan ÖNCE)

- **W-2B-R2** → `SUPERSEDED — NON-INTERFERING BOUNDS REQUIRED` (arşiv; sonuç-değişmez).
- **W-2B-R3 pre-registration DONDU** — anchor **`70ea2de` @ 19:34:57**, sha256 `bff6bf9…`. İçerik: matched-arm
  ON/OFF, 54-koşu, izinli-completion-trigger-sınıfları, workload/cwnd-ledger, A+C-bounds, sonuç-sınıfları
  (DECISIVE/BENEFICIAL/NEUTRAL/NO_COMPLETION_WITHIN_HORIZON/INVALID), **ON≈OFF→cold-start-PASS-yazılmaz**, w_units=8-SABİT.

---

## 4. W-2B-R3 — 54-KOŞU CONFIRMATORY SONUÇ (TÜM-VALID)

`RTT{100,250,1000} × size{65,300,1200} × feature{ON,OFF} × 3rep`. Her-koşu enstrüman-zinciri geçti
(acceptance-RTT-pin=100 · per-cell RTT-realization p50/p95 · overlap-gate · arg-paritesi · clock-domain ·
ack_cap_hard=false · app_dup=0).

| size | verdict (3-RTT tutarlı) | ON (recv / comp) | OFF (recv / comp) | ratio / ff-burst |
|---|---|---|---|---|
| **65B** | **NEUTRAL** | 9 / ~14.1–15.0s | 9 / ~14.7s (Δ 2–4%) | 8.0 / 1 |
| **300B** | **DECISIVE** | 9 / **~61–62s** ✔ | **1 / DEADLOCK** ✗ | 8.0 / 1 |
| **1200B** | **NO_COMPLETION@45s** | **3 distinct-shard / no-block** | **1 / DEADLOCK** | 8.0 / 1 |

**Verdict dağılımı:** 3× NEUTRAL, 3× DECISIVE, 3× NO_COMPLETION_WITHIN_HORIZON. **acceptance: TÜM-VALID.**

---

## 5. ANA HİKAYE — PİLOT OVERCLAIM'İN TERSİNE DÖNÜŞÜ

Önceki tur izole-pilot (yalnız-ON kolu; **300B-OFF test-EDİLMEMİŞTİ**) "cold-start full-starvation'da nötr"
iddiasına yol açmıştı. Abkarada bunu overclaim olarak işaretleyip confirmatory-matris şart koştu.

**Confirmatory ON/OFF-matched matris bu iddiayı ÇÜRÜTTÜ:**
- **300B-OFF: recv=1, deadlock** (3-RTT × 3-rep hepsi) — gönderen İLK-paketten sonra durur.
- **300B-ON: recv=9, @~61s tamamlar** (3-RTT × 3-rep hepsi).
- → cold-start **300B'de DECISIVE** (nötr-DEĞİL).

Pilot-mekanizma-açıklamam da ("cold-start-nötr-çünkü-BtlBw-ACK-gerektirir") YANLIŞTI. **Doğru mekanizma:**
cold-start'ın liveness-faydası **BtlBw-değil PreConverge floor-dispatch** üzerinden — TOTAL-ACK-starvation'da-BİLE
decisive. **Abkarada'nın confirmatory-matris-ısrarı doğrulandı.**

---

## 6. MEKANİSTİK YORUM (confirmatory)

- **empirical_initial_dispatch ≈ 1 UNIT** (ff_burst=1; ratio=w_units=8) TÜM boyutlarda — **protokol-cwnd DEĞİL**
  (ampirik-ilk-dispatch; W-1-cwnd'iyle-karıştırılmaz). ff-burst UNIT-sabit; fark
  BYTE-tabanlı floor-dispatch-hızından + OFF-deadlock'tan gelir (workload/cwnd-rejimi Abkarada-düzeltme-3'le açığa-çıktı).
- **65B NEUTRAL:** küçük-workload (520B) → OFF-de floor-dispatch'le @~14.7s tamamlar → cold-start-gerekmez.
- **300B DECISIVE:** OFF (no-PreConverge-floor) 1-pakette DEADLOCK; ON (cold-start PreConverge-floor) tüm-8-birim
  dispatch → @~61s tamamlar. completion_trigger=FirstFlight (recovery-DEĞİL; sürdürülen-dispatch) · recovery_path=none.
- **1200B NO_COMPLETION@45s (ikisi de):** app_delivery=0. **Doğrulanmış DATA-plane partial-progress asimetrisi:**
  ON **3 distinct source-shard** (transport_dup=0; floor-dispatch-liveness), OFF **1** (deadlock). Control-plane-DEĞİL
  (probe receiver_events'e-sayılmaz) ama **block-completion-DA-DEĞİL** (kısmi 3/8). Nihai-kayıt: *"cold-start produces
  additional source progress, but completion benefit is not demonstrated within 45 seconds."* DECISIVE-vs-NEUTRAL
  45s-cap'te ÇÖZÜLMEDİ. YASAK: 1200B-decisive/liveness-solved/recovery-impossible/FEC-ARQ-hükmü.

> **MEKANİZMA-İNCELTMESİ (Abkarada; freeze-öncesi güçlendirme):** "cold-start-ON BtlBw-oluşmadan-da source-dispatch'in-
> devam-etmesini-sağlar; observed-completion first-flight-source-dispatch-iledir (recovery-DEĞİL)." **"PreConverge
> floor-dispatch" KESİN-kök-neden-olarak-mühürlenmeden-ÖNCE** `state_at_each_source_wire_emission / srtt / pacing_rate
> / dispatch_authority` state-at-emission-ledger'ıyla-doğrulanmalı (A1-blocker-DEĞİL; freeze-mekanizma-açıklamasını sağlamlaştırır).

---

## 7. ADVERSARIAL-VERIFY (6 bağımsız-skeptik; 0/6 REFUTED)

Tüm 6 ana-iddia AYAKTA. En değerli non-refuting bulgu (skeptik-titizliğinin kanıtı):
- **1200B-PROGRESS caveat:** skeptik "recv-farkı probe/echo (control-plane)" dedi → DOĞRULAMA: receiver_events
  YALNIZ-DATA-sayar (probe 0x19 `continue`; ev_count'a-girmez) + `transport_dup=0` → **3 recv = 3 DISTINCT
  source-shard** (DATA-plane). AMA skeptik-uyarısı-geçerli: bu **block-completion-DEĞİL** (app_delivery=0). §closure
  hassaslaştırıldı. Bu "partial-progress ≠ completion" ayrımı adversarial-verify sayesinde netleşti.
- Diğer 5: DECISIVE-300B (kesin), ARG-PARITY (script feature-bağımsızlığı), TRIGGER-RECOVERY-SPLIT (guard'lı),
  WORKLOAD-CWND (ham-alanlar doğrular), ACCEPTANCE-BOUNDS (suppression_release completion-SONRASI, CAP-release-değil).

---

## 8. HÜKÜM — DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL — SCOPE-LIMITED CHARACTERIZATION COMPLETE

> - **W-1:** cold-start DECISIVE (workload>initial-cwnd + ACK feedback; 300/1200B).
> - **W-2B-R3 (TOTAL ACK starvation):** **300B DECISIVE** (OFF-deadlock recv=1, ON @~61–62s PreConverge-floor) ·
>   **65B NEUTRAL** (ikisi tamamlar) · **1200B NO_COMPLETION@45s ikisi** (app_delivery=0; DATA-plane partial-progress
>   ON-3/OFF-1; DECISIVE-vs-NEUTRAL 45s'te çözülmedi). completion = sürdürülen first-flight-dispatch (recovery_path=none).
> - **W-3-65:** heal 3/3 (RecoveredByForwardRecoveryControl; PostTimeoutRecovery-yok-65B).
> - **H1:** clean.

**FULL gerekçesi (Abkarada-semantiği):** full-starvation-sınır-koşulu deterministik-karakterize-edildi +
doğal-production-path'ler-ayrıştırıldı + ON/OFF-marjinal-etki-dürüstçe-ölçüldü + uygulama-doğruluğu-korundu.
Cold-start'ın faydası **workload-rejimine-bağlı**: `300B/w8 DECISIVE (kanıtlı)` · `65B/w8 NEUTRAL (kanıtlı)` ·
`1200B/w8 completion-UNRESOLVED@45s (partial-progress-var)`. **"Faydalı-bölge 65B-üzeri" GENELLEMESİ KULLANILMAZ**
(300–1200B-arası ölçülmedi). Hüküm: `FULL — SCOPE-LIMITED CHARACTERIZATION COMPLETE`.
**AÇIK (A2):** ürün-alıcı-patience/resource + POST-TIMEOUT RECOVERED-CONTROL NOTIFICATION CONTRACT. Merge/flip-YASAK.

---

## 9. COMMIT HARİTASI (bu tur)

| Commit | İçerik |
|---|---|
| `70ea2de` | R2 SUPERSEDED + R3 pre-registration FREEZE + overclaim-fix (koşulardan ÖNCE) |
| `4b1c52e` | R3 harness (trigger/recovery-split + workload/cwnd-ledger + A+C-bounds; production-code-SIFIR) |
| `96c6e7a` | R3 + W-3-65 orkestratörleri (size-scaled-horizon; rx --w-units/--unit-bytes; drain) |
| `bab30c8` | R3 per-cell verdict analyzer |
| `1d50c6e` | **SENDER-SIDE CLOSURE FULL** + evidence + status-board + memory |

Testler: dr_cs1_acceptance 22 + dr_cs1_wire 12 + cold_start_liveness 4 + udp_runner_smoke 5 yeşil; fmt/clippy temiz.
Push: `a056b8b..1d50c6e → origin/d1-sprint5c`.

---

## 10. DERSLER

1. **Pilot ≠ confirmatory.** Tek-kollu-pilot (300B-OFF-test-edilmemiş) "nötr" overclaim'ine yol açtı; confirmatory
   ON/OFF-matched matris **300B-DECISIVE** ortaya çıkardı. **Abkarada'nın matris-ısrarı bilimsel-olarak-haklıydı.**
2. **Devralınan [CAL] otomatik-geçerli-değil.** Eski-hipotez-bound'ları (CAP=64/T_MAX=30s) yeni-senaryoyla
   çeliştiğinde körlemesine-taşımak ölçümü-bozar (partial-starvation'a-çevirir). A+C ile düzeltildi.
3. **Alan-isimlendirme mekanizmayı-doğru-yansıtmalı.** `recovery_path`-tek-alan aşırı-genelleştirmeydi; completion_
   trigger vs recovery-event ayrımı gerçeği-doğru-taşıdı (first-flight-completion recovery-DEĞİL).
4. **workload/dispatch birinci-sınıf.** empirical-initial-dispatch≈1-unit (protokol-cwnd-DEĞİL; Abkarada-düzeltme-2) + ratio=w_units — size-etkisini workload/dispatch-rejim-
   etkisinden ayırdı; hem-ölçüm-hem-yorum netleşti.
5. **Adversarial-verify interpretation-nüanslarını yakalar.** 1200B "control-plane-mı-data-plane-mı" nüansı
   skeptikle netleşti (`transport_dup=0` → data-plane-partial ama completion-değil). → daha-dürüst-closure.
6. **İki-DUR-B + confirmatory-düzeltme = disiplin çalışıyor.** Ölçüm pre-registration'ı iki-kez-çürüttü, dürüstçe-
   DUR-B'ye-taşındı, kararla-çözüldü; pilot-overclaim confirmatory'yle-düzeltildi. Üretim-koduna hiç-dokunulmadı.
