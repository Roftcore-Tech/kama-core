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

**Mekanistik yorum:**
- **workload/dispatch (Abkarada-düzeltme-2 — İSİM):** `empirical_initial_dispatch_bytes` ≈ **1 unit** (ff_burst=1;
  ratio=w_units=8) TÜM boyutlarda. **Bu sender'ın PROTOKOL-İÇİ cwnd-değişkeni DEĞİLDİR** — faithful-wire'da
  stall-gap(1500ms)-öncesi gözlenen AMPİRİK ilk-dispatch-miktarıdır (W-1'in "cwnd"iyle KARIŞTIRILMAMALI). ff-burst
  UNIT-sabit; boyut-farkı BYTE-tabanlı floor-dispatch-hızından + OFF-deadlock'tan gelir.
- **65B NEUTRAL (kanıtlanmış-nötr-hücre):** küçük-workload (520B) → OFF-de floor-dispatch'le tamamlar (recv=9) →
  cold-start-gerekmez.
- **300B DECISIVE (kanıtlanmış-faydalı-hücre):** OFF 1-pakette DEADLOCK (recv=1); ON tüm-8-birimi-gönderir →
  @~61–62s tamamlar (recv=9). **Cold-start'ın nedensel-liveness-faydası.** recovery_path=none; completion_trigger=
  FirstFlight — completion RETRANSMISSION/RECOVERY ile-DEĞİL, sürdürülen first-flight-source-dispatch ile.
- **1200B (UNRESOLVED@45s):** 45s-cap'te İKİSİ DE blok-tamamlamaz (app_delivery=0; NO_COMPLETION_BY_45S — mutlak-
  NoRecovery-DEĞİL). **DATA-plane PARTIAL-progress asimetrisi (doğrulandı; adversarial §5):** ON **3 DISTINCT
  source-shard** (transport_dup=0), OFF **1** (deadlock). Control-plane-DEĞİL (probe receiver_events'e-sayılmaz)
  ama **blok-completion-DA-DEĞİL** (kısmi 3/8). **Nihai-kayıt: "cold-start produces additional source progress,
  but completion benefit is not demonstrated within 45 seconds."** YASAK-iddialar: 1200B-decisive · 1200B-liveness-
  solved · 1200B-recovery-impossible · FEC/ARQ-hükmü.

**Sınır-bulgusu (Abkarada-düzeltme-1 — KAPSAM):** Cold-start'ın faydası **workload-rejimine-bağlıdır**:
`300B/w8 DECISIVE (kanıtlı)` · `65B/w8 NEUTRAL (kanıtlı)` · `1200B/w8 completion-UNRESOLVED@45s`. **"Faydalı-bölge
65B-üzeri" GENELLEMESİ KULLANILMAZ** — 300–1200B-arası-boyutlar HİÇ-ölçülmedi. **Mekanizma-inceltmesi (Abkarada):**
cold-start-ON BtlBw-oluşmadan-da source-dispatch'in-devam-etmesini-sağlar; observed-completion first-flight-source-
dispatch-iledir (recovery-DEĞİL). "PreConverge floor-dispatch" KESİN-kök-neden-olarak-yazılmadan-ÖNCE (freeze-öncesi)
`state_at_each_source_wire_emission / srtt / pacing_rate / dispatch_authority` state-at-emission-ledger'ıyla-doğrulanmalı.

> **REINTERPRETASYON NOTU (frozen-evidence):** R3-marker'ları (`70ea2de`-harness) eski-etiketi
> `w2b_effective_initial_cwnd_bytes` / `w2b_workload_to_initial_cwnd_ratio` taşır = AYNI-DEĞER, yeniden-yorumla
> `empirical_initial_dispatch_bytes` / `workload_to_effective_dispatch_ratio` (protokol-cwnd-DEĞİL; Abkarada-
> düzeltme-2). Harness alan-adı düzeltildi (sonraki-koşular); değerler-özdeş olduğundan 54-koşu re-run-GEREKMEZ.

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

> ## **DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL — SCOPE-LIMITED CHARACTERIZATION COMPLETE**
> - **W-1:** cold-start DECISIVE when workload > empirical-initial-dispatch AND some ACK feedback exists.
> - **W-2B (R3, TOTAL ACK starvation) — workload-regime dependent:**
>   - **300B/w8: DECISIVE (proven)** — OFF deadlocks after 1 packet (recv=1); ON completes @~61–62s (recv=9)
>     via sustained first-flight source dispatch (recovery_path=none; no retransmission/recovery).
>   - **65B/w8: NEUTRAL (proven)** — both complete (~14.7s).
>   - **1200B/w8: UNRESOLVED @45s** — neither completes within the 45s cap (app_delivery=0 both). Verified
>     DATA-plane partial-progress asymmetry (ON 3 distinct source-shards, transport_dup=0; OFF 1, deadlock):
>     *cold-start produces additional source progress, but completion benefit is not demonstrated within 45s.*
>   - Sizes 300–1200B NOT measured; **no "beneficial above 65B" generalization**.
> - **W-3:** post-blackout healing validated (3/3).
> - **H1:** clean.

**FULL — SCOPE-LIMITED CHARACTERIZATION COMPLETE gerekçesi (Abkarada-semantiği; "her-boyutta-completion"
DEĞİL):** full-starvation-sınır-koşulu deterministik-karakterize-edildi + doğal-production-path'ler-ayrıştırıldı
(completion_trigger vs recovery-event) + ON/OFF-marjinal-etki-dürüstçe-ölçüldü (65B-NEUTRAL / 300B-DECISIVE /
1200B-UNRESOLVED@45s) + açık-hücre horizon-sınırlı-doğru-sınıflandırıldı + uygulama-doğruluğu+dedup-korundu
(app_dup=0 tüm-hücre). **Kanıtlanan: 300B/w8-DECISIVE + 65B/w8-NEUTRAL kesin; 1200B/w8 completion-UNRESOLVED@45s
(partial-progress-var); 300–1200B-arası-ölçülmedi. Genelleme YOK.**

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
- **HÜKÜM:** `DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL — SCOPE-LIMITED CHARACTERIZATION COMPLETE` — W-1 decisive
  (workload>empirical-dispatch+ACK) · W-2B-R3 {65B-NEUTRAL, 300B-DECISIVE, 1200B-UNRESOLVED@45s+partial-progress}
  · W-3 heal-3/3 · H1-clean. (300–1200B-arası-ölçülmedi; "beneficial-above-65B" genellemesi YOK.)
- **ÜRETİM-ETKİSİ:** SIFIR (harness/test-only; flag-OFF byte-özdeş; clippy+pin yeşil; merge/flip-YASAK).
- **KANIT:** faithful two-netns; 54+3-koşu; her-koşu VALID (acceptance-RTT-pin + overlap-gate + arg-paritesi +
  clock-domain + ack_cap_hard=false + app_dup=0); pre-reg-anchor'ları donmuş (a635efa/70ea2de); adversarial-0/6.
- **AÇIK/DEVİR (A2 — sıradaki):** ürün-alıcı patience/resource (iki-seviye-cap) + **POST-TIMEOUT RECOVERED-CONTROL
  NOTIFICATION CONTRACT** (ACK-bağımsız-post-timeout-recovery ürün-öğrenimi). 1200B DECISIVE-vs-NEUTRAL çözümü
  (uzun-horizon; opsiyonel-follow-up). End-to-end-product-FULL A2-öncesi iddia-EDİLMEZ.
- **KARAR-BEKLEYEN:** A2-talimatı (A1-DUR-SON-sonrası yazılır); default-flip YALNIZ A2+rollout+§4.4-benchmark-sonrası.
- **DUR:** onay-bekliyor — PR#44-DRAFT'ta; merge/flip TALİMAT-DIŞI, Abkarada-kararı-gerekir.
