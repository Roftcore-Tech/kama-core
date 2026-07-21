# DR-1e-v2 — FuzzyConfig CONFIG-INJECTION [REVİZE-TASLAK — ONAY-BEKLİYOR; NO-IMPLEMENTATION]

> v1-denetim-düzeltmeleri (Abkarada-§13): tek-API-modeli · versioning-DEFERRED · fingerprint-DEFERRED ·
> validation-sınıflandırma-tablosu · default-equivalence-netleştirmesi. v1-arşivde ([DR-1e](DR-1e-config-injection.md)).

## 1. API (tek-model; §13.1)

```rust
pub struct FuzzyConfig { /* mevcut: 5×3 trapezoid + 9 consequent */ }
pub struct ValidatedFuzzyConfig(FuzzyConfig);          // newtype; tek-üretim-yolu validate()
#[derive(Debug, thiserror::Error, PartialEq)] pub enum FuzzyConfigError { /* kural-başına-variant */ }

impl FuzzyConfig {
    pub fn validate(self) -> Result<ValidatedFuzzyConfig, FuzzyConfigError>;
}
impl Sender {
    pub fn new() -> Self; // DEĞİŞMEZ — fixture'la ortak-internal-ctor'a delege
    pub fn try_with_fuzzy_config(config: FuzzyConfig) -> Result<Self, FuzzyConfigError>; // validate-içeride
}
```
- Caller-validation-akışı-açık: `try_with_fuzzy_config` içinde `config.validate()?` (tek-hata-tipi).
- **`SenderConfig`-god-config YOK** (mevcut-ihtiyaç-yalnız-fuzzy; kapsam=fuzzy-config). Diğer-fixture'lar
  (emergency/hysteresis/budget/retx) İLERİDE-ayrı-DR (her-biri-kendi-ihtiyaç-gerekçesiyle).
- Ortak-internal: `fn new_with_validated(cfg: ValidatedFuzzyConfig) -> Self`; `new()` =
  `new_with_validated(FIXTURE_VALIDATED)` (fixture-validate'i-derleme-testinde-kanıtlı — panik-yolu-üretimde-yok).

## 2. Versioning — **DEFERRED** (§13.2)
Yalnız in-memory-struct-injection var; serialized-format/network-transfer/persistent-store/remote-distribution
HEPSİ-kapsam-dışı → `version`-alanı-EKLENMEZ. (Somut-consumer+failure-mode-belgelenmeden-retain-edilmez.)

## 3. Fingerprint — **DEFERRED** (§13.3)
FNV-1a-önerisi-GERİ-ÇEKİLDİ: 1e-için-gereksiz · canonical-encoding-tanımsız · bespoke-hash-yolu · collision/
field-ordering-yüzeyi. Gelecekte-config-identification-gerekirse: önce-canonical-encoding-tanımı + repoda-onaylı-
mevcut-primitif (BLAKE2s — telemetry-etiketi-olduğu-açıkça; güvenlik-kimliği-değil) — ayrı-DR.

## 4. Validation-kuralları — sınıflandırma-tablosu (§13.4)

| Kural | Sınıf | Fixture-geçer? | Neden-şimdi |
|---|---|---|---|
| permille/ppm/bw range-sınırları | YAPISAL | ✓ | domain-dışı-değer-matematiği-bozar |
| çarpım-overflow-güvenliği (u64-ara-tip-kontrolü) | YAPISAL | ✓ | ADR-17-taşma-yasağı |
| trapezoid `a≤b≤c≤d` | YAPISAL | ✓ | trap_permille-öngereksinimi |
| zero-width/division-invalid şekiller | YAPISAL | ✓ | payda-0-yolu |
| `covers_domain` (sol-omuz+sağ-omuz+kesin-örtüşme; 5-girdi) | **SPEC-İNVARYANT** (fusion.rs [TUR-4]-invariant + golden `covers_domain`-testi) | ✓ | zero-weight-deliği motor-sözleşmesi |
| `fec_zero≤fec_med≤fec_high` vb. consequent-SIRALAMASI | **POLİTİKA-KISITI** — **ENFORCE-EDİLMEZ** (v1'den-GERİ-ÇEKİLDİ: spec-açık-invariant-değil; kalibrasyon-uzayını-daraltır) | ✓ | — (validate-dışı; dokümante-uyarı) |

## 5. Default-equivalence (§13.5)
Kanıt-kümesi: {output-bytes · scheduling-order · pacing-decisions · counters · traces · golden-vectors ·
deterministik-test-sonuçları} default-yolda-baseline-özdeş. Yeni-eklenen-sayaç-olmayacak (1e-sayaç-eklemiyor);
eklenirse baseline'da-yok-oldukları-için-özdeşlik-karşılaştırmasına-girmez + default-yolda-sıfır-ayrıca-doğrulanır.

## 6. Net-hükümler (§13.6)
- **API:** yukarıdaki-imza-bloğu (tek-model).
- **Validation-ownership:** `FuzzyConfig::validate` (fusion.rs-modülü; motor-matematiğine-dokunmaz).
- **Default-delegation:** `new()` → ortak-internal-ctor (fixture-validated-sabit).
- **No-versioning-now / no-fingerprint-now** (yukarıda-gerekçeli; istisna-yok).
- **Touched-files:** `fusion.rs` (FuzzyConfigError + validate + ValidatedFuzzyConfig; ~+120-satır) ·
  `sender.rs` yalnız-ctor-bölgesi (~480-506: fixture-hard-wire → ortak-internal + `try_with_fuzzy_config`).
- **Forbidden:** bw.rs-yasak-6-blok · `fuzzy_evaluate`-girdi-kurulumu (923-955) · `decide()`-gövdesi (320-393) ·
  kural-tablosu/emergency/compress-hardcode'ları · TokenBucket/SACK-V2/ZAMAN-MODELİ · tüm-diğer-fixture'lar.
- **Tests:** (i) `new()`≡`try_with_fuzzy_config(fixture)` davranışsal-özdeşlik (aynı-event-dizisi→decisions/diag
  bit-özdeş; N8-spot-kalıbı) (ii) validate-red-matrisi (kural-başına-negatif) (iii) fixture-validate-Ok (iv) mevcut-CI.
- **Rollback:** additive-API; `try_with_fuzzy_config`-çağıranları-silmek-yeter; `new()`-yolu-değişmediğinden-risk-yok.
- **Commit-boundary:** TEK-commit = API+validate+testler+{A+B+C}; politika/eşik-DEĞERLERİ-ASLA-bu-commit'te-değil.
