# DR-1a IMPLEMENTATION REPORT (Commit F/G/H) — ayrı-belge (§19-arşiv-kuralı)

> Onay-zinciri: "DR-1e-v2 IMPLEMENTATION ACCEPTED — ONE INVARIANT-HARDENING PATCH REQUIRED" →
> "DR-1a-v2 ADDENDUM APPROVED WITH SAME-EVENT CAUSALITY INTERPRETATION" → "DR-1a-v2 APPROVED FOR
> IMPLEMENTATION". DR-1a-v2 + addendum korunur; bu-rapor implementasyon-kanıtıdır.

## Commit-F — 1e-invariant-sertleştirme (`227c3af`)
`new_internal(ValidatedFuzzyConfig)` (raw-overload-YOK) · `ValidatedFuzzyConfig::trusted_fixture()`
pub(crate) (bypass-yok; unsafe-yok; runtime-panik-yok) · compile_fail-doctest (dış-raw-kuruluş E0423) ·
`fixture_validates`-CI-invariantı · §2-diag-dil-daraltması ("Semantic non-regression demonstrated; …
Performance neutrality has not been independently benchmarked").

## Commit-G — authority + decision-semantics (`9a30240`)
- **Authority (§6):** `FecActuationMode{Manual{enabled}, Controller}`; ayrı-bool-KALDIRILDI; default
  `Manual{enabled:false}` (M1-determinizm-spot bit-özdeş: fec_enabled→fec_mode-geçişi davranış-birebir).
  `set_manual_fec_enabled`: Controller'da `Err(NotInManualMode)` (sessiz-no-op-yasak; state-değişmez; sayaç).
  Eski-`set_fec_enabled` kaldırıldı (üretim-caller-yok — grep-teyit; test/harness-migre).
- **ratio→r (§8):** `compute_requested_r` — `(ratio×k_actual+500)/1000` + clamp-r_code_max; overflow-kanıtı;
  golden-K8-boundary-tablosu (12-nokta) + exhaustive-property (ratio-0..=1000 × k∈{1..8,255}).
- **Latch (§9) + same-event-causality-guard (addendum-adı):** `fec_controller_latch` saf-zincir; guard
  `event_index==fusion_event_index`; LITERAL-0-fallback'ler (initial/no-rule/mismatch — config-bağımsız);
  emergency **"Temporary safety interlock pending 1b/1d arbitration"** (requested-görünür; planned=0).
- **Ordering-kanıtı (§4):** fuzzy latch'ten-ÖNCE (same-event-eligible-testli); **RecoveredControl
  fuzzy-koşturmaz (index-sabit) VE emisyon-üretmez → blok-KAPATAMAZ** (testli — 4.5-DUR-koşulu-oluşmadı).
- **Diag (§11):** `FecActuationDiag` named-struct; sender'da-son-latch+bounded-sayaçlar (per-block-tam-kayıt
  harness-yolunda; alloc/unbounded-yok).

## Commit-H — engine-actuation-wiring + integration
- Engine-path-zincir (gerçek-emisyon): snapshot→latch→enqueue×planned_r→plan_fec_parity(token)→decisions→
  seal→decode. [Engine-matris](variable-r-engine-matrix.md): 9-satır PASS; requested→emitted-zinciri-
  ayrışıyor (enqueued==emitted==7-A-D-toplamı); clamp(1000→8→4) engine-yolunda; partial-k_actual=5-formülü;
  emergency-planned=0; r>0→r=0-contamination-yok.
- **Manual-non-regression (§13):** disabled=default → {A+B+C}+N8+golden+M1-spot bit-özdeş; enabled →
  mevcut-fixed-FEC-harness'lar (sprint5c/n8/n8_v2/m1/param-b2) AYNEN-geçiyor (migre-tek-satır-API-adı);
  Controller-default-DEĞİL-testi.
- **1a/1b-sınırı (§12):** "Scheduling policy unchanged; workload presented to the existing policy changes."
  Queue-order/token/rezervasyon/öncelik/pacer/BtlBw/budget-cap DOKUNULMADI; budget-denied-GAUGE yalnız-
  görünürlük. Requested-r-artışında emitted-düşebilir/collapse-görülebilir — 1a-hatası-değil; diag-ayrıştırır.

## Gate-sonuçları (her-commit-ayrı-yeşil)
| Commit | {A} | B/C | N8+golden | fmt/clippy |
|---|---|---|---|---|
| F (227c3af) | değişmedi (at139+1-doctest) | PASS | tam-suite | temiz |
| G (9a30240) | at 140→145 (+5) | PASS | tam-suite + M1-spot-bit-özdeş | temiz |
| H | engine 22→27 (+5) | PASS | tam-suite | temiz |

## Açık-sınırlar (1a-kapsam-dışı; değişmedi)
BtlBw-offered-kilitlenmesi (1b-girdisi; DEĞİŞİKLİK-YASAĞI-uygulandı) · parity-token-artığı-zamanlaması (1b) ·
dup↔parity-arbitrasyon (1d) · fuzzy-eşik-kalibrasyonu (kesit-2-sonrası) · Kesit-2/Sprint-6 İZİN-BEKLİYOR.
