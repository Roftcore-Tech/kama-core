# Kama Transport v1 — dizin

**Ne:** hostile/zayıflamış ağlar (yüksek ve burst kayıp, düşük bant, yüksek RTT, asimetrik yol,
bozuk geri-besleme) için veri-öncelikli, mesaj-granülü, ARQ+FEC'li, yönlü-geri-besleme okuyan
uyarlanır güvenilir taşıyıcı. Kripto TUTMAZ (INV-ENGINE-SOC); güvenlik katmanının müşterisidir.

| | |
|---|---|
| **Durum** | **FROZEN / READY** (Faz-6, 2026-09-11) — `reports/transport-v1/TRANSPORT-V1-CLOSURE.md` |
| **Kanonik kimlik** | profil `KamaV1Profile` (`crates/client-core/src/runtime.rs`) · benchmark `kama-v1-full` (`client_core::runtime::TRANSPORT_V1_PROFILE_ID`) · mimari `transport-v1` · manifest şeması `kama-bench-manifest/v1` |
| **Hedef zarf** | [`HOSTILE-ENVELOPE.md`](HOSTILE-ENVELOPE.md) — doğrulanmış çekirdek / uzmanlaşma / kısmen doğrulanmış / dışarıda |

## Mimari belgeler (`docs/` = mimari NEDİR)

| belge | içerik |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | donmuş mimari: veri yolu, ACK/SACK, kayıp modeli, ARQ, tail probe, FEC, dönüş yolu, pacing, telemetri |
| [`PRODUCT-PROFILE.md`](PRODUCT-PROFILE.md) | `KamaV1Profile` alan-alan sözleşmesi (SHIP/EXPERIMENTAL, neden, kanıt) |
| [`INVARIANTS.md`](INVARIANTS.md) | INV-001..016 — gelecek işin bilinçli koruması ya da sürüm-kırması gereken kurallar |
| [`KNOWN-LIMITATIONS.md`](KNOWN-LIMITATIONS.md) | LIM-001..007 |
| [`TRANSPORT-CRYPTO-HANDOFF.md`](TRANSPORT-CRYPTO-HANDOFF.md) | transport'un gelecek güvenlik katmanına verdiği sınır (kripto tasarımı DEĞİL) |
| [`EVOLUTION.md`](EVOLUTION.md) | gelecek adaylar, tetikleyiciler, sürümleme kuralı, değişiklik beyanı politikası |
| [`adr/`](adr/) | freeze · zero-ack bootstrap · FEC v1 rolü · MinimalParity istisnası · hostile doğrulama sözleşmesi |

## Kanıt (`reports/` = oraya NASIL gelindi)

`reports/transport-v1/`: `TRANSPORT-V1-CLOSURE.md` (nihai hüküm) · `PHASE5-T0-T7-REPORT.md` ·
`phase5/{EXPECTATION,RESULTS-RUNNING,ERRATUM-1-blackout-boundary}.md` · `PHASE4-REPORT.md` ·
`BENCHMARK-METHODOLOGY.md` (donmuş) · `NIGHTLY-INFRA-WAIVER.md` · `FINAL-MECHANISM-AUDIT.md`.
Önceki fazlar: `reports/transport-v1-audit/`, `reports/phase{1,2,3}-*/`.
Ham Faz-5 verisi (repo dışı, kalıcı disk): `/home/ryuzaki/kama-phase5-results/stageB/`.

## Bilinen sınırlar (özet)

Sıfır-geri-besleme kuruluşu garanti edilmez (LIM-001) · FEC nedensel katkısı kurulmamış (LIM-002)
· dönüş-bant doygunluğu zayıf doğrulanmış (LIM-004) · TCP tam-tel kıyası yapılamaz (LIM-005) ·
§11 acil yolu saha-doğrulanmamış (LIM-006).

## Gelecek değişiklik nasıl yapılır

Transport runtime kararını değiştiren HER değişiklik EVOLUTION §9.5/§22'deki altı beyanı yapar,
etkilenen T-hücrelerini ADR-hostile-validation-contract ile yeniden koşar ve kimliği
`transport-v1.x`/`transport-v2` yapar. `kama-v1-full` adı korunarak davranış değiştirilemez.
Spec: repo kökü `specs/` (engine-spec, TDD; `[AMENDMENT-N]` deseni).
