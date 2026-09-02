# DR-1a-v2 — ratio→r KABLOSU: OTORİTE + SEMANTİK [REVİZE-TASLAK — ONAY-BEKLİYOR; NO-IMPLEMENTATION]

> v1-denetim-düzeltmeleri (Abkarada-§14). v1-arşivde ([DR-1a](DR-1a-ratio-to-r.md)). Ön-koşul: DR-1e-v2.

## 1. OTORİTE STATE-MACHINE (§14.1 — gizli-ikinci-state-kaldırıldı)

```rust
enum FecActuationMode {
    Manual { enabled: bool },  // DEFAULT: Manual { enabled: false } (mevcut-davranış)
    Controller,
}
```
Ayrı-`fec_enabled: bool`-alanı KALDIRILIR (impl'de: mevcut-bool → enum'a-migre; tek-writer=mode-enum).
Manual-enabled enum-İÇİNDE → Controller→Manual-geçişinde-dönülecek-durum EXPLICIT (gizli-reset-semantiği-yok).

**API (§14.2 — sessiz-no-op-YASAK):** mevcut `set_fec_enabled(bool)` **DEPRECATE** →
`pub fn set_manual_fec_enabled(&mut self, enabled: bool) -> Result<(), FecAuthorityError>`
(Controller-mode'da `Err(FecAuthorityError::ControllerOwnsFec)` — typed, caller-visible; sayaç-değil-hata).
Mevcut-caller-envanteri: üretim-çağıranı YOK (yalnız test/harness — grep: sprint5c_fec_emit, n8-testleri,
m1-harness, engine-testleri) → migration = test-güncellemesi; deprecate-süresi-kısa (aynı-commit-serisi).
`set_fec_actuation(mode) -> Result<(), FecAuthorityError>` — geçiş-etkinliği §12-transition-tablosu.

**Runtime-vs-build-flag (§14.3):** 1a = **RUNTIME-mode-yalnız** (default=Manual). Cargo-feature-YOK
(kill-switch/binary-size/rollout-gerekçesi-yok). "Flag-OFF=Manual" cümlesindeki-"flag" = runtime-mode-default'u.

## 2. FORMÜL (§14.4/§14.5)

```text
r = (ratio × k_actual + 500) / 1000        // half-up; tam-sayı
r_final = min(r, profile.r_code_max)        // tek-clamp; YENİ-policy-const-YOK (R_ENQUEUE_CAP-önerisi-GERİ-ÇEKİLDİ)
```
- **k_actual** (nominal-K-değil): partial-block'ta-gerçek-source-sayısı. Kapsam: full-block (k_actual=K) ·
  final-partial (k_actual<K) · timeout-flush-partial (aynı-formül) · K=1-edge (ratio≥500→r=1) · empty-block
  İMKANSIZ (k_actual≥1-invariant; blok-source'suz-açılmaz) · `k_actual>negotiated-K` → yapısal-reject
  (mevcut-schema-invariant; enqueue-öncesi-assert-değil-typed-hata).
- **Overflow:** ratio≤1000, k_actual≤255 → maks 255_000+500 « u32 — `u32`-aritmetik-kanıtlı-taşmasız (yorum+test).
- Bounded-alloc: r_final ≤ r_code_max ≤ profil-tablosu-sabiti → mevcut-profile-bound-yeterli (yeni-cap-sınıfı-yok).
- **K=8-boundary-golden-tablosu (impl-testi):** ratio {0, 62, 63, 187, 188, 312, 313, 437, 438, 562, 563, 687,
  688, 812, 813, 937, 938, 1000} → r {0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8} + profile-clamp-noktası (r8-üstü-yok
  K8R4-profilinde: ratio≥438→clamp-4 — profil-0x03-satırı-ayrı) + k_actual∈{1,5,8}-varyantları. Property:
  ∀ratio,k_actual: formül-eş + monotonluk + clamp.

## 3. VARIABLE-r RECEIVER-KANITI (§14.6 — varsayım-değil)

**Kod-kanıtı (mevcut):**
- `codec.rs:84-92,210-227`: repair-esi-kabulü `esi < r_code_max`-BOUND'la (EXACT-R-beklentisi-YOK);
  `ReceivedSymbol`-listesi-dinamik; decode-koşulu sufficiency (`any_missing`/yeterlilik) — kaç-repair-geldiği-
  serbest.
- `receiver.rs:40` yorum+tip: "r-alanı KALKTI (R_code_max profile/code-space'ten)" — transport-katmanı-R-taşımaz.
- `admission.rs` (engine): profil-gate profile_id-bazlı; r-bağımsız.
- PARAM/profil: `r_code_max`-taşır (ÜST-SINIR-semantiği), exact-R-semantiği-YOK (schema-param.rs).
- `r=0`-blok: receiver'da-blok-state'i source-arrival'la-açılır (block_have); repair-hiç-gelmezse decode-koşulu
  yalnız-tam-source'la-sağlanır — r=0 alıcıda-özel-durum-DEĞİL.
**Kapsam-boşluğu (dürüst):** r<Rmax ve blok-arası-değişken-r İNTEGRASYON-testi-YOK → impl-commit'inde ZORUNLU-plan:
blok-A-r=0 · blok-B-r=1 · blok-C-r=Rmax · kayıpsız-decode · recoverable-loss · unrecoverable-loss ·
out-of-order-repair · duplicate-repair (DuplicateEsi-fail-closed-mevcut) · block-boundary'de-mode-transition.
**Hüküm:** kod-kanıtı + bu-test-planı-yeşil-olmadan "wire-visible-değişiklik-gerekmez" ONAYLANMIŞ-SAYILMAZ.

## 4. LATCH-ANI (§14.7 — seçenek-karşılaştırmalı)

| Seçenek | Temsil-ettiği-durum | Sakınca |
|---|---|---|
| Block-open-snapshot | blok-başlangıç-koşulu | uzun-blokta-bayat; parity-ihtiyacı-blok-SONU-koşuluna-bağlı |
| K'inci-source-accepted | blok-içeriği-tamamlandığı-an | ≈block-close (fark: enqueue-gecikmesi) |
| **Block-close/enqueue-snapshot [SEÇİLEN]** | parity-ÜRETİM-kararının-verildiği-an — en-taze-ağ-durumu | uzun-blokta-open-close-arası-değişim-görmez (kabul: karar-anı-önemli) |
| Window-aggregate | yumuşatılmış | yeni-mekanizma; determinizm-yükü; 1a-kapsam-aşımı |

Seçim-gerekçesi: parity-emisyon-kararı enqueue-anında-verilir → o-anın-decision'ı en-ilgili-ağ-durumu;
determinizm: enqueue-anı-event-akışından-deterministik; ACK-event'i-blok-ilişkisi-gerekmez (decision-global-
ağ-durumu-özeti). Partial-timeout-flush: aynı-an-kuralı (flush=enqueue-anı). Idle'da-mode-transition: §12.
**LatchedDecision-alanları:** {ratio, decision_event_index (event-sayacı; ADR-17-timestamp-yerine-index),
no_rule, mode, emergency_active, age_events (=şimdiki-index−decision-index)}.

## 5. INITIAL / STALE / no_rule (§14.8/§14.9)

- **Initial:** hiç-decision-yokken-blok-kapanırsa → **ratio=LITERAL-0** (parity-yok; güvenli).
- **no_rule:** → **LITERAL-0** (v1'in-"fec_zero-consequent"-bağı-GERİ-ÇEKİLDİ — fallback-kalibrasyon-değerine-
  bağlanamaz; fec_zero-config-alanı-sıfır-olmayabilir).
- **Stale:** yapısal-kural = decision yalnız-AYNI-event-epoch-penceresinde-geçerli: `age_events >
  STALE_EVENT_WINDOW` → LITERAL-0 + `stale_zero_count`-telemetri. Eşik-DEĞERİ-politika-olarak-DONDURULMAZ:
  impl'de muhafazakâr-yapısal-öneri (örn. son-decision-sonrası-hiç-yeni-event-yoksa=stale-değil; pencere-değeri
  [CAL]-etiketli-deferred-policy) — decision-age-telemetri-HER-durumda.

## 6. EMERGENCY (§14.10)

**"Temporary safety interlock pending 1b/1d arbitration"** — ürün-politikası/optimal/kalibrasyon-sonucu-DEĞİL;
double-spend-önleme-suppression'ı. Emergency-aktifken yeni-blok-latch → LITERAL-0 + telemetri:
`suppression_reason=EmergencyPendingArbitration`, suppressed_requested_r, suppressed-parity-count.
Emergency-hücreleri 1b/1d-gelene-kadar controller-FEC-kalibrasyonuna-SOKULMAZ.

## 7. TELEMETRY (§14.11 — named-struct; tuple-değil)

```rust
pub struct FecActuationDiag {
    pub mode: FecActuationMode,
    pub controller_ratio: u32,        // son-decision
    pub computed_requested_r: u64,    // formül-çıktısı (clamp-öncesi)
    pub clamped_r: u64,
    pub planned_r: u64,               // enqueue-hedefi (=clamped_r; blok-başına)
    pub enqueued_r: u64,              // gerçek-enqueue-çağrıları (kümülatif)
    pub emitted_r: u64,               // wire'a-çıkan (kümülatif; kesit-1-collapse-ölçer)
    pub canceled_r: u64,
    pub budget_denied_r: u64,         // token-red (plan_fec_parity-break sayacı; 1b-öncesi-görünürlük)
    pub suppression_reason: Option<SuppressionReason>,
    pub last_block_id: u32,
    pub k_actual_last: u8,
    pub profile_r_max: u16,
    pub decision_age_events: u64,
    pub stale_zero_count: u64,
}
```
Amaç: 1a'nın-aktüatöre-GERÇEKTEN-ulaştığını-ölçmek (kesit-1-arm-collapse-tekrarı-önlenir); 1b-policy-uygulamaz.

## 8. MODE-TRANSITION-TABLOSU (§14.12)

| Durum | Geçiş | Etkinlik-anı | Davranış |
|---|---|---|---|
| Idle, blok-yok | M→C | ANINDA | sonraki-blok-Controller-latch'li |
| Açık-partial-blok | M→C | **SONRAKİ-block-boundary** | mevcut-blok Manual-kuralıyla-biter (mid-block-yasak) |
| Full-block-close-anında | M→C | O-close'dan-SONRAKİ-blok | close-latch'i-eski-mode'la |
| C→M{enabled:false} | — | SONRAKİ-boundary | pending-parity-kuyruğu: KORUNUR (emisyon-sürer; iptal-ayrı-karar-değil — sürpriz-drop-yok) |
| C→M{enabled:true} | — | SONRAKİ-boundary | Manual-sabit-akış-devralır |
| Emergency-sırasında | herhangi | boundary | emergency-suppression-önceliği-sürer (§6) |
| Pending-parity-varken | herhangi | boundary | kuyruk-drenajı-mode-bağımsız (emisyon-yolu-değişmiyor) |
| Rollback-isteği | C→M{false} | boundary | + diag-snapshot |
| Hiç-blok-kapanmazsa | pending-transition | **event-tabanlı-fallback:** N-event-sonra-force-effective (idle-kilidi-önlenir; N-yapısal-küçük) |

**Immediate-safety-disable** normal-transition'dan-AYRI: `set_manual_fec_enabled`-Manual-mode + pending_fec-
temizliği ayrı-acil-API-ihtiyacı-DOĞARSA ayrı-DR (şimdi-eklenmiyor; gerekçe-yok).

## 9. 1a/1b-SINIRI (§14.13 — dürüst-ifade)

**"Scheduling policy unchanged; workload presented to the existing policy changes."** Kanıt-yükümlülükleri:
queue-order-aynı · token-policy-aynı · rezervasyon-yok · preemption-yok · DATA/ARQ-öncelik-aynı · yalnız-
requested-parity-count-değişir · pending_fec-bound (mevcut-VecDeque + r≤r_max×aktif-blok — büyük-r-bellek-
büyümesi-bounded: enqueue-blok-tamamında, blok-sayısı-akışla-sınırlı; impl-testinde-assert).

## 10. Revize-artefakt-listesi (§14.14)
Authority-state-machine (§1) · transition-tablosu (§8) · k_actual-formül+overflow (§2) · variable-r-kanıt+
test-planı (§3) · latch-kararı (§4) · initial/no_rule/stale (§5) · emergency-interlock (§6) · telemetry-struct
(§7) · 1a/1b-sınırı (§9) · touched: sender.rs{mode-enum, latch, plan_fec_parity-sayaç-görünürlüğü-additive},
kama-engine{blok-tamam→enqueue×r_final, k_actual-akışı} · forbidden: bw-6-blok+pacer/schedule-sıralaması+
TokenBucket · property+boundary-golden (§2) · integration (§3) · rollback (§8). **NO-IMPLEMENTATION.**
