# PRE-SURGICAL BASELINE (5c-öncesi) — non-regression referansı

> Tag: `pre-surgical-baseline` → main `bb61c2d`. Bu, 5c mühürlü-dokunuşlarından-ÖNCEKİ son-temiz-durum.
> Her 5c mini-DR-sonrası bu-referansa-karşı karşılaştırılır. **ÖNEMLİ BULGU aşağıda: desen1_sim exact-değerleri
> HashSet-nondeterministiktir → "bit-for-bit exact-değer" karşılaştırması desen1 için GEÇERSİZ; güvenilir-gate
> aralık-assertion-pass/fail + exact-test-sayılarıdır.**

## A. DETERMİNİSTİK EXACT-GATE'LER (bit-for-bit karşılaştırılabilir) — BİRİNCİL REFERANS

Bu sayılar RNG-bağımsız, process-arası SABİT. 5c-karşılaştırmasının çekirdeği bunlardır:

| Gate | Değer (baseline) |
|---|---|
| kama-noise-wire test | **90 passed** (default) + libsodium-oracle differential 3 |
| kama-fec-codec test | **25 passed** |
| kama-transport-schema test | **9 passed** |
| kama-integration test | **9 passed** (5a pipeline+control) |
| adaptive-transport test (normal) | **118 passed** |
| desen1_sim (ignored, ağır-sim) | **5 passed** (aralık-assertion) |
| fmt --check · clippy -D warnings | temiz (default + all-features) |

**5c-KURALI:** her mini-DR-commit-sonrası bu-sayılar AYNEN korunmalı (düşüş/fail = İHLAL → pre-surgical-baseline'a revert).

## B. desen1_sim 5-PİN — ARALIK-ASSERTION (deterministik PASS/FAIL; exact-değer DEĞİL)

Pinler BOOLEAN-ARALIK'tır (RNG-bağımsız pass/fail); exact-nokta-değerler HashSet-order-nondeterministik (§D):

| Test | Pin-koşulu (deterministik) |
|---|---|
| `desen1_expiry_pin_3a2` | `qd_max_us > 1_000_000` (qd>1s canlı) **VE** `btlbw_at_daralma_end < 2100` (tek-adım-demotion ≥%50) |
| `scan_long_narrowing_convergence` | `btlbw_at_daralma_end < 600` (kopmasız-daralma kanal-bandına-yakınsar; kanal=375) |
| `scan_subkbps_anchor` | `btlbw_at_daralma_end >= rate − rate/4` (sub-kbps-sabit-kanala yakınsama) |
| `desen1_period_counters` | dönem-sayaçları (unique/dropped/P-vektör aralıkları) |
| `recalibrate_cadence` | kadans-yeniden-kalibrasyon invariant'ı |

**5c-KURALI:** bu 5 aralık-assertion AYNEN geçmeli (margin-daralması bile İHLAL-şüphesi → incele).

## C. FEC-KAPALI HOT-PATH (yapısal, deterministik)

- `sender.rs:134` `TransmitKind::FecParity` variant VAR ama **emit-EDİLMEZ** (§10-politika; 5c'de bu-değişecek — DR-S5-1).
- `bench/src/kama.rs:7,133` FEC-parity ÜRETİLMEZ → wire'da yalnız DATA. Hot-path allocation-yok (FEC-kapalı).
- **5c-KURALI:** FEC-emisyon-AÇILINCA (DR-S5-1), FEC-KAPALI-yol (feature/flag-off) hâlâ allocation-yapmamalı (koşullu-emisyon).

## D. ⚠ BULGU — desen1_sim EXACT-DEĞERLERİ NONDETERMİNİSTİK (bit-for-bit GEÇERSİZ)

desen1_sim `HashSet<u64>` (recv/acked) kullanır; Rust HashSet-iteration random-seed-SipHash → **process-arası
iteration-order değişir** → ACK-batch-sırası → rate-control-yolu → exact-diagnostik-değerler koşudan-koşuya OYNAR.
**Kod-değişmeden ölçülen-varyasyon (üç-koşu):**
```
qd_max        : 33.3s / 36.7s / 38.7s        (aralık-pin: >1s ✓ hepsinde)
btlbw@70s     : 584 / 1001 / 1251            (aralık-pin: <2100 ✓ hepsinde)
infl_max      : 45× / 54× / 67×
expiry_s      : 0.0 / 33.5                    (0-veya-canlı; aralık-pin qd>1s ✓)
uzun-daralma@125 : 377 / 380 / 389           (aralık-pin: <600 ✓ hepsinde)
```
**SONUÇ:** desen1 exact-sayılarına "bit-for-bit-aynı" karşılaştırması UYGULANAMAZ (RNG-artefaktı, kod-regresyonu-değil).
Güvenilir-non-regression-gate = (A) exact-test-sayıları + (B) aralık-assertion-pass/fail + (C) FEC-off-yapısal.
`sim_determinism.rs` scripted-sim (HashSet-order-bağımsız) AYRI ve deterministiktir (birebir run1==run2 assert'i geçer).

**KARAR-TALEBİ (Abkarada):** 5c-non-regression-metodolojisi = {A+B+C aralık/count-gate} olsun mu (ÖNERİM: EVET, robust);
YOKSA desen1_sim `HashSet→BTreeSet` deterministik-yapılsın-mı-önce (sim-harness-değişikliği; kendisi-pin)? İkincisi
seçilirse ayrı-mini-DR (DR-S5-0) gerekir çünkü sim-harness-mühürlü-pin.

## E. NETEM-NIGHTLY PİNLERİ (§4.4 self-hosted; YEREL-DEĞİL)

Kullanıcı-listesindeki bazı pinler netem-benchmark'tır (self-hosted nightly, §4.4 — bu-ortamda-KOŞULMAZ):
- **DESEN-1 4114→308** (`sender.rs:511`-yorumu "netem-mühür: 4114→308") — netem-daralma-yakınsama.
- **P5 {100×12, S5}** — P5-GATE netem-profili (memory: p5-parasitic-riski TUR-4-sonrası re-prove).
- **qd-52.2s** — netem-senaryosu (yerel-desen1 farklı-parametre: 33–39s).
- **Y3 delivered-EAI** — K6R-netem-mühürlü ([Y3 KALICI-PİN] yorumu desen1:269).

**5c-KURALI:** bu netem-pinleri nightly-CI-gate'idir (§4.4); 5c-yerel-commit'leri bunları-re-measure-EDEMEZ.
Emisyon-açık FEC'in netem-etkisi nightly'de-ölçülür; yerel-gate A+B+C'dir.

---
**ÖZET:** 5c-non-regression-referansı = A (exact-count) + B (aralık-pin) + C (FEC-off-yapısal), her-mini-DR-commit'inde.
desen1-exact-değer bit-comparison GEÇERSİZ (RNG). netem-pinleri nightly. Metodoloji-onayı Abkarada'da (§D karar-talebi).
