# DR-1e — FuzzyConfig CONFIG-INJECTION (nötr-yapısal-yarı) [TASLAK — ONAY BEKLİYOR; NO-IMPLEMENTATION]

- **Amaç:** `FuzzyConfig` runtime/test-girdisi olabilsin; mevcut `fixture()` VARSAYILAN-davranış olarak AYNEN
  kalsın; kalibrasyon kod-değişikliği-değil CONFIG-değişikliği hâline gelsin. Politika/eşik-DEĞERLERİ bu-DR'ın
  KAPSAMI-DIŞI (veriden-sonra ayrı-DR).
- **Davranışsal-özdeşlik-tanımı (bu-DR'ın non-regression-ölçütü; "byte-for-byte" derlenmiş-binary-anlamında
  KULLANILMAZ):** {output-bytes · scheduling-order · pacing-decisions · counters · traces · golden-vectors ·
  deterministik-test-sonuçları} default-yolda mevcut-baseline ile ÖZDEŞ.

## 20-KONU KARAR-TABLOSU

| # | Konu | Karar-taslağı |
|---|---|---|
| 1 | Injection-noktası | Yeni-ctor `Sender::with_config(cfg: SenderConfig)`; `SenderConfig{fuzzy: FuzzyConfig, …ileride-diğer-fixture'lar}`. `fuzzy_evaluate`-çağrı-zinciri DEĞİŞMEZ (controller-nesnesi zaten `DeterministicFuzzyController::new(cfg)`-parametrik). |
| 2 | Sahiplik/lifetime | Config `Sender`-kurulumunda **taşınır (owned, by-value)**; controller-içinde yaşar. Paylaşım/`Arc` YOK (tek-sender-tek-config). |
| 3 | Immutable-tercihi | Kurulum-sonrası **IMMUTABLE**. Runtime-mutasyon/hot-swap YOK (determinizm + ADR-17; yeniden-konfig = yeni-Sender). |
| 4 | `Sender::new()` geriye-uyum | `new()` AYNEN-KALIR ve `with_config(SenderConfig::fixture())`'a delege eder → default-yol davranışsal-özdeş. Hiçbir-mevcut-çağıran değişmez. |
| 5 | Test-only vs runtime-API ayrımı | TEK-public-API (`with_config`) — test-only-feature-kapısı GEREKMEZ çünkü default-yol-özdeş ve API-additive. (`cfg(test)`-özel-yol YOK — test/üretim-ayrışması-riski.) |
| 6 | Invalid-trapezoid doğrulaması | `SenderConfig::validate()` → `Result<ValidatedConfig, ConfigError>`: her-trapezoid `a≤b≤c≤d` (typed-error; panik-yok). `with_config` `ValidatedConfig` alır → invalid-config TİP-DÜZEYİNDE sender'a-giremez. |
| 7 | Range-doğrulaması | Alan-bazlı-sınırlar validate'te: permille-alanlar ≤1000; ppm ≤1_000_000; bw-eşikleri log2_fp-domain-monoton; sıfır-genişlik-domain reddi. |
| 8 | Monotonic-membership | Mevcut `covers_domain()`-invariantı (sol-omuz-LOW + sağ-omuz-HIGH + kesin-örtüşme) validate'e ZORUNLU-kural olarak-taşınır (5-girdinin-5'i için). |
| 9 | Consequent-sınırları | 9-consequent ∈ [0,1000]-permille; ek-sıra-kuralı: `fec_zero≤fec_med≤fec_high`, `arq_light≤arq_med≤arq_moderate`, `sr_low≤sr_med≤sr_high` (anlamsız-tersine-çevirme reddi). |
| 10 | Hardcoded rule/emergency kapsamı | **KAPSAM-DIŞI.** Kural-tablosu (R1-R4), SR1, emergency-tetiği (min3>500‰), compress bu-DR'da HARDCODED-KALIR (politika-yüzeyi; taramadan-sonra ayrı-DR). Bu-DR yalnız-eşik/consequent-DEĞERLERİNİ enjekte-edilebilir-yapar. |
| 11 | Config-versioning | `SenderConfig.version: u32` alanı EKLENİR (v1); validate bilinmeyen-versiyonu reddeder. (İleri-uyum; serde/dosya-formatı bu-DR-kapsamı-dışı — struct-literal-injection yeter.) |
| 12 | Fingerprint/hash | `ValidatedConfig::fingerprint() -> u64` (FNV-1a, alan-sıralı-deterministik) — telemetri/rapor "hangi-config'le-koşuldu" izi. Kripto-hash DEĞİL (güvenlik-iddiası-yok; registry-dışı-primitif-eklenmez). |
| 13 | Determinizm-etkisi | SIFIR: config=veri; motor-matematiği-değişmez; aynı-config+aynı-input→aynı-output (ADR-17-korunur). Fingerprint sabit-algoritmalı-deterministik. |
| 14 | Golden yeniden-türetme-koşulu | Default-yol: golden'lar AYNEN-GEÇER (fixture-değerleri-değişmedi — türetme-GEREKMEZ). Golden-yeniden-türetme YALNIZ kalibre-değerler-fixture'a-işlenirse (spec-§16-adımı; ayrı-DR+ayrı-commit). Non-fixture-config'ler golden-kapsamı-dışı (test-kendi-assert'ini-taşır). |
| 15 | Feature-flag gerekli-mi | **GEREKMEZ:** additive-API + default-delege = flag'siz-nötr. (DR-1a'nın-aksine burada-davranış-dalı-yok.) |
| 16 | Rollback-yolu | API-additive → rollback = `with_config`-çağıranların-silinmesi; `new()`-yolu-hiç-değişmediği-için üretim-rollback-riski-yok. |
| 17 | Mühürlü-komşuluk | Dokunulan-dosyalar: `fusion.rs` (validate/fingerprint — motor-matematiğine-DEĞİL, yalnız-config-katmanı), `sender.rs` **yalnız-ctor-bölgesi** (~480 `Sender::new` fixture-hard-wire satırları). bw.rs/rate.rs/pacer.rs/receiver.rs DOKUNULMAZ. |
| 18 | Yasak-satırlar | bw.rs yasak-6-blok (W_BTLBW 555-575 · W_qd 577-582 · btlbw.update 604-605/619-620 · credit_delivered 633-634 · on_sent ~1530) + `fuzzy_evaluate`-girdi-kurulumu (923-955) + `decide()`-kural-tablosu (334-393) + TokenBucket/SACK-V2/ZAMAN-MODELİ — tek-satır-değişmez. Ctor-değişikliği bu-satırların-HİÇBİRİNE-komşu-değil. |
| 19 | Default-non-regression-kriteri | {A+B+C}-tam + davranışsal-özdeşlik-tanımı (üstte) default-yolda: tüm-mevcut-testler değişiklik-SIZ-geçer (test-gövdesi-uyarlaması-bile-gerekmemeli — `new()`-imzası-aynı). |
| 20 | N8 + CI kanıt-planı | (i) `new()==with_config(fixture)` özdeşlik-testi: aynı-event-dizisi → diag_rate/decisions bit-özdeş (N8-spot-kalıbı). (ii) validate-red-testleri (invalid-trapezoid/range/monoton/consequent-sıra). (iii) fingerprint-determinizm-testi. (iv) mevcut-CI (fmt/clippy/test/audit/deny) yeşil. |

## KAPSAM-SINIRI
Yalnız-injection-YOLU. Eşik-DEĞERLERİ, kural-tablosu-parametrizasyonu, dosya/serde-config-formatı, hot-reload,
emergency/hysteresis/budget-fixture'larının-injection'ı (SenderConfig'e-alan-eklemek-additive-uzantı) — HEPSİ-SONRA.
