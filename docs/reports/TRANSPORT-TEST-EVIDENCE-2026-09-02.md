# Kama Transport — Test Kanıt Raporu

**Tarih:** 2026-09-02 · **Kapsam:** main @ `ca65e6cb` (PR #45 squash-merge) + henüz birleşmemiş `origin/test` @ `74b16183`
**Amaç:** transport katmanı için bugüne kadar koşulmuş testlerin ve kalite kapılarının **kanıta dayalı** dökümü — neyin gerçekten koştuğu, neyin koşmadığı, bilinen kör noktalar dahil.

> Dürüstlük notu: bu rapor yalnız gerçekten koşmuş ve çıktısı kayıtlı kapıları "yeşil" sayar.
> "CI-green" iddiaları run-ID + headSha ile bağlanır; sayı taşıma yapılmaz (RULE-7).

---

## 1. Hosted-CI tam mührü (main'e birleşen içerik)

**Run `33247485008` — 10/10 SUCCESS**, headSha `23ce05d7` = PR #45 tepe commit'i = main'e birleşen içerik (2026-08-29).

| # | Job | Sonuç | Süre |
|---|---|---|---|
| 1 | quality (fmt + clippy -D + test --all-features + release build) | ✅ | 1s 06d |
| 2 | semantic-matrix: default-workspace | ✅ | 41d 50s |
| 3 | semantic-matrix: all-features | ✅ | 57d 08s |
| 4 | semantic-matrix: experimental-cold-start | ✅ | 42d 23s |
| 5 | semantic-matrix: experimental-fec-budget | ✅ | ~72 s |
| 6 | semantic-matrix: diagnostic (diag-sched-telemetry) | ✅ | 38d 08s |
| 7 | security (cargo-audit + cargo-deny) | ✅ | ~43 s |
| 8 | miri (`-p crypto -p ccr-codec`) | ✅ | ~64 s |
| 9 | fuzz-smoke (5 hedef × 60 s) | ✅ | 7d 10s |
| 10 | aead-differential (RustCrypto == libsodium byte-eşitlik oracle'ı) | ✅ | ~33 s |

**semantic-matrix neden 5 konfigürasyon:** `--all-features` default build'in vekili DEĞİLDİR — iki gerçek kırılma yalnız
default/tekil-feature build'lerde görünüp `--all-features`'ta gizlenmişti (DR-RATE-RAMP-2B §A1 kaydı). Matrix ayrıca
`COMPILED == 0` durumunda hard-fail eder (hiç test binary'si derlenmeyen konfigürasyonun sessiz-yeşili engellenir).

**Bu mühre giden CI-destanı (4 kök-neden, hepsi teşhisli):** billing-bloğu → 5. koreografi-enstrümanının
feature-gate izolasyonu (`2471e1b6`) → yank'li chacha20 0.10.1 duplicate'i (bench→quinn→rand zinciri; `23ce05d7`)
→ ağ-kesintisi-retry. Ara run'lar `33183689458` ve `33185870443` failure olarak kayıtlıdır — yeşil, ancak son
run'da elde edildi.

## 2. Lokal kapı: `scripts/gate.sh` — 13 kontrol

Taze `git worktree` + izole `CARGO_TARGET_DIR` + `-j2` seri koşu şartıyla:
step 0 `cargo fmt --all -- --check` · step 1 compile-closure (`--workspace --all-targets --all-features`) ·
step 2–6 beş konfigürasyonda `cargo test --workspace` · step 7a–7e beş konfigürasyonda `clippy -D warnings` ·
step 8 worktree-pristine (`git status --porcelain` boş).

Bilinen sınırlar (kayıtlı, gizlenmiyor):
- Negatif-kontrollerden yalnız **kasıtlı-compile-hatası → step 1 kırmızı** kanıtlandı; kasıtlı-failing-test ve
  kasıtlı-tracked-dosya-değişimi kontrolleri `pending` (EXECUTION-STATE.json `gate_negative_controls`).
- step 0, branch'te PR yokken hosted CI'ın hiç koşmaması kör noktasına karşı eklendi (POST-CI-FORMAT-DRIFT
  vakası: 136 hunk birikmişti).

## 3. netem kanıt zinciri (7 düşman profili)

`harness/netem-profiles.sh` (transport-layer-tdd §4.4.2): p1_loss50 · p2_loss80 · p3_loss95 · p4_jitter ·
p5_revpath (ileri %5 / geri %80 — asimetrik) · p6_blackout (dinamik) · p7_bbr (%2 kayıp — QUIC'in kazanabildiği
dürüstlük profili). Kalıcı `pin_neigh` ARP-pinlemesi dahildir (pinlenmemiş sub-kbps ölçümü kontamine ilan edilmiştir).

Ana kanıt kampanyaları (ayrıntı ve corrigendum'larıyla birlikte: whitepaper v0.2/v0.3 ve
`reports/kapi-eval-v1/`, `reports/s2-region/`, `reports/dr-arq-product-tail-1/`):
- **KAPI-eval (r5, ön-kayıtlı, Holm-düzeltmeli):** p2 (%80 kayıp) `kama-tail-v1` medyan 0,505 vs `q-dgram-arq`
  0,040; eşleştirilmiş ortalama fark **+38,4 puan** [%95 CI +31,5, +44,9], n=50. NEGATİF bulgu da kayıtlı:
  h3 patlamalı-kayıpta Kama 0,92–0,97 vs Quinn 1,00 → ürün P0 düzeltmesine dönüştü.
- **DR-ARQ-PRODUCT-TAIL-1 (iki-netns/netem, 2290 satır):** terminal-tail horizon-tamamlama
  **OFF 8/360 (%2,2) → ON 329/360 (%91,4)**; probe üst-sınırı hiç aşılmadı (global max = cap = 3);
  2290 satırda invariant ihlali 0; adversaryal satırlarda spurious retransmit 0. Bilinen tek başarısız hücre
  kayıtlı: terminal-tail L10-med ON 0/30 (sabit cap-3'ün 3-kayıplı kuyrukta erken tükenmesi — tuning kalemi).

## 4. Test envanteri — main tarafı

- adaptive-transport: 26 modül, 17.338 kaynak-LOC, `#![forbid(unsafe_code)]`; ~330 test / 33 entegrasyon dosyası
  (tv_ack, tv_seq, tv_rtt, tv_bw, tv_src, tv_pace, tv_loss, tv_arq, tv_emergency, tv_fusion, rule_coverage_map,
  cold_start_liveness, dr_cs1_acceptance (25), ek4_a2_liveness, desen1_sim (5 `#[ignore]` ağır sim) …).
- Golden-kilitli davranış pinleri: `inv_rate_1_loss_declaration_preserves_phase_gain` (INV-RATE-1 faz-gain
  koruması), q18_check (füzyon kalibre-kusuru pini), FEC sealed-ordering testleri, K-4 tx-oran pinleri.
- kama-noise-wire: ~90+ test (D1 sprint'leri) + trybuild sealed-type kapısı + AEAD differential-oracle.
- Fuzz: 5 hedef (`ack_decode`, `fec_decode`, `noise_wire_decode`, `protector_open`, `wire_v0`).

## 5. Test envanteri — `origin/test` branch'i (test mühendisi: h4s4nk44n; HENÜZ BİRLEŞMEDİ)

9 commit (2026-08-07 → 08-09), **72 dosya, +10.532 satır; ürün kaynak dosyalarına (`crates/*/src`) SIFIR dokunuş**
(üç bağımsız yöntemle doğrulandı). İçerik:

### 5.1 Mutation-güdümlü test seti (251 → 443 test)
Dört `cargo mutants` kampanyası, ölçülmüş çıktılarla:
- kama-noise-wire (tüm crate): 436 mutant → 36 missed / 349 caught / 50 unviable / 1 timeout; hedefli 16 test 34'ünü kapatıyor.
- kama-noise-wire `pn.rs`: 87 mutant, missed **13 → 2** (kalan 2 eşdeğer-mutant ilan edilmiş).
- adaptive-transport (8 dosya): 505 mutant → 53 missed / 430 caught; hedefli 11 test 16'sını kapatıyor.
- kama-transport-schema: 54 mutant → 11 missed / 39 caught.
Adlandırılmış kör noktalar dürüstçe kayıtlı (sabit-döndüren accessor'lar ölçüm-enstrümanı olarak kullanıldığından
mutant'ı yaşatıyor; `param.rs`'in hiç test dosyası yoktu → eklendi). 8 mükerrer test silindi.

### 5.2 Kani biçimsel doğrulama — 20 ispat, 5 dosya
pn (8) · loss (3) · fusion (4) · seq (3) · prefix (2). **İki gerçek kusur bulundu ve KASTEN kırmızı bırakıldı**
(düzeltilince yeşile dönüp haber verecek şekilde; nightly'de `continue-on-error`):
- **K-1 — `pn.rs:52` taşma:** `reconstruct()` (RFC 9000 A.3 uyarlaması) `seq.rs:26`'daki taşma guard'ının
  eşleniğinden yoksun. Erişilebilirlik SINIRLI kanıtlı: `expected < SEND_PN_CAP (2^62)` altında Kani SUCCESSFUL —
  mevcut çağıranlarla tetiklenemez ("system safe today"); tetik bölgesi u64 uzayının 2^-33'ü (sınır testleri +
  proptest + fuzz'ın kaçırma sebebi). Not: `pn_trunc` AEAD doğrulamasından ÖNCE cleartext outer'dan okunur.
- **K-2 — `loss.rs:44` sıfıra bölme:** `scale_duration` `den==0` guard'sız; `den` public `LossConfig.time_den`'den
  gelir ve `with_config()` yolu açık (branch'in kendi testi propagasyonu ölçüyor). Statü: `[BEKLİYOR — Abkarada]`
  (guard mı, `LossConfig::validate()` mı).

### 5.3 Soak (uzun-oturum) testleri — repoda İLK
`soak_long_session.rs` (2 test): sayan `#[global_allocator]` + sanal saat; oracle **plato** (sıfır-sızıntı değil):
ısınma sonrası tepe bellek dört çeyrekte artmıyor; ACK-açlığında durum sınırlı kalıyor. Sender'ın 7 büyüyebilir
koleksiyonundan yalnız `tombstones`'un görünür cap'i olduğu (4096) kayda geçirildi. Süre: 0,27 s release.

### 5.4 Zamanlama yan-kanal ölçümü (teşhis; CI kapısı DEĞİL)
`timing_side_channel.rs`: 7 reddetme sınıfı, kalibrasyon-önce yöntem (gürültü tabanı → sinyal → `signal > 5×noise`,
aksi halde "NOT MEASURABLE" — "sızıntı yok" DENMEZ), N=40.000/sınıf, A/B-interleaved.

### 5.5 CI yeniden-yazımı + nightly (test branch'inin en kritik katkısı)
Ölçülmüş CI kör-nokta bulguları:
- **BUL-01:** ADR-18/CLAUDE.md'nin "ZORUNLU kalite kapısı" dediği nightly netem kıyası **CI'da hiç yoktu** ve
  `.gitignore`'daki `/results` satırı baseline'ı repodan dışlıyordu. Branch bunu düzeltir: `compare.py --gate`
  (tolerans ABS 0,03 / REL 0,05, `gate-report.json`, regresyonda exit 1) + 7-profil×5-tekrar committed baseline
  (36 dk koşuldu, kapının negatif-testi doğrulandı).
- **BUL-02:** main'in miri hedefi (`crypto`, `ccr-codec`) **3-satır iskelet** → miri her PR'da 0 test koşup yeşil
  yanıyordu (sahte güven). Yeniden hedeflendi (kama-transport-schema, 9 test) + "0 test = fail" assert'i.
- **BUL-03:** `--all-features` altında trybuild sealed-type kapısı 0 test koşuyordu (`test-util` cfg-out) →
  meta-assertion eklendi.
- Yeni job'lar: **asan**, **msan** (SIMD bağımlılıkları hedefli), **coverage** (`--fail-under-lines 92`;
  ölçülen 93,65 %), release-mode test (CI-04: release'te overflow wrap'lenir + `debug_assert` düşer — main yalnız
  build ediyordu), `--no-default-features` görünümü (CI-05), her job'a timeout (CI-09).
- `nightly.yml` (YENİ): fuzz-deep (6 hedef × 30 dk + corpus cache/cmin), branch-coverage (%78 kapı; ölçülen
  81,21 %), sim-nightly (ignored ağır simler), **netem-gate** (self-hosted; runner atanmadı — bilinçli fail),
  kani (K-1 düzeltilene dek continue-on-error).
- `EVIDENCE.md` (956 satır): merge-base'de temiz klonda "yalnız ölçüm, yorum yok" ham dosya — 11.172 src LOC /
  6.207 test LOC / 260 `#[test]` envanteri, kapsam, sanitizer, fuzz fizibilite, exit kodlarıyla.

### 5.6 Birleştirme görünümü (bilgi; karar §7)
`git merge-tree` ile ölçüldü: **5 çakışma** — `.github/workflows/ci.yml` (iki taraf da baştan yazdı; anlamsal
uzlaştırma gerekir), `fuzz/Cargo.toml` + `fuzz/Cargo.lock` (iki bağımsız fuzz seti: 5+6 farklı isimli hedef),
`crates/adaptive-transport/Cargo.toml` (`[lints]` ek satırı), `.gitignore`. 24 test dosyası + nightly.yml +
baseline temiz auto-merge. Riskler: kapsam eşikleri (%92/%78) merge-base'e göre kalibre — yeni main'in büyüyen
yüzeyinde doğrulanmadan kapı yapılmamalı; Kani ispatları ve 207 test, eski API imzalarına karşı derleniyor —
merge sonrası derleme/uyarlama gerekebilir.

## 6. Bilinen açık kalemler (test-kanıt perspektifinden)

1. K-1 / K-2 ürün kodunda **hâlâ açık** (test branch'i yalnız ispatları getiriyor; karar owner'da).
2. Nightly netem kapısı için self-hosted runner atanmamış (BUL-01 düzeltmesi merge edilse bile runner şart).
3. miri'nin gerçek kapsamı dar (schema-crate); noise-wire `-Zmiri-disable-isolation` ister, fec-codec 1.913 s'te
  bitmedi → CI dışı.
4. Binary blackout-survival metriği (BENCH-01) hâlâ proxy (bkz. olgunluk raporu).
5. gate.sh'in iki negatif kontrolü `pending`.
