# ADR — §8.5 bütçe kısıtı ile §11 MinimalParity ilişkisi

**Status:** ACCEPTED (Faz-6 dondurma, 2026-09-11) — normatif çözüm; spec `[AMENDMENT-3]`.

## Context

Engine-spec §8.5 `parity_final = min(fuzzy_parity, budget_cap, profile_cap)` yazar ve
`budget_cap = BtlBw·1000/budget_ref` (budget_ref 125 kB/s, `[CAL]`) dar bantta (BtlBw < 125 B/s)
**0'a iner**. §11 ise aynı köşe ailesinde (loss HIGH ∧ bw LOW ∧ ack_confidence LOW)
"minimal parity shards" ister ve §11:796 / TDD §5.6 "parity yine §8.5 ile clamp" der. Literal
okuma: §8.5 tüm onarım otoritesini sıfırlar, §11 sıfırdan büyük parity ister — çelişki.
Faz-2'de kod bu köşeyi bir KARAR ile geçmiş, politikayı sahibe bırakmıştı (`sender.rs` clamp bloğu
yorumu). Faz-6 bu kararı dondurmak zorunda.

## Decision — doğrulanmış uygulama ne yapıyorsa spec ONU söyler

Uygulama (`crates/adaptive-transport/src/sender.rs`) satır satır okundu; iki AYRI mekanizma var:

**(1) §8.5 — NORMAL adaptif bütçe, sıfırlama-reddi ile.** `fuzzy_evaluate` arbiter çıkışına
`clamped = min(final_ratio, cap, 900)` uygular. `cap == 0 ∧ final_ratio > 0` ise clamp
**uygulanmaz**, `final_ratio` aynen geçer, `diag_parity_budget_would_zero` artar. Aksi hâlde
uygulanır (`diag_parity_budget_clamped`). BtlBw örneği yokken cap = profil tavanı (900‰).

**(2) §11 — dar, ÜÇ KAPILI acil-durum istisnası, §8.5'e TABİ DEĞİL.** `fec_controller_latch`
emergency dalı yalnız şu üçü BİRLİKTE iken MinimalParity üretir: fuzzy mod
`EmergencyForwardRedundancy` (loss HIGH ∧ bw LOW ∧ ack_conf LOW) **∧** retreat-hysteresis merdiveni
≥ Dup2 (health < 300‰) **∧** framing SegmentBlock. Oran `EmergencyConfig` sabitidir (Dup2 100‰,
Dup3 200‰), yalnız profil `r_code_max` ile kırpılır, `budget::parity_final` ÇAĞRILMAZ ve normal
oranın **yerine geçer** (toplanmaz). Tek latch yolu → ikinci FEC emitörü yoktur.

Normatif sonuç (spec'e `[AMENDMENT-3]` olarak işlendi — engine §8.5, §11; TDD §5.6):
> §8.5 normal bütçeyi yönetir ve yalnız KÜÇÜLTÜR, sıfırlamaz. §11 MinimalParity kendi üç
> kapısıyla sınırlı bir istisnadır, §8.5 bütçesine tabi değildir, sınırsız ikinci emitör olamaz.

## Önerilen ifadeden SAPMA (raporlanıyor, zorla uydurulmadı)

Faz-6 görevi istisnayı *"§8.5 bütçesi tüm onarım otoritesini yok edecekken"* diye tanımlamayı
önermişti. **Uygulama böyle çalışmıyor:** §11'in tetiği §8.5'in sıfıra inmesi DEĞİL, fuzzy
emergency modu + hysteresis merdivenidir; §8.5'in sıfırlama-reddi ise normal yolun bağımsız
kuralıdır. İki mekanizma aynı köşede farklı kapılarla çalışır ve birbirini tetiklemez. Önerilen
tetik cümlesi bu yüzden alınmadı; spec koda göre yazıldı. Çekirdek yorum (§8.5 normal otorite,
§11 dar/sınırlı istisna, tek otorite korunur) uygulamayla ÖRTÜŞÜYOR — bu nedenle blocker değil,
clause-düzeyi düzeltmedir.

## Alternatives considered

* §8.5'i literal uygulayıp §11'i çıkarmak → ürünün birincil hedef rejiminde FEC'i kapatır; reddedildi.
* §11 istisnasını §8.5 bütçesine bağlamak → istisnanın var oluş sebebini yok eder; reddedildi.
* Runtime'ı önerilen tetiğe göre yeniden yazmak → Faz-6 dondurma kuralını ihlal eder; reddedildi.

## Consequences

* Spec ve kod artık aynı cümleyi söylüyor; Faz-4 audit'inin "spec-içi çelişki" kalemi kapandı.
* `would_zero` sayacı normal yolu izler; **emergency yolu bu sayaçları artırmaz** (bilinen kapsam
  sınırı, KNOWN-LIMITATIONS LIM-006). `would_zero = 0` görmek "§8.5 her yerde uygulandı" DEMEZ.
* Emergency duplication (Tiny framing) hiçbir bütçe kısıtından geçmez — aynı sınır.

## Evidence

* Kod: `sender.rs` clamp bloğu (would_zero/clamped), `fec_controller_latch` emergency dalı,
  `emergency.rs::action`, `budget.rs::{budget_cap_permille, parity_final}`.
* Faz-5 T3b sondası: köşeye GİRİLDİ (BtlBw 19 ve 1 B/s), `would_zero = 0` çünkü arbiter parity
  önermedi → `SPEC CONFLICT NOT REACHED`.
* **Faz-5 T0–T7, 330/330 Kama koşusu: `recover_fwd_entries = 0`.** §11 yolu hiçbir doğrulama
  hücresinde ateşlemedi (T6 health Dead iken bile `ack_confidence = 1000‰` raporlandı). İstisna
  birim-testlidir (`tests/tv_emergency.rs`), saha-doğrulanmamıştır → HOSTILE-ENVELOPE "OUTSIDE".

## Revisit conditions

Saha/ürün kanıtı §11 köşesinin (çift-yönlü ≥ %80 kayıp + sub-kbps) hedef zarfta olduğunu
gösterirse: önce ack_confidence'ın o köşede neden 1000‰ kaldığı ölçülür (EVOLUTION §9.6), sonra
istisnanın bütçe ilişkisi yeniden değerlendirilir. `transport-v1.x` sürüm kimliği gerekir.
