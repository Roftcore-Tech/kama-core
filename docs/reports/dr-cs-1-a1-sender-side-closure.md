# DR-CS-1 TUR-A1 — SENDER-SIDE CLOSURE

> **BAĞLAYICI ÇERÇEVE:** ARQ tüm kollarda aktiftir; DR-CS-1 cold-start LIVENESS DR'ı. **PRODUCTION CODE
> DEĞİŞİKLİĞİ SIFIR** (harness/test-only). Bu ARQ-vs-FEC karşılaştırması DEĞİL — cold-start'ın nedensel
> marjinal etkisinin ölçümüdür.

**Tarih:** 2026-07-26 · **Branch:** `d1-sprint5c` · **PR:** #44 (DRAFT — merge/flip YASAK)
**Pre-registration anchor'ları:** F-A1-0 `a635efa` · R2(SUPERSEDED) `fe119b4` · **R3 `70ea2de`** (sha256 `bff6bf9…`).
**Harness:** `4b1c52e` (A+C bounds + trigger/recovery-split + workload/cwnd-ledger; production-code-SIFIR).

---

## 0. YÖNETİCİ ÖZETİ

TUR-A1'in kalan sender-side hücreleri kapandı. **İki gerçek pre-registration/gerçeklik çelişkisi** faithful-
ölçümle ortaya çıktı (recovery-path + [CAL]-bounds), her ikisi de dürüstçe DUR-B'ye taşındı ve Abkarada
kararıyla çözüldü. **Pilot bir aşırı-hüküm ("cold-start nötr") confirmatory-matrisle DÜZELTİLDİ** — tam
ON/OFF matched matris cold-start'ın 300B'de **DECISIVE** olduğunu ortaya çıkardı.

### Hücre sonuçları
| Hücre | Sonuç | Kanıt |
|---|---|---|
| **W-1** (OFF-vs-ON) | cold-start DECISIVE (workload>initial-cwnd) | F4: ON blok-0 2.7–5.6s; OFF deadlock (300/1200B) |
| **W-2B-R3** (full ACK-starvation) | **65B NEUTRAL · 300B DECISIVE · 1200B NO_COMPLETION@45s (progress-asimetrik)** | 54-koşu VALID (§3) |
| **W-3-65** (post-blackout heal) | **3/3 PASS** | RecoveredByForwardRecoveryControl; PostTimeoutRecovery-yok |
| **DI-2B** (reverse-blackout) | PASS (F4) | probe-reach + echo-lost + TimedOut@60s |
| **DI-10 / mixed-version** | TAMAM (F4) | g10-g10d + g9 |
| **H1-desen denetimi** | **CLEAN** | 14-script semantik; tek-tarihsel a6d199b-fixed |

---

## 1. METODOLOJİ + ENSTRÜMAN ZİNCİRİ
Faithful two-netns wall-clock (production-code-SIFIR). Her R3-koşusu: acceptance-RTT-pin(=100ms) · per-cell
RTT-realization (p50/p95 tol) · overlap-gate (receiver_events>0) · binary-provenance (bin_cs≠bin_off) ·
arg-paritesi (feature-hariç birebir) · clock-domain (tek-host MONOTONIC) · inter-run-drain (BUG-H3). A+C
bounds: CAP-release-KALDIRILDI (suppressed_ack saturating-diagnostic; ACK_CAP_HARD=6000 runaway→INVALID,
release-DEĞİL — hiçbir hücrede tetiklenmedi); T_MAX-emekli; formül-türevi CELL_OBSERVATION_HORIZON (30/85/45s).

---

## 2. İKİ DUR-B (dürüst süreç)
- **DUR-B(1) recovery-path:** §9-ARQ SACK-reaktif → full-suppression'da ateşleyemez; motor §11 forward-recovery
  (ACK-bağımsız) kullanır. Pre-registered `ExistingPtoArq` mekanistik-imkânsız. → Abkarada **A3+B1**.
- **DUR-B(2) [CAL]-bounds:** devralınan CAP=64-release suppression'ı ~3.2s'te bırakıp senaryoyu partial-
  starvation'a çeviriyordu. → Abkarada **A+C hibrit** (CAP-release-red, formül-türevi-horizon, trigger/recovery-
  split, workload/cwnd-ledger). → **W-2B-R3**.

---

## 3. W-2B-R3 — CONFIRMATORY SONUÇ (54 koşu; TÜM-VALID)

**⚠ PİLOT DÜZELTMESİ:** İzole-pilot (yalnız-ON, 300B-OFF-test-EDİLMEDİ) "cold-start nötr" iddiasına yol açmıştı.
**Confirmatory ON/OFF-matched matris bunu ÇÜRÜTTÜ.** Abkarada'nın confirmatory-matris-ısrarı doğrulandı.

| size | verdict | ON (recv/comp) | OFF (recv/comp) | trigger | ratio/ff |
|---|---|---|---|---|---|
| **65B** | **NEUTRAL** (3 RTT) | 9 / ~14–15s | 9 / ~14.7s | FirstFlight | 8.0 / 1 |
| **300B** | **DECISIVE** (3 RTT) | 9 / **~61s** ✔ | **1 / deadlock** ✗ | FirstFlight | 8.0 / 1 |
| **1200B** | **NO_COMPLETION@45s** (3 RTT) | **3 / progress** | **1 / deadlock** | NoCompletionWithinHorizon | 8.0 / 1 |

**Mekanistik yorum (doğru):**
- **workload/cwnd:** effective_initial_cwnd ≈ **1 unit** (ff=1; ratio=w_units=8) TÜM boyutlarda. Yani cwnd
  UNIT-sabit; fark BYTE-tabanlı dispatch-hızından + OFF-deadlock'tan gelir.
- **65B NEUTRAL:** küçük-workload → OFF-de floor-dispatch'le tamamlar (recv=9) → cold-start-gerekmez.
- **300B DECISIVE:** OFF 1-pakette DEADLOCK (recv=1; BtlBw-yok + PreConverge-floor-yok); ON cold-start'ın
  **PreConverge floor-dispatch**'iyle tüm-birimleri-gönderir → @~61s tamamlar. **Cold-start'ın nedensel-
  liveness-faydası.** (recovery_path=none; completion_trigger=FirstFlight — yani recovery-DEĞİL, sürdürülen-
  dispatch. Cold-start ARQ-substratını değiştirmez; dispatch-liveness'ini korur.)
- **1200B:** 45s-cap'te İKİSİ DE blok-tamamlamaz (app_delivery=0; NO_COMPLETION_BY_45S — mutlak-NoRecovery-DEĞİL,
  formül ~264s). **DATA-plane PARTIAL-progress asimetrisi (doğrulandı; adversarial §5):** ON **3 DISTINCT
  source-shard** alır (transport_dup=0; floor-dispatch-liveness), OFF **1** (deadlock). Bu control-plane-DEĞİL
  (probe receiver_events'e-sayılmaz) ama **blok-completion-DA-DEĞİL** (kısmi 3/8-birim). → cold-start-liveness
  1200B'de-de partial-progress-sağlar; DECISIVE-vs-NEUTRAL 45s-cap'te ÇÖZÜLMEDİ (300B-deseni ~264s'te DECISIVE-
  öngörür ama bu-turda-ölçülmedi).

**Sınır-bulgusu (düzeltilmiş):** Pilot "cold-start-nötr-mekanizması-BtlBw-gerektirir" da YANLIŞTI. Confirmatory:
cold-start'ın liveness-faydası **BtlBw-değil PreConverge-floor-dispatch** üzerinden — TOTAL-ACK-starvation'da-BİLE
decisive (300B). Cold-start yalnız faydalı-bölgede-değil, **faydasız-sınır-bölgesi (65B) de haritalandı.**

---

## 4. W-3-65 (B1) — 3/3 PASS (kapandı)
finalized=1 · TimedOut-tek-sefer · RecoveredByForwardRecoveryControl · PostTimeoutRecovery-yok (65B; DATA-ACK-
kaynaklı 300/1200B için korunur) · app_dup=0 · no-busy-loop · no-second-timeout. F4-UNOBSERVED çözüldü.

---

## 5. ADVERSARIAL-VERIFY (6 bağımsız-skeptik; 0/6 REFUTED — tüm-bulgular AYAKTA)

| İddia | Refuted? | Bulgu / çözüm |
|---|---|---|
| DECISIVE-300B | HAYIR | OFF 3-RTT×3-rep DEADLOCK (recv=1/fin=0); ON tamamlar (61.16–62.07s; "~61s"→"~61–62s" düzeltildi). Contrast HER hücrede kesin. |
| ARG-PARITY | HAYIR | config-diff + **runner-script** feature-bağımsızlığı (suppress-data-acks/horizon/no-settle-exit `$feat`-gated-DEĞİL, HER-İKİ-kola uygulanır) → starvation-matched. Non-feature-arg-farkı YOK. |
| TRIGGER-RECOVERY-SPLIT | HAYIR | completion_trigger wire-kind-byte'a-güvenir (yeni-DATA-kind→DUR-B guard'ı + new_retransmit_path_count=0 + floor-rate-dispatch-zamanı recovery-burst'ü-dışlar). Adreslendi. |
| WORKLOAD-CWND | HAYIR | ratio unit_bytes'ı-sadeleştirir AMA ham w2b_unit_bytes/total_workload_bytes bağımsız-doğrular. total_workload=configured (delivered-değil; iddia configured-diyor). |
| 1200B-PROGRESS | HAYIR (caveat-DOĞRULANDI) | **KRİTİK NÜANS:** skeptik "+2 probe/echo" dedi — DOĞRULAMA: receiver_events YALNIZ-DATA sayar (probe 0x19 line-278'de `continue`; ev_count'a-girmez); **transport_dup=0 → 3 recv = 3 DISTINCT source-shard** (DATA-plane partial-progress, probe-DEĞİL). AMA skeptik-uyarısı-geçerli: bu **blok-completion-DEĞİL** (app_delivery=0 ikisi-de). §3/§6 hassaslaştırıldı. |
| ACCEPTANCE-BOUNDS | HAYIR | ack_cap_hard=false TÜM-hücre; app_dup=0 TÜM; recv>0 TÜM; RTT-tol-PASS. suppression_release 27/54'te (yalnız-tamamlayan-hücreler) ateşler ama completion'dan 2–9s SONRA → starvation causal-pencerede-korundu (CAP-release-DEĞİL). |

**Sonuç:** 0/6-refuted. Skeptik-1200B-nüansı DOĞRULANDI + hassaslaştırıldı (data-plane-partial-progress ≠ completion).
Diğer-concern'ler adreslendi (interpretation-nuance/proof-strength; refute-DEĞİL).

---

## 6. HÜKÜM

> ## **DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL**
> - **W-1:** cold-start DECISIVE when workload > initial-cwnd AND some ACK feedback exists (300/1200B).
> - **W-2B (R3, TOTAL ACK starvation):** cold-start is **DECISIVE at 300B** (OFF deadlocks after 1 packet;
>   ON completes @~61–62s via PreConverge floor-dispatch) — **NOT neutral**; **NEUTRAL at 65B** (both complete,
>   workload fits); **1200B NO_COMPLETION within 45s cap for BOTH** (app_delivery=0) with a verified **DATA-plane
>   partial-progress asymmetry** (ON 3 distinct source-shards, transport_dup=0; OFF 1, deadlock) — cold-start
>   liveness partial-progresses even here, but completion (DECISIVE-vs-NEUTRAL) is UNRESOLVED at the 45s cap.
>   Completion path = sustained first-flight dispatch (recovery_path=none; no recovery re-send).
> - **W-3:** post-blackout healing validated (3/3).
> - **H1:** clean.

**FULL gerekçesi (Abkarada-semantiği):** full-starvation-sınır-koşulu deterministik-karakterize-edildi +
doğal-production-path'ler-ayrıştırıldı (completion_trigger vs recovery-event) + ON/OFF-marjinal-etki-dürüstçe-
ölçüldü (65B-neutral / 300B-decisive / 1200B-progress) + uygulama-doğruluğu-korundu (app_dup=0 tüm-hücre).
Cold-start'ın hem **faydalı-bölgesi** hem **faydasız-sınırı** haritalandı.

**AÇIK (A2):** ürün-alıcı-patience/resource + **POST-TIMEOUT RECOVERED-CONTROL NOTIFICATION CONTRACT** (ürün-
katmanının ACK-bağımsız post-timeout-recovery'yi nasıl-öğrendiği). End-to-end-product-FULL A2-öncesi iddia-EDİLMEZ.
Merge/default-flip YASAK.

---

## 7. STATUS-BOARD GÜNCELLEME ÖNERİSİ
```
W-2B faithful ACK-starvation  → GO  (R3: 65B-NEUTRAL/300B-DECISIVE/1200B-progress; kapandı)
W-3 restore=61 @ 65B          → GO  (3/3 PASS)
A1 sender-side acceptance      → FULL
A2 (product receiver)          → NO-GO (sıradaki; + POST-TIMEOUT RECOVERED-CONTROL NOTIFICATION CONTRACT)
```

## 8. DUR-SON

**DUR-SON — DR-CS-1 TUR-A1 SENDER-SIDE TAMAMLANDI (FULL).**

- **YAPILAN:** A1 kalan-hücreleri (W-2B-R3 54-koşu confirmatory + W-3-65 3/3 + H1-CLEAN) kapandı. İki gerçek
  pre-reg/gerçeklik-çelişkisi (recovery-path + [CAL]-bounds) faithful-ölçümle-bulundu, dürüstçe-DUR-B'ye-taşındı,
  Abkarada-kararıyla (A3+B1, A+C-hibrit) çözüldü. **Pilot-overclaim confirmatory-matrisle DÜZELTİLDİ** →
  cold-start 300B'de DECISIVE (nötr-DEĞİL). Adversarial-verify 0/6-refuted (1200B-nüansı doğrulandı+hassaslaştırıldı).
- **HÜKÜM:** `DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL` — W-1 decisive(workload>cwnd+ACK) · W-2B-R3
  {65B-NEUTRAL, 300B-DECISIVE, 1200B-NO_COMPLETION@45s+partial-progress} · W-3 heal-3/3 · H1-clean.
- **ÜRETİM-ETKİSİ:** SIFIR (harness/test-only; flag-OFF byte-özdeş; clippy+pin yeşil; merge/flip-YASAK).
- **KANIT:** faithful two-netns; 54+3-koşu; her-koşu VALID (acceptance-RTT-pin + overlap-gate + arg-paritesi +
  clock-domain + ack_cap_hard=false + app_dup=0); pre-reg-anchor'ları donmuş (a635efa/70ea2de); adversarial-0/6.
- **AÇIK/DEVİR (A2 — sıradaki):** ürün-alıcı patience/resource (iki-seviye-cap) + **POST-TIMEOUT RECOVERED-CONTROL
  NOTIFICATION CONTRACT** (ACK-bağımsız-post-timeout-recovery ürün-öğrenimi). 1200B DECISIVE-vs-NEUTRAL çözümü
  (uzun-horizon; opsiyonel-follow-up). End-to-end-product-FULL A2-öncesi iddia-EDİLMEZ.
- **KARAR-BEKLEYEN:** A2-talimatı (A1-DUR-SON-sonrası yazılır); default-flip YALNIZ A2+rollout+§4.4-benchmark-sonrası.
- **DUR:** onay-bekliyor — PR#44-DRAFT'ta; merge/flip TALİMAT-DIŞI, Abkarada-kararı-gerekir.
