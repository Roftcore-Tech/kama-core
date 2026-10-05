# G-S3 (S3 SEAM-FROZEN) — KAPI RAPORU

**Tarih:** 2026-10-06 · **Hazırlayan:** Claude Code (icra) · **Karar sahibi:** Abkarada
**Bu belge karar İÇERMEZ.** Kapının ölçütlerini kanıtlarıyla listeler; dondurma kararı sahibindir.
**Güncelleme 2026-10-05b:** §7 eklendi (B1 `kama-contracts` ve G-S3 kanıt işi sonrası durum); onun
geçersiz kıldığı cümlelerin üstü çizildi. §1–§6'daki satır numaraları hâlâ `77848003` ağacına göredir.
**Güncelleme 2026-10-05c:** §7.3'teki S1 kapsama sayımı güncellendi (Hayri taşıması, PR #92); eski cümlenin üstü çizildi.
**Durum notu (2026-10-05, merge anında güncellendi):** bu raporun dayandığı PR'ların hepsi main'dedir
(#82 `6b2d6bcb` · #83 `e341b020` · #85 `88961ee1` · #86 `ca788d4c` · #84 `285824b6` · #87 `77848003`).
Satır numaraları main `77848003` ağacına göredir (merge öncesi tek tek doğrulandı).

Kapı tanımı (`SPRINT-BACKLOG-FREEZE.md`): A1.1 + A1.2 sayıları · A2.* vektörleri commit · A4.* yeşil · C1.2 yeşil.

## 1. Kalibrasyon sayıları

| Slot | Değer | Yer |
|---|---|---|
| KS-DOS-1 `HALF_OPEN_TTL` | S&F mailbox TTL ile aynı (bugün 7 gün) | `specs/ccr-format-spec.md:3058` |
| KS-DOS-1 `HALF_OPEN_CAP` | 64 kayıt / gönderen kimliği | `specs/ccr-format-spec.md:3059` |
| KS-DOS-1 `PER_SOURCE_INIT_RATE` | 1/s, 32 burst | `specs/ccr-format-spec.md:3061` |
| KS-DOS-1 `PENDING_RESPONSE_TRIAL_CAP` | 16 (= istemci pending INIT tavanı) | `specs/ccr-format-spec.md:3062` |
| KS-GRACE-1 `N_g` | 64 (onaylayan paket #1 sayılır) | `specs/kama-noise-wire-tdd-v2.0.md:525` |
| KS-GRACE-1 `T_g` | 30 s | `specs/kama-noise-wire-tdd-v2.0.md:529` |
| KS-HANDLE-1 | `HANDLE_TOMBSTONE` = S&F mailbox TTL | `specs/ccr-format-spec.md:2724` (slot), `:7` (kapanış kaydı) |
| KS-PN-1 | K = 1024 | `specs/transport-layer-tdd.md:869` |
| KS-AGE-1 | `SESSION_MAX_AGE` = 7 gün | `specs/ccr-format-spec.md:3131` |
| KS-HB-1 | 8 s / 25 s / OS takvimi | `specs/ccr-format-spec.md:3136` |

Kodda sayıların karşılığı: `PENDING_RESPONSE_TRIAL_CAP` yalnız sahtede (`crates/client-core/src/seam_mock.rs:42`);
`N_g` → `GRACE_MIN_PACKETS` (`crates/kama-noise-wire/src/epoch_machine.rs:46`). İkisi de testte spec değeriyle
karşılaştırılıyor (`h_demux_7_cap_is_named_and_matches_spec`, `machine_epoch::n_g_matches_spec`).
`T_g` ve diğer sabitlerin kodda karşılığı YOK (A5 işi).

## 2. Vektörler (A2.2–A2.7)

| Suite | Vaka | Dosya |
|---|---|---|
| `ccr-kdf-v1` | 12 | `vectors/ccr-kdf-v1/ccr-kdf-v1.json` |
| `ccr-ssi1-v1` | 16 | `vectors/ccr-ssi1-v1/ccr-ssi1-v1.json` |
| `ccr-ssr1-v1` | 8 | `vectors/ccr-ssr1-v1/ccr-ssr1-v1.json` |
| `ccr-ssd1-v1` | 15 | `vectors/ccr-ssd1-v1/ccr-ssd1-v1.json` |
| `ccr-deprecated-neg-v1` | 7 | `vectors/ccr-deprecated-neg-v1/ccr-deprecated-neg-v1.json` |
| `ccr-ed25519-neg-v1` | 10 | `vectors/ccr-ed25519-neg-v1/ccr-ed25519-neg-v1.json` |

Toplam 68 vaka; commit `c9119026` (#78).

- **Python referansından üretildiğinin kanıtı:** her dosyanın `generator` alanı `ref/crypto/gen_vectors.py`;
  CI job'u `vectors (SCHEMA.json kapısı, python tarafı)` her PR'da `python3 ref/crypto/gen_vectors.py --check`
  koşar (`.github/workflows/ci.yml:256`) — dosyalar referanstan yeniden üretilip byte-byte karşılaştırılır.
  Referansın kendisi RFC vektörleriyle doğrulanır (`ref/crypto/selftest.py`, commit `b4f3faca`, #68).
- ~~**SSD1'in Rust ile bayt-özdeşliği: KANIT ZAYIF.** 4 Ekim'de #78 hazırlanırken beş SSD1 seal vakası
  `LegacySessionProtector` çıktısıyla tek seferlik karşılaştırıldı ve özdeş bulundu; bu karşılaştırma
  commit'li bir test DEĞİLDİR ve CI'da koşmaz. `ccr-*` vektörlerinin bugün hiçbir Rust tüketicisi yok
  (`crates/` altında bu suite adlarına atıf yok). Tüketim A5.1 / A5.2 "Done" ölçütüdür.~~
  **[2026-10-05b] SSD1 için KAPANDI — commit'li tüketici testi var, CI'da koşuyor (bkz. §7.2).**
  KDF / SSI1 / SSR1 için tüketici hâlâ yok: **pending A5.1 / A5.5** (§7.2).
- PN sürekliliği kararı (#85) vektörleri değiştirmedi: `gen_vectors.py --check` → hepsi "aynı".

## 3. A4.* — sözleşme tipleri, sahteler, sözleşme testleri

| Kalem | Kanıt | Durum |
|---|---|---|
| A4.1 tipler derleniyor | `crates/client-core/src/seam.rs`; commit `62f2744a` (#77); CI `static` + `test` | main'de, yeşil |
| A4.2 MockCrypto mock-fixture | `crates/client-core/tests/noise_mock_fixture.rs` f1–f10; commit `df84e3e5` (#81) + #87 (f10, H-RETRY genişletmesi) | main'de, yeşil (genişletme `77848003`) |
| A4.3 MockTransport mock-fixture | `crates/kama-noise-wire/tests/noise_transport_mock_fixture.rs` f1–f8; commit `df84e3e5` (#81) | main'de, yeşil |
| A3.1 H-DEMUX | `noise_conformance_core/demux.rs`, 7 test; #83 | main'de (`e341b020`), yeşil |
| A3.2 epoch matrisi | `noise_conformance_core/epoch.rs`, 9 hücre + 5b; #84 | main'de (`285824b6`); **4 hücre beyanlı kırmızı** (aşağıda) |
| A4.4 yedi satır | `noise_conformance_core/s3_*.rs`, mock 10 test + identity 1 test; #87 | main'de (`77848003`), yeşil; ~~H-HANDLE tip yarısı pending B1~~ **[2026-10-05b] tip yarısı B1 ile kapandı (§7.1)** |

Koşucu: `scripts/ci/noise-conformance.sh --target mock|machine|identity|real`; CI job'u
`seam conformance (App↔Transport; hedef=mock|real)` içinde dört adım. Kapsama beyanı:
`scripts/ci/noise-seam-coverage.json` ~~(8 satır mock: green, 1 satır pending B1; hepsi real: pending A5)~~ **[2026-10-05b] 9 satır green, 1 satır static (H-SPLIT), 0 beklemede; real: pending A5 (§7.1)**.

### A3.2 kırmızı hücreler (düzeltilmedi; A5.4)

| Hücre | Beklenen | Gözlenen | Yer |
|---|---|---|---|
| 5b | `N_g` tam 64, kurucu argümanından bağımsız | bütçe = `max(argüman, GRACE_MIN_PACKETS)` | `crates/kama-noise-wire/src/epoch_machine.rs:57-67` |
| 6 | onaydan 30 s sonra grace kapanır | makinede zaman girdisi ve `T_g` sabiti yok | `epoch_machine.rs:112-127` |
| 7 | paket ve süre koşulu, hangisi önce | paket yarısı tutuyor; süre yarısı kurulamıyor | `epoch_machine.rs:112-127` |
| 9 | legacy zarfta ≤2 anahtar denemesi, sayaçla gözlenir | `LegacySessionProtector` tek anahtar; deneme ve sayaç yok | `crates/kama-noise-wire/src/legacy.rs:76-116` |

Beyan: `epoch.rs` `DECLARED_RED`; beyansız kırmızı ya da yeşile dönmüş beyan testi düşürür.

## 4. Statik kapılar

| Kapı | Kanıt | Durum |
|---|---|---|
| C1.2 `layer_check.py` | CI job `static (fmt + clippy)`, `.github/workflows/ci.yml:56-57`; commit `52083030` (#80); `--root` eki #86 | yeşil |
| C1.4 `no_inline_expected.py` | aynı job; main'de 13 conformance dosyası taranıyor | yeşil |
| `seam-api-drift` | CI job `seam conformance`; S1 trait bloğu ↔ ayna | yeşil |

## 5. Bu raporun dayandığı PR'lar

| PR | İçerik | Hosted CI (merge öncesi son uç) | main commit |
|---|---|---|---|
| #82 | S1 §3 `DeadlineExpired` cümlesi | koşmaz (yalnız `.md`) | `6b2d6bcb` |
| #83 | A3.1 H-DEMUX + S1 §3 pending tavanı cümlesi | 11/11 | `e341b020` |
| #85 | PN sayacı epoch'lar boyunca sürekli | 11/11 | `88961ee1` |
| #86 | `layer_check.py --root` | 11/11 | `ca788d4c` |
| #84 | A3.2 epoch matrisi | 11/11 (main'e eşitlenmiş uç `bd112705`) | `285824b6` |
| #87 | A4.4 yedi satır + MockCrypto genişletmesi | 11/11 (main'e eşitlenmiş uç `2b5cf06a`) | `77848003` |

Her squash-merge'den sonra main'in ağacı, merge öncesi hesaplanan beklenen ağaçla karşılaştırıldı; hepsi aynı.

## 6. Açık kalanlar (adıyla ve hangi kapıyı beklettiğiyle)

| Açık | Bekleyen kapı |
|---|---|
| ~~H-HANDLE tip yarısı: `HubHandle` newtype + trybuild negatifi~~ | ~~B1 (`kama-contracts`); G-S3'ü bekletip bekletmeyeceği sahip kararı~~ **KAPANDI — B1 (§7.1)** |
| A3.2'nin 4 kırmızı hücresi (5b, 6, 7, 9) | G-IMPL-S3 (A5.4 Done ölçütü); G-S3 için "matris yeşil" ölçütünün beyanlı kırmızıyla sağlanmış sayılıp sayılmayacağı sahip kararı |
| `real` hedefi yok (`GAP-S3-REAL-A5`) | G-IMPL-S3 |
| ~~`ccr-*` vektörlerinin Rust tüketicisi yok; SSD1 bayt-özdeşliği commit'li test değil~~ **[2026-10-05b] SSD1 kapandı (§7.2).** Kalan: `ccr-kdf-v1` tüketicisi → **pending A5.1**; `ccr-ssi1-v1` / `ccr-ssr1-v1` tüketicisi → **pending A5.5**; `ccr-ed25519-neg-v1` ve `ccr-deprecated-neg-v1` tüketicisi → A5 (imza doğrulama / eski format reddi yazılınca) | G-IMPL-S3 (~~A5.1 / A5.2~~ A5.1 / A5.5) |
| Mutasyon sınaması yapılmadı (C1.6) | kapı değil (NON-GATING) |
| A3.3 hub yarım-açık sınır testleri | kapı değil (NON-GATING; hub-server iskeleti dolunca) |
| `ccr-format-spec.md:2808` ve `transport-layer-tdd.md:676-677`: replay identity'de `epoch` — tek pencere kararından sonra fazlalık | ~~karar bekliyor (#85'te not edildi); G-S3'ü bekletmez~~ **[2026-10-05b] karar verildi (epoch kimlikten çıkar); ayrı spec PR'ında uygulanıyor (iş emri §2)** |
| ~~S1 tarafında `connect()` → `Backpressure` (pending tavanı) için test yok~~ | ~~G-S1 (S1 §6'da satırı yok)~~ **KAPANDI — S1 §6 H-PENDING-CAP + `seam_conformance_core::h_pending_cap` (§7.3); bugün yalnız mock hedefinde** |
| "Sahteye karşı yeşil" yalnız sahtenin sözleşmeye uyduğunu gösterir | G-IMPL-S3'te aynı gövdeler gerçek koda koşunca kapanır |

## 7. Güncelleme — 2026-10-05b (B1 `kama-contracts` + G-S3 kanıt işi)

Bu bölüm iki PR'ın getirdiğini kaydeder: **B1** (`kama-contracts`, #89) ve **G-S3 kanıt** (iş emri §3).
Yazıldığı anda ikisi de main'e girmemiştir; aşağıdaki yollar o PR'ların ağacına göredir.

### 7.1 B1 ile değişenler

| Kalem | Önce | Sonra |
|---|---|---|
| S1/S3 trait ve tiplerinin yeri | `kama-app/src/lib.rs`, `client-core/src/seam.rs`, test-yerel ayna | `crates/kama-contracts/src/{s1,s3,handles}.rs` (harici bağımlılık 0) |
| H-HANDLE tip yarısı | pending | **yeşil**: `HubHandle(u32)` newtype; trybuild negatif testi `crates/kama-noise-wire/tests/ui/hub_handle_is_not_session_handle.rs`; ayrıca `kama-contracts` içinde derleme-zamanı dönüşümsüzlük iddiası |
| `MockCrypto` | `crates/client-core/src/seam_mock.rs` | `crates/kama-noise-wire/src/seam_mock.rs` (`test-util`); `PENDING_RESPONSE_TRIAL_CAP` sabiti de orada |
| Katman kuralı | L1–L3 | + C1–C5 (`scripts/ci/layer_check.py`); `kama-noise-wire → client-core` dev-dependency'si kalktı |
| Drift kapısı | S1 ↔ test aynası | S1 ve S3 ↔ `kama-contracts` kaynağı (`seam-api-drift.py --seam s1\|s3`) |
| S3 §6 ↔ kapsama | ayrışık (kimse ölçmüyordu) | hizalı: §6'ya `H-REPLAY-POISON` eklendi, beyana `H-SPLIT` (statik; kanıt `layer_check.py`) |
| Kapsama beyanı | 8 green, 1 pending B1 | 9 green, 1 static, 0 beklemede; hepsi real: pending A5 (H-HANDLE/type hedeften bağımsız) |

### 7.2 Vektör tüketimi

| Suite | Vaka | Rust tüketicisi | Durum |
|---|---|---|---|
| `ccr-ssd1-v1` | 15 | `crates/kama-noise-wire/tests/noise_conformance_core/ssd1_vectors.rs` (`vectors_ssd1::*`, 3 test) | **yeşil** — 15/15 vaka koşuluyor: 5 `seal` (AAD, nonce96 ve wire byte'ları BAYT-ÖZDEŞ), 10 `open` (3 kabul: plaintext aynı; 7 ret: karar ve gerekçe aynı, pencere ilerlemiyor) |
| `ccr-kdf-v1` | 12 | yok | **pending A5.1** (D1–D7 türetimi yazılınca) |
| `ccr-ssi1-v1` | 16 | yok | **pending A5.5** (oturum kontrolü + enjekte edilebilir entropi) |
| `ccr-ssr1-v1` | 8 | yok | **pending A5.5** |
| `ccr-ed25519-neg-v1` | 10 | yok | pending A5 (katı kip imza doğrulama) |
| `ccr-deprecated-neg-v1` | 7 | yok | pending A5 (eski format reddi) |

SSD1 testinin sınadığı kod bugünkü `LegacySessionProtector`'dır (22 B zarf), S3 trait'leri değil. Koşucu:
`scripts/ci/noise-conformance.sh --target vectors`; CI'da `seam conformance` job'unun bir adımı ve `test`
job'u. Beklenen byte'ların tamamı vektör dosyasından okunur (`no_inline_expected.py` yeşil).

Bir gözlem (vektör dosyası DEĞİŞTİRİLMEDİ): `open.too-short` vakası `expected_counter = 5`,
`seen_counters = []` veriyor; bu ikisi birlikte kurulabilir bir alıcı state'i değil. Vaka uzunluk
kontrolünde reddedildiği için state'e bakılmıyor ve test bunu açıkça yalnız o ret türü için kabul ediyor.

### 7.3 S1: pending tavanı

S1 §6'ya `H-PENDING-CAP` satırı eklendi; gövde `crates/kama-integration/tests/common/seam/contract.rs`
`rows::h_pending_cap`. Tavan hedefin adlandırılmış sabitinden okunur ve testte ccr §35.15'teki sayıyla
karşılaştırılır. `scripts/ci/seam-coverage.json`: ~~14 satırın 2'si `mock_only`, 12'si `not_implemented`.~~
**[2026-10-05c — Hayri taşıması]** 14 satırın 12'si `mock_only` (bütün davranış satırları: Hayri'nin `feature/app`'teki
yedi satırı — altısının gövdesi özgün testlerden yazıldı, H-ROUNDTRIP'inki süitte zaten vardı — + dört eksik satır +
H-PENDING-CAP), 2'si `static` (H-NO-CRYPTO-EDGE, H-NO-PLANE1; kanıt `layer_check.py`), 0'ı `not_implemented`. Mock hedefi
artık `kama-app`'in `MockTransport`'udur (tek kopya).

### 7.4 Silme kapısı

`scripts/ci/word_coverage.py` + `.github/workflows/spec-word-coverage.yml`: spec/karar belgesi değişen her
PR'da koşar; taban sürümdeki her kelimenin yeni sürümde en az o kadar geçmesini ister. Bu rapor da kapsamdadır.

