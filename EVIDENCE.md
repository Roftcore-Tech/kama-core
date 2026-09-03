# EVIDENCE.md — kama-core ham test/kapsam kanıtı

> Bu dosya yalnız ölçüm içerir. Yorum/karar/değerlendirme yoktur.
> Her adım: (a) çalıştırılan tam komut, (b) ham çıktı (uzunsa toplam satır sayısı + son 40 satır), (c) çıkış kodu.

- Klon: temiz klon (`git clone` → detached checkout), commit `bb61c2da2898b975ca52f836381c55aa36e886b9`.
- Klon yolu (çalışma target'ı ayrı; mevcut çalışma ağacı kirletilmedi): `scratchpad/kama-core-clean`.
- Toolchain: `rust-toolchain.toml` pini `1.96.0` (workspace kökünde override aktif). Nightly gereken adımlar (miri, fuzz, branch-cov, ASan) ayrı işaretlendi.

## ADIM 0 — Ön koşul: toolchain + araç kurulumu

Bu ortamda başlangıçta HİÇBİR Rust toolchain kurulu DEĞİLDİ (`rustc`/`cargo`/`rustup` bulunamadı).
Aşağıdaki araçlar kuruldu; ham kurulum logları `scratchpad/logs/00-rustup-install.log` ve
`scratchpad/logs/01-tooling-install.log` içindedir.

Kurulan bileşenler ve çıkış kodları:

| bileşen | sonuç |
|---|---|
| rustup + rustc/cargo 1.96.0 (profile default; clippy, rustfmt, rust-src) | exit=0 |
| llvm-tools-preview (stable) | exit=0 |
| nightly toolchain (minimal + miri + rust-src + llvm-tools-preview) | exit=0 |
| cargo-llvm-cov (cargo install --locked) | exit=0 |
| cargo-nextest (cargo install --locked) | exit=0 |
| cargo-fuzz (cargo install --locked) | exit=0 |
| cargo-audit (cargo install --locked) | (aşağıda ADIM 6.4/ORTAM'da sürüm) |
| cargo-deny (cargo install --locked) | (aşağıda ADIM 6.4/ORTAM'da sürüm) |

Sistem kütüphanesi notu: `libsodium.so.23` (runtime, sürüm 1.0.18) mevcut; `libsodium-dev`
(pkg-config `.pc` + `.so` symlink) YOK → `--all-features` yolundaki `sodiumoxide v0.2.7`
kaynak-derleme (vendored, C-derleyici ile) yapar.


---

## ADIM 1 — Envanter

Workspace 13 crate (resolver=2). `crates/{crypto, ccr-codec, client-core, discovery, hub-server, ra-ca, admin-server}` yalnız `lib.rs` stub (3 satır: `//! ...` + boş). Dolu crate'ler: `adaptive-transport, kama-noise-wire, kama-fec-codec, kama-transport-schema, kama-integration, bench`.

### 1.1 Crate başına satır (wc -l) ve test-fn sayısı

Komut (özet):
```
for d in crates/*/; do
  src_loc = wc -l of src/**/*.rs ; test_loc = wc -l of tests/**/*.rs
  #[test] = grep -c '#\[test\]' (src+tests) ; tokio = grep -c '#\[tokio::test'
done
```

| crate | src_loc | src_files | test_loc | test_files | #[test] | #[tokio::test] |
|---|---|---|---|---|---|---|
| adaptive-transport | 5963 | 19 | 2289 | 14 | 119 | 3 |
| admin-server | 3 | 1 | 0 | 0 | 0 | 0 |
| bench | 1553 | 7 | 0 | 0 | 5 | 5 |
| ccr-codec | 3 | 1 | 0 | 0 | 0 | 0 |
| client-core | 3 | 1 | 0 | 0 | 0 | 0 |
| crypto | 3 | 1 | 0 | 0 | 0 | 0 |
| discovery | 3 | 1 | 0 | 0 | 0 | 0 |
| hub-server | 3 | 1 | 0 | 0 | 0 | 0 |
| kama-fec-codec | 575 | 4 | 831 | 4 | 25 | 0 |
| kama-integration | 9 | 1 | 648 | 2 | 9 | 0 |
| kama-noise-wire | 2607 | 18 | 2250 | 16 | 93 | 0 |
| kama-transport-schema | 444 | 5 | 189 | 1 | 9 | 0 |
| ra-ca | 3 | 1 | 0 | 0 | 0 | 0 |

TOTALS: src rs = **11172** satır; tests rs = **6207** satır; `#[test]` = **260**; `#[tokio::test]` = **8**.
(Not: sayımlar src+tests içindeki `#[test]`/`#[tokio::test]` attribute string eşleşmesidir; bench src'deki 5+5 birim/tokio testleri dahil.)

### 1.2 `#[ignore]` taşıyan testler (dosya:satır → fn)

Toplam `#[ignore]` = **5** — hepsi `crates/adaptive-transport/tests/desen1_sim.rs` içinde:

| dosya:satır | fn | ignore sebebi |
|---|---|---|
| desen1_sim.rs:267 | `desen1_expiry_pin_3a2` | "ağır sim; manuel: cargo test --test desen1_sim -- --ignored --nocapture" |
| desen1_sim.rs:294 | `scan_long_narrowing_convergence` | "ağır sim; manuel" |
| desen1_sim.rs:398 | `recalibrate_cadence` | "ağır sim; manuel" |
| desen1_sim.rs:415 | `desen1_period_counters` | "ağır sim; manuel" |
| desen1_sim.rs:426 | `scan_subkbps_anchor` | "ağır sim; manuel" |

### 1.3 Her crate `[features]`

Yalnız **kama-noise-wire**'da `[features]` var; diğer 12 crate'te `[features]` bölümü YOK.

```
kama-noise-wire/Cargo.toml:
[features]
# test-constructor'ı dış-crate entegrasyon-testlerine açar (production'da KAPALI)
test-util = []
# RustCrypto==libsodium AEAD differential-oracle. TEST-ONLY, DEFAULT-KAPALI.
libsodium-oracle = ["dep:sodiumoxide"]
```

Hiçbir crate'te `default = [...]` yok (no-default-features farkı yalnız feature'lı crate'te belirir).

### 1.4 `cargo tree --workspace --depth 1`  (exit=0)

Öz bağımlılıklar (dolu crate'ler):
```
adaptive-transport → thiserror v2.0.18, tokio v1.52.3  ;  [dev] tokio
admin-server       → (yok)
bench              → adaptive-transport, clap v4.6.1, quinn v0.11.11, rcgen v0.13.2,
                     rustls v0.23.41, serde v1.0.228, serde_json v1.0.150, thiserror, tokio
ccr-codec/client-core/crypto/discovery/hub-server/ra-ca → (yok; stub)
kama-fec-codec     → kama-transport-schema, reed-solomon-simd v3.1.0, thiserror ; [dev] kama-transport-schema
kama-integration   → kama-fec-codec, kama-noise-wire, kama-transport-schema ; [dev] zeroize v1.9.0
kama-noise-wire    → chacha20poly1305 v0.10.1, kama-transport-schema, thiserror, zeroize
                     ; [dev] proptest v1.11.0, trybuild v1.0.118
                     ; --all-features EKLER: sodiumoxide v0.2.7
kama-transport-schema → thiserror ; [dev] blake2 v0.10.6
```
`kama-noise-wire` default vs `--all-features` tek fark: **sodiumoxide v0.2.7** (native libsodium link).


---

## ADIM 2 — Koşum matrisi

Tüm 8 komut **exit=0** (workspace testleri geçti). Özet:

| # | komut | exit | süre (s) | log satır | test result |
|---|---|---|---|---|---|
| 2.1 | cargo test --workspace | 0 | 74.4 | 834 | 261 passed; 0 failed; 5 ignored |
| 2.2 | cargo test --workspace --all-features | 0 | 100.7 | 638 | 263 passed; 0 failed; 5 ignored |
| 2.3 | cargo test --workspace --no-default-features | 0 | 5.5 | 635 | 261 passed; 0 failed; 5 ignored |
| 2.4 | cargo test --workspace --all-features --release | 0 | 129.3 | 783 | 263 passed; 0 failed; 5 ignored |
| 2.5 | cargo test --workspace -- --ignored | 0 | 12.2 | 366 | 5 passed (ignored sim'ler); 0 failed |
| 2.6 | cargo test --workspace --doc | 0 | 1.0 | 74 | 1 passed (yalnız adaptive_transport doc-test) |
| 2.7 | cargo test -p kama-noise-wire --test trybuild | 0 | 10.6 | 36 | **running 1 test** → 1 passed |
| 2.8 | cargo test -p kama-noise-wire --all-features --test trybuild | 0 | 2.2 | 9 | **running 0 tests** → 0 passed |

Notlar (ham çıktıdan):
- 2.1/2.3 birim+entegrasyon 261 passed; 2.2/2.4 (all-features) 263 passed (fark: libsodium-oracle differential-oracle testleri, +2).
- 5 `#[ignore]` sim testi normal koşularda 'ignored'; 2.5'te fiilen koştu → **5 passed**, ağır testin süresi `desen1_sim.rs` = **11.48s** (2.5 toplam 12.2s).
- doc-test: yalnız `adaptive_transport` 1 doc-test; diğer 12 crate 0 doc-test.

### 2.7 vs 2.8 — trybuild farkı (KRİTİK)

`kama-noise-wire` trybuild UI testi (sealed-type forge guard) yalnız DEFAULT feature'da koşar; `--all-features` (test-util açık) altında **cfg ile eleniyor** → 0 test.

**2.7 (default) — running 1 test:**
```
     Running tests/trybuild.rs (target/debug/deps/trybuild-4b3a883e3ac01f3d)
running 1 test
    Checking kama-noise-wire-tests v0.0.0 (/tmp/claude-1000/-home-kaan-kama-core/0d592b35-72b0-4405-865c-5d62b569ac9f/scratchpad/kama-core-clean/target/tests/trybuild/kama-noise-wire)
test tests/ui/forge_opened.rs ... ok
test tests/ui/forge_proof.rs ... ok
test sealed_type_cannot_be_forged ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
```

**2.8 (--all-features) — running 0 tests:**
```
   Compiling sodiumoxide v0.2.7
   Compiling kama-noise-wire v0.1.0 (/tmp/claude-1000/-home-kaan-kama-core/0d592b35-72b0-4405-865c-5d62b569ac9f/scratchpad/kama-core-clean/crates/kama-noise-wire)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.08s
     Running tests/trybuild.rs (target/debug/deps/trybuild-a3b9d5737207ff5b)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

### ADIM 2 — ham çıktı (son 40 satır, ANSI temiz)

#### 2.1  (`cargo test --workspace`) — exit=0, süre=74.4s, toplam 834 satır
```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests hub_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_fec_codec

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_integration

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_noise_wire

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_transport_schema

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests ra_ca

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

#### 2.2  (`cargo test --workspace --all-features`) — exit=0, süre=100.7s, toplam 638 satır
```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests hub_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_fec_codec

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_integration

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_noise_wire

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_transport_schema

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests ra_ca

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

#### 2.3  (`cargo test --workspace --no-default-features`) — exit=0, süre=5.5s, toplam 635 satır
```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests hub_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_fec_codec

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_integration

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_noise_wire

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_transport_schema

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests ra_ca

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

#### 2.4  (`cargo test --workspace --all-features --release`) — exit=0, süre=129.3s, toplam 783 satır
```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests hub_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_fec_codec

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_integration

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_noise_wire

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_transport_schema

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests ra_ca

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

#### 2.5  (`cargo test --workspace -- --ignored`) — exit=0, süre=12.2s, toplam 366 satır
```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests hub_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_fec_codec

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_integration

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_noise_wire

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_transport_schema

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests ra_ca

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

#### 2.6  (`cargo test --workspace --doc`) — exit=0, süre=1.0s, toplam 74 satır
```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests hub_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_fec_codec

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_integration

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_noise_wire

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kama_transport_schema

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests ra_ca

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```


---

## ADIM 3 — Kapsam (llvm-cov)

Komutlar (`--exclude bench`; stub crate'ler crypto/ccr-codec/client-core/discovery/hub-server/ra-ca/admin-server yürütülebilir kod içermediğinden tabloda yok):

| # | komut | exit | süre (s) |
|---|---|---|---|
| 3.1 | cargo llvm-cov --workspace --exclude bench --summary-only | 0 | 18.2 |
| 3.2 | cargo llvm-cov --workspace --exclude bench --show-missing-lines | 0 | 8.3 |
| 3.3 | cargo llvm-cov --workspace --exclude bench --json --output-path cov.json | 0 | 8.1 |
| 3.4 | cargo +nightly llvm-cov --workspace --exclude bench --branch --summary-only | 0 | 28.4 |
| 3.5 | python3 cov_analysis.py (cov.json) | 0 | 0.1 |

cov.json üretildi: 1329197 bayt, 0 satır (llvm-cov export).

### 3.1 — özet (region/function/line kapsamı), TOTAL
```
Filename                                    Regions    Missed Regions     Cover   Functions  Missed Functions  Executed       Lines      Missed Lines     Cover    Branches   Missed Branches     Cover
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
adaptive-transport/src/ack.rs                   331                13    96.07%          12                 0   100.00%         189                 3    98.41%           0                 0         -
adaptive-transport/src/arq.rs                    46                 0   100.00%           8                 0   100.00%          45                 0   100.00%           0                 0         -
adaptive-transport/src/budget.rs                 24                 0   100.00%           4                 0   100.00%          17                 0   100.00%           0                 0         -
adaptive-transport/src/bw.rs                    136                 9    93.38%          18                 2    88.89%         113                 7    93.81%           0                 0         -
adaptive-transport/src/clock.rs                   7                 0   100.00%           2                 0   100.00%           6                 0   100.00%           0                 0         -
adaptive-transport/src/emergency.rs              56                 0   100.00%          11                 0   100.00%          76                 0   100.00%           0                 0         -
adaptive-transport/src/fec.rs                    79                 6    92.41%           2                 0   100.00%          41                 0   100.00%           0                 0         -
adaptive-transport/src/fusion.rs                233                 5    97.85%          17                 1    94.12%         170                 5    97.06%           0                 0         -
adaptive-transport/src/loss.rs                  259                16    93.82%          31                 2    93.55%         195                13    93.33%           0                 0         -
adaptive-transport/src/pacer.rs                  45                 0   100.00%           9                 0   100.00%          40                 0   100.00%           0                 0         -
adaptive-transport/src/rate.rs                   88                 1    98.86%           8                 0   100.00%          63                 1    98.41%           0                 0         -
adaptive-transport/src/read.rs                   45                 2    95.56%           6                 0   100.00%          28                 0   100.00%           0                 0         -
adaptive-transport/src/receiver.rs              747                12    98.39%          42                 2    95.24%         464                 9    98.06%           0                 0         -
adaptive-transport/src/rtt.rs                   162                 6    96.30%          21                 0   100.00%         113                 4    96.46%           0                 0         -
adaptive-transport/src/sender.rs               2373               158    93.34%          88                15    82.95%        1384               114    91.76%           0                 0         -
adaptive-transport/src/seq.rs                    23                 0   100.00%           1                 0   100.00%          11                 0   100.00%           0                 0         -
adaptive-transport/src/sim.rs                    50                 0   100.00%           6                 0   100.00%          24                 0   100.00%           0                 0         -
kama-fec-codec/src/codec.rs                     381                41    89.24%          17                 2    88.24%         266                28    89.47%           0                 0         -
kama-fec-codec/src/profile.rs                    29                 1    96.55%           5                 0   100.00%          28                 1    96.43%           0                 0         -
kama-noise-wire/src/aad.rs                       17                 0   100.00%           1                 0   100.00%           9                 0   100.00%           0                 0         -
kama-noise-wire/src/control.rs                  477                65    86.37%          19                 1    94.74%         296                25    91.55%           0                 0         -
kama-noise-wire/src/demux.rs                     29                 1    96.55%           4                 0   100.00%          16                 1    93.75%           0                 0         -
kama-noise-wire/src/epoch_machine.rs             52                 1    98.08%           7                 0   100.00%          52                 1    98.08%           0                 0         -
kama-noise-wire/src/handle_assign.rs             69                 0   100.00%           5                 0   100.00%          41                 0   100.00%           0                 0         -
kama-noise-wire/src/hub_sim.rs                   71                 0   100.00%           7                 0   100.00%          55                 0   100.00%           0                 0         -
kama-noise-wire/src/init_flight.rs               77                 3    96.10%          13                 1    92.31%          77                 3    96.10%           0                 0         -
kama-noise-wire/src/inner.rs                    104                 8    92.31%           3                 0   100.00%          67                 7    89.55%           0                 0         -
kama-noise-wire/src/legacy.rs                   151                11    92.72%           9                 2    77.78%         129                12    90.70%           0                 0         -
kama-noise-wire/src/opened.rs                    31                 7    77.42%           8                 1    87.50%          35                 3    91.43%           0                 0         -
kama-noise-wire/src/outer.rs                     55                 0   100.00%           3                 0   100.00%          41                 0   100.00%           0                 0         -
kama-noise-wire/src/pn.rs                       117                 6    94.87%          11                 1    90.91%          82                 6    92.68%           0                 0         -
kama-noise-wire/src/protector.rs                224                20    91.07%          12                 2    83.33%         192                15    92.19%           0                 0         -
kama-noise-wire/src/restart.rs                   75                 1    98.67%          11                 0   100.00%          73                 0   100.00%           0                 0         -
kama-noise-wire/src/retry.rs                     48                 9    81.25%           7                 1    85.71%          42                 8    80.95%           0                 0         -
kama-noise-wire/src/session.rs                   83                15    81.93%          14                 4    71.43%          71                12    83.10%           0                 0         -
kama-transport-schema/src/control.rs             25                 0   100.00%           2                 0   100.00%          24                 0   100.00%           0                 0         -
kama-transport-schema/src/inner_kind.rs          19                 6    68.42%           3                 1    66.67%          18                 6    66.67%           0                 0         -
kama-transport-schema/src/param.rs               49                 1    97.96%           2                 0   100.00%          24                 0   100.00%           0                 0         -
kama-transport-schema/src/prefix.rs              83                 7    91.57%           3                 0   100.00%          60                13    78.33%           0                 0         -
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
TOTAL                                          6970               431    93.82%         452                38    91.59%        4677               297    93.65%           0                 0         -
```

### 3.4 — nightly BRANCH kapsamı (TOTAL + branch sütunu)

TOTAL branch: **660 branch, 124 missed, %81.21**. (region/function/line TOTAL 3.1 ile aynı.)

En düşük branch-cover dosyalar (branch% artan):
```
adaptive-transport/src/budget.rs                 24                 0   100.00%           4                 0   100.00%          17                 0   100.00%           0                 0         -
Running                                    branch=(target/llvm-cov-target/debug/build/adaptive-transport/2f77982da79ae4c7/out/adaptive_transport-2f77982da79ae4c7) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/admin-server/5f03413faea0a9ad/out/admin_server-5f03413faea0a9ad) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/ccr-codec/78d241d5946b8b29/out/ccr_codec-78d241d5946b8b29) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/client-core/a85e1592c4c848e5/out/client_core-a85e1592c4c848e5) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/crypto/19272571ee86eac0/out/crypto-19272571ee86eac0) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/discovery/c85f0771539bcd8e/out/discovery-c85f0771539bcd8e) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/hub-server/6bd0996ddd19086b/out/hub_server-6bd0996ddd19086b) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/kama-fec-codec/69b8346dc4ade0ef/out/kama_fec_codec-69b8346dc4ade0ef) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/kama-integration/85c2655448133f91/out/kama_integration-85c2655448133f91) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/kama-noise-wire/0e55db8cc611d125/out/kama_noise_wire-0e55db8cc611d125) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/kama-transport-schema/9b9a01413943a938/out/kama_transport_schema-9b9a01413943a938) (miss src/lib.rs/unittests)
Running                                    branch=(target/llvm-cov-target/debug/build/ra-ca/a5e014b3fc1e1ba5/out/ra_ca-a5e014b3fc1e1ba5) (miss src/lib.rs/unittests)
adaptive-transport/src/budget.rs           branch=- (miss 0/0)
adaptive-transport/src/clock.rs            branch=- (miss 0/0)
```

### 3.2 — show-missing-lines (kapsanmayan satır aralıkları, dosya bazında)
```
crates/adaptive-transport/src/ack.rs: 125, 175, 196
crates/adaptive-transport/src/bw.rs: 234, 235, 236, 241, 242, 243, 252
crates/adaptive-transport/src/fusion.rs: 51, 67, 306, 307, 308
crates/adaptive-transport/src/loss.rs: 181, 182, 183, 190, 191, 260, 261, 262, 280, 281, 282, 283, 311
crates/adaptive-transport/src/rate.rs: 129
crates/adaptive-transport/src/receiver.rs: 256, 364, 399, 469, 470, 471, 480, 481, 482
crates/adaptive-transport/src/rtt.rs: 40, 41, 42, 217
crates/adaptive-transport/src/sender.rs: 495, 607, 608, 676, 696, 697, 698, 757, 761, 762, 774, 775, 776, 777, 778, 779, 781, 796, 797, 834, 882, 965, 1007, 1010, 1043, 1060, 1083, 1179, 1202, 1203, 1204, 1205, 1206, 1207, 1208, 1209, 1210, 1211, 1212, 1213, 1214, 1215, 1217, 1249, 1250, 1251, 1252, 1256, 1257, 1258, 1259, 1260, 1261, 1262, 1263, 1285, 1325, 1326, 1327, 1328, 1329, 1330, 1331, 1332, 1353, 1354, 1355, 1356, 1358, 1418, 1450, 1451, 1452, 1456, 1457, 1458, 1459, 1460, 1461, 1462, 1463, 1464, 1465, 1466, 1467, 1468, 1484, 1485, 1486, 1504, 1506, 1507, 1509, 1512, 1513, 1514, 1518, 1519, 1520, 1524, 1525, 1526, 1553, 1554, 1555, 1564, 1565, 1566, 1710
crates/kama-fec-codec/src/codec.rs: 108, 109, 110, 137, 138, 139, 140, 143, 204, 205, 206, 207, 212, 213, 214, 215, 218, 224, 225, 226, 227, 340, 342, 343, 344, 345, 346, 392
crates/kama-fec-codec/src/profile.rs: 39
crates/kama-noise-wire/src/control.rs: 139, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 334, 390
crates/kama-noise-wire/src/demux.rs: 41
crates/kama-noise-wire/src/epoch_machine.rs: 124
crates/kama-noise-wire/src/init_flight.rs: 123, 124, 125
crates/kama-noise-wire/src/inner.rs: 65, 68, 74, 77, 84, 88, 103
crates/kama-noise-wire/src/legacy.rs: 118, 119, 120, 122, 123, 124, 133, 170, 171, 172, 173, 177
crates/kama-noise-wire/src/opened.rs: 50, 51, 52
crates/kama-noise-wire/src/pn.rs: 85, 86, 87, 126, 127, 129
crates/kama-noise-wire/src/protector.rs: 155, 156, 157, 158, 181, 226, 335, 343, 344, 346, 347, 348, 349
crates/kama-noise-wire/src/retry.rs: 36, 41, 42, 43, 91, 92, 93, 94
crates/kama-noise-wire/src/session.rs: 63, 64, 65, 67, 68, 69, 85, 86, 87, 130, 131, 132
crates/kama-transport-schema/src/inner_kind.rs: 42, 43, 44, 45, 46, 48
crates/kama-transport-schema/src/prefix.rs: 48, 49, 50, 51, 73, 74, 75, 76, 83, 87, 88, 89, 90
```

### 3.5 — cov.json region analizi + hata-variant construct-site çapraz referansı

> Yöntem notu (ölçüm, yorum değil): variant tespiti REGEX ile yapıldı; yalnız görevin belirttiği 3 kalıp tarandı: `Err(<Variant>`, `.ok_or(<Variant>`, `.ok_or_else(|| <Variant>`. `#[from]`/`?`-dönüşümü, `.map_err(|_| Variant)` ve doğrudan `return Variant` YAKALANMADI. 'region counts' = o satırda başlayan region-entry segment count'ları (cov.json segments: [line,col,count,has_count,is_region_entry,is_gap]). bench crate cov dışı → BenchError satırları 'dosya-cov-yok'.

#### 3.5a — src dosyalarında count==0 region-entry satırları (özet)

TOPLAM count==0 region-entry satırı (yalnız src): **324**. Dosya bazında dağılım:
```
crates/adaptive-transport/src/ack.rs: 13 zero-count region lines -> [125, 175, 196, 211, 224, 229, 230, 231, 232, 233, 236, 237, 243]
crates/adaptive-transport/src/bw.rs: 8 zero-count region lines -> [214, 234, 235, 236, 241, 242, 243, 252]
crates/adaptive-transport/src/fec.rs: 6 zero-count region lines -> [78, 82, 83, 84, 85, 86]
crates/adaptive-transport/src/fusion.rs: 5 zero-count region lines -> [51, 67, 306, 307, 308]
crates/adaptive-transport/src/loss.rs: 14 zero-count region lines -> [181, 182, 183, 189, 190, 260, 261, 262, 280, 281, 283, 311, 326, 364]
crates/adaptive-transport/src/rate.rs: 1 zero-count region lines -> [129]
crates/adaptive-transport/src/read.rs: 2 zero-count region lines -> [36, 45]
crates/adaptive-transport/src/receiver.rs: 10 zero-count region lines -> [256, 363, 392, 399, 469, 470, 471, 480, 481, 482]
crates/adaptive-transport/src/rtt.rs: 4 zero-count region lines -> [40, 41, 42, 217]
crates/adaptive-transport/src/sender.rs: 97 zero-count region lines -> [495, 564, 567, 607, 608, 616, 618, 626, 657, 676, 690, 696, 756, 760, 774, 775, 776, 777, 779, 781, 796, 797, 833, 882, 965, 1007, 1010, 1043, 1060, 1083, 1179, 1202, 1203, 1204, 1205, 1209, 1210, 1215, 1217, 1249, 1250, 1251, 1252, 1256, 1257, 1258, 1259, 1273, 1285, 1304, 1325, 1326, 1327, 1328, 1353, 1354, 1355, 1356, 1358, 1418, 1450, 1451, 1452, 1456, 1457, 1459, 1460, 1461, 1462, 1463, 1464, 1465, 1468, 1484, 1485, 1486, 1504, 1506, 1507, 1509, 1512, 1513, 1514, 1518, 1519, 1520, 1524, 1525, 1526, 1553, 1554, 1555, 1564, 1565, 1566, 1625, 1710]
crates/kama-fec-codec/src/codec.rs: 36 zero-count region lines -> [97, 108, 109, 110, 137, 143, 165, 167, 204, 205, 212, 218, 224, 266, 271, 274, 276, 299, 306, 316, 324, 340, 342, 343, 344, 345, 346, 362, 363, 371, 372, 383, 390, 392, 396, 397]
crates/kama-fec-codec/src/profile.rs: 1 zero-count region lines -> [39]
crates/kama-noise-wire/src/control.rs: 44 zero-count region lines -> [127, 131, 139, 193, 194, 195, 196, 198, 199, 200, 201, 202, 203, 204, 221, 230, 239, 248, 289, 290, 294, 295, 308, 309, 310, 311, 314, 315, 326, 327, 331, 332, 334, 345, 346, 354, 355, 356, 369, 370, 379, 382, 383, 390]
crates/kama-noise-wire/src/demux.rs: 1 zero-count region lines -> [41]
crates/kama-noise-wire/src/epoch_machine.rs: 1 zero-count region lines -> [124]
crates/kama-noise-wire/src/init_flight.rs: 3 zero-count region lines -> [123, 124, 125]
crates/kama-noise-wire/src/inner.rs: 8 zero-count region lines -> [65, 68, 71, 74, 77, 84, 88, 103]
crates/kama-noise-wire/src/legacy.rs: 10 zero-count region lines -> [118, 119, 120, 122, 123, 124, 133, 154, 170, 177]
crates/kama-noise-wire/src/opened.rs: 3 zero-count region lines -> [50, 51, 52]
crates/kama-noise-wire/src/pn.rs: 6 zero-count region lines -> [85, 86, 87, 126, 127, 129]
crates/kama-noise-wire/src/protector.rs: 18 zero-count region lines -> [155, 156, 157, 158, 181, 198, 224, 226, 235, 250, 293, 319, 335, 342, 343, 344, 346, 349]
crates/kama-noise-wire/src/restart.rs: 1 zero-count region lines -> [102]
crates/kama-noise-wire/src/retry.rs: 6 zero-count region lines -> [36, 41, 42, 43, 91, 92]
crates/kama-noise-wire/src/session.rs: 13 zero-count region lines -> [63, 64, 65, 67, 68, 69, 85, 86, 87, 130, 131, 132, 149]
crates/kama-transport-schema/src/inner_kind.rs: 6 zero-count region lines -> [42, 43, 44, 45, 46, 48]
crates/kama-transport-schema/src/param.rs: 1 zero-count region lines -> [53]
crates/kama-transport-schema/src/prefix.rs: 6 zero-count region lines -> [48, 49, 73, 83, 87, 88]
```

#### 3.5b — hata-variant construct site'ları (region count ile)

| variant | dosya:satir | region counts | calisti/calismadi |
|---|---|---|---|
| TransportError::UnknownFrameType | crates/adaptive-transport/src/ack.rs:227 | 1 | calisti |
| TransportError::TrailingBytes | crates/adaptive-transport/src/ack.rs:248 | 1 | calisti |
| TransportError::UnknownFrameType | crates/adaptive-transport/src/fec.rs:80 | 1 | calisti |
| TransportError::InvalidShardCount | crates/adaptive-transport/src/fec.rs:89 | 1 | calisti |
| TransportError::TrailingBytes | crates/adaptive-transport/src/fec.rs:96 | 1 | calisti |
| TransportError::Truncated | crates/adaptive-transport/src/read.rs:25 | 2 | calisti |
| BenchError::Other | crates/bench/src/event.rs:118 | n/a | dosya-cov-yok |
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:92 | 1 | calisti |
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:101 | 3,3 | calisti |
| FecError::SourceCount | crates/kama-fec-codec/src/codec.rs:122 | 1 | calisti |
| FecError::SymbolLen | crates/kama-fec-codec/src/codec.rs:130 | 1 | calisti |
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:137 | 0 | CALISMADI |
| FecError::DuplicateEsi | crates/kama-fec-codec/src/codec.rs:143 | 0 | CALISMADI |
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:151 | 410,410,410 | calisti |
| FecError::KnownZeroTooLarge | crates/kama-fec-codec/src/codec.rs:191 | 1 | calisti |
| FecError::SymbolLen | crates/kama-fec-codec/src/codec.rs:204 | 0 | CALISMADI |
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:212 | 0 | CALISMADI |
| FecError::DuplicateEsi | crates/kama-fec-codec/src/codec.rs:218 | 0 | CALISMADI |
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:224 | 0 | CALISMADI |
| FecError::SourceInZeroRegion | crates/kama-fec-codec/src/codec.rs:231 | 1 | calisti |
| FecError::DuplicateEsi | crates/kama-fec-codec/src/codec.rs:237 | 1 | calisti |
| FecError::Insufficient | crates/kama-fec-codec/src/codec.rs:246 | 3 | calisti |
| FecError::ProfileBound | crates/kama-fec-codec/src/profile.rs:30 | 1 | calisti |
| FecError::ProfileBound | crates/kama-fec-codec/src/profile.rs:33 | 1 | calisti |
| FecError::ProfileBound | crates/kama-fec-codec/src/profile.rs:36 | 1 | calisti |
| FecError::ProfileBound | crates/kama-fec-codec/src/profile.rs:39 | 0 | CALISMADI |
| WireError::ControlCountExceedsCap | crates/kama-noise-wire/src/control.rs:154 | 2,2 | calisti |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:221 | 28,28,0 | calisti |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:230 | 36,36,0 | calisti |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:239 | 9,9,0 | calisti |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:248 | 15,15,0 | calisti |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:259 | 8,8,1 | calisti |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:270 | 1 | calisti |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:297 | 1 | calisti |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:334 | 0 | CALISMADI |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:390 | 0 | CALISMADI |
| WireError::InnerParse | crates/kama-noise-wire/src/handle_assign.rs:51 | 1 | calisti |
| WireError::InnerParse | crates/kama-noise-wire/src/handle_assign.rs:54 | 1 | calisti |
| WireError::InnerParse | crates/kama-noise-wire/src/handle_assign.rs:91 | 1 | calisti |
| WireError::InnerParse | crates/kama-noise-wire/src/hub_sim.rs:83 | 1 | calisti |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:65 | 0 | CALISMADI |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:68 | 0 | CALISMADI |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:74 | 0 | CALISMADI |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:77 | 0 | CALISMADI |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:84 | 0 | CALISMADI |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:88 | 0 | CALISMADI |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:103 | 0 | CALISMADI |
| WireError::SealInput | crates/kama-noise-wire/src/legacy.rs:133 | 0 | CALISMADI |
| WireError::DatagramTooShort | crates/kama-noise-wire/src/legacy.rs:170 | 0 | CALISMADI |
| WireError::UnknownHandle | crates/kama-noise-wire/src/legacy.rs:177 | 0 | CALISMADI |
| WireError::DatagramTooShort | crates/kama-noise-wire/src/outer.rs:50 | 1 | calisti |
| WireError::UnknownWireFormat | crates/kama-noise-wire/src/outer.rs:58 | 2 | calisti |
| WireError::ReservedWireFlags | crates/kama-noise-wire/src/outer.rs:62 | 1 | calisti |
| PnReject::TooOld | crates/kama-noise-wire/src/pn.rs:113 | 27447 | calisti |
| PnReject::Duplicate | crates/kama-noise-wire/src/pn.rs:117 | 3135 | calisti |
| PnReject::FutureGap | crates/kama-noise-wire/src/pn.rs:121 | 318 | calisti |
| WireError::SealInput | crates/kama-noise-wire/src/protector.rs:181 | 0 | CALISMADI |
| WireError::EpochKeysMissing | crates/kama-noise-wire/src/protector.rs:224 | 113,113,0 | calisti |
| WireError::PnCapReached | crates/kama-noise-wire/src/protector.rs:226 | 0 | CALISMADI |
| WireError::SealInput | crates/kama-noise-wire/src/protector.rs:230 | 3 | calisti |
| WireError::InnerParse | crates/kama-noise-wire/src/protector.rs:285 | 1 | calisti |
| WireError::EpochNotAccepted | crates/kama-noise-wire/src/protector.rs:289 | 1 | calisti |
| WireError::EpochKeysMissing | crates/kama-noise-wire/src/protector.rs:293 | 81,81,0 | calisti |
| WireSizeError::EmptySymbol | crates/kama-noise-wire/src/protector.rs:335 | 0 | CALISMADI |
| WireError::SessionClosed | crates/kama-noise-wire/src/session.rs:99 | 1 | calisti |
| WireError::UnknownHandle | crates/kama-noise-wire/src/session.rs:153 | 8,8,2 | calisti |
| WireError::SessionClosed | crates/kama-noise-wire/src/session.rs:155 | 1 | calisti |
| SchemaError::UnknownInnerKind | crates/kama-transport-schema/src/inner_kind.rs:75 | 5,5 | calisti |
| SchemaError::ProfileBound | crates/kama-transport-schema/src/param.rs:39 | 1 | calisti |
| SchemaError::ProfileBound | crates/kama-transport-schema/src/param.rs:42 | 1 | calisti |
| SchemaError::ReservedFlagsSet | crates/kama-transport-schema/src/prefix.rs:37 | 1 | calisti |
| SchemaError::PayloadTooLong | crates/kama-transport-schema/src/prefix.rs:42 | 1 | calisti |
| SchemaError::SymbolLenMismatch | crates/kama-transport-schema/src/prefix.rs:48 | 0 | CALISMADI |
| SchemaError::BufferTooShort | crates/kama-transport-schema/src/prefix.rs:73 | 0 | CALISMADI |
| SchemaError::ReservedFlagsSet | crates/kama-transport-schema/src/prefix.rs:83 | 0 | CALISMADI |
| SchemaError::PayloadTooLong | crates/kama-transport-schema/src/prefix.rs:87 | 0 | CALISMADI |

#### 3.5c — construct site'ında TÜM region count == 0 olanlar (hepsi 0) — **toplam 26**

| variant | dosya:satir | region counts |
|---|---|---|
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:137 | 0 |
| FecError::DuplicateEsi | crates/kama-fec-codec/src/codec.rs:143 | 0 |
| FecError::SymbolLen | crates/kama-fec-codec/src/codec.rs:204 | 0 |
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:212 | 0 |
| FecError::DuplicateEsi | crates/kama-fec-codec/src/codec.rs:218 | 0 |
| FecError::EsiOutOfRange | crates/kama-fec-codec/src/codec.rs:224 | 0 |
| FecError::ProfileBound | crates/kama-fec-codec/src/profile.rs:39 | 0 |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:334 | 0 |
| WireError::ControlDecode | crates/kama-noise-wire/src/control.rs:390 | 0 |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:65 | 0 |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:68 | 0 |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:74 | 0 |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:77 | 0 |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:84 | 0 |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:88 | 0 |
| WireError::InnerParse | crates/kama-noise-wire/src/inner.rs:103 | 0 |
| WireError::SealInput | crates/kama-noise-wire/src/legacy.rs:133 | 0 |
| WireError::DatagramTooShort | crates/kama-noise-wire/src/legacy.rs:170 | 0 |
| WireError::UnknownHandle | crates/kama-noise-wire/src/legacy.rs:177 | 0 |
| WireError::SealInput | crates/kama-noise-wire/src/protector.rs:181 | 0 |
| WireError::PnCapReached | crates/kama-noise-wire/src/protector.rs:226 | 0 |
| WireSizeError::EmptySymbol | crates/kama-noise-wire/src/protector.rs:335 | 0 |
| SchemaError::SymbolLenMismatch | crates/kama-transport-schema/src/prefix.rs:48 | 0 |
| SchemaError::BufferTooShort | crates/kama-transport-schema/src/prefix.rs:73 | 0 |
| SchemaError::ReservedFlagsSet | crates/kama-transport-schema/src/prefix.rs:83 | 0 |
| SchemaError::PayloadTooLong | crates/kama-transport-schema/src/prefix.rs:87 | 0 |

### 3.6 — tanımlı ama (3 kalıpla) construct EDİLMEYEN / hiç referans EDİLMEYEN variantlar
```

-- TransportError (5 variants): ['Truncated', 'TrailingBytes', 'UnknownFrameType', 'InvalidShardCount', 'FeatureNotImplemented']
   NOT in Err()/ok_or construct sites: ['FeatureNotImplemented']
   NOT referenced as TransportError::<V> ANYWHERE in src: []

-- BenchError (5 variants): ['Io', 'Json', 'Quic', 'Tls', 'Other']
   NOT in Err()/ok_or construct sites: ['Io', 'Json', 'Quic', 'Tls']
   NOT referenced as BenchError::<V> ANYWHERE in src: ['Io', 'Json']

-- FecError (9 variants): ['ProfileBound', 'SourceCount', 'SymbolLen', 'DuplicateEsi', 'EsiOutOfRange', 'Insufficient', 'KnownZeroTooLarge', 'SourceInZeroRegion', 'RsEngine']
   NOT in Err()/ok_or construct sites: ['RsEngine']
   NOT referenced as FecError::<V> ANYWHERE in src: []

-- WireError (14 variants): ['DatagramTooShort', 'UnknownWireFormat', 'ReservedWireFlags', 'PnReject', 'EpochNotAccepted', 'AeadFail', 'InnerParse', 'SealInput', 'EpochKeysMissing', 'PnCapReached', 'ControlDecode', 'ControlCountExceedsCap', 'UnknownHandle', 'SessionClosed']
   NOT in Err()/ok_or construct sites: ['PnReject', 'AeadFail']
   NOT referenced as WireError::<V> ANYWHERE in src: ['PnReject']

-- PnReject (3 variants): ['TooOld', 'Duplicate', 'FutureGap']
   NOT in Err()/ok_or construct sites: []
   NOT referenced as PnReject::<V> ANYWHERE in src: []

-- WireSizeError (2 variants): ['EmptySymbol', 'ControlCountExceedsCap']
   NOT in Err()/ok_or construct sites: []
   NOT referenced as WireSizeError::<V> ANYWHERE in src: []

-- SchemaError (6 variants): ['PayloadTooLong', 'ReservedFlagsSet', 'SymbolLenMismatch', 'ProfileBound', 'UnknownInnerKind', 'BufferTooShort']
   NOT in Err()/ok_or construct sites: []
   NOT referenced as SchemaError::<V> ANYWHERE in src: []
```


---

## ADIM 4 — Sanitizer / miri

miri crate bazında ayrı koşuldu (görev talimatı: uzunsa -p bazında böl). nightly = 1.99.0-nightly, miri sysroot önceden kuruldu.

| # | komut | exit | süre (s) | sonuç |
|---|---|---|---|---|
| 4.1a | cargo +nightly miri test -p kama-noise-wire | **1** | 35.6 | 6 test ok, sonra ABORT (miri unsupported op) |
| 4.1b | cargo +nightly miri test -p kama-transport-schema | 0 | 2.7 | 9 passed, 0 failed |
| 4.1c | cargo +nightly miri test -p kama-fec-codec | **101 (SONLANDIRILDI)** | 1783.6 (kesim) | tümü PASS ama aşırı yavaş; bütçe içinde bitmedi → elle kill |
| 4.2 | cargo +nightly miri test -p crypto -p ccr-codec | 0 | 1.4 | 0 test (ikisi de stub); ok — **mevcut CI hedefi bu** |
| 4.3 | RUSTFLAGS="-Zsanitizer=address" cargo +nightly test -p kama-noise-wire --target x86_64-unknown-linux-gnu | 0 | 15.1 | 90 passed, 0 failed, **0 ASan hatası** |

### 4.1a — miri kama-noise-wire (exit=1)

Birim + bazı entegrasyon testleri miri altında geçti (6 passed), sonra `pn_proptest` test dosyasında ABORT:

```
running 7 tests
test future_gap_rejects_far_first ... error: unsupported operation: `getcwd` not available when isolation is enabled
  --> .../library/std/src/sys/paths/unix.rs:27:17
   = help: set MIRIFLAGS=-Zmiri-disable-isolation to disable isolation;
   = help: or set MIRIFLAGS=-Zmiri-isolation-error=warn ...
   = note: this is on thread `future_gap_reje`
error: aborting due to 1 previous error
error: test failed, to rerun pass `-p kama-noise-wire --test pn_proptest`
```
(Kök: proptest çalışırken `getcwd` çağırıyor; miri default-isolation bunu engelliyor → UB değil, miri-izolasyon kısıtı. `MIRIFLAGS=-Zmiri-disable-isolation` denenmedi — görev tam komutu istedi.)

### 4.1b — miri kama-transport-schema (exit=0)
9 passed, 0 failed.

### 4.1c — miri kama-fec-codec (SONLANDIRILDI, exit=101)

Testler geçiyor ama reed-solomon GF-aritmetiği miri-yorumu altında **pratik-dışı yavaş**. Ham ilerleme (kesimden önce):
```
Running unittests src/lib.rs           → 0 tests ok (0.03s)
Running tests/engine_reuse.rs          → 5 passed  (finished in 1913.42s)
Running tests/golden.rs                → 3 passed  (finished in 1027.23s)
Running tests/known_zero.rs (8 tests)  → 2 passed, sonra known_zero_k8_kactual1 sırasında KILL
```
Yani gözlemlenen kadarıyla 0 fail; tek bir test binary'si (engine_reuse, 5 test) **1913s**. Toplam koşum bütçe içinde bitmedi → elle sonlandırıldı (SIGTERM). `run.sh` kesim anındaki duvar-saatini (1783.6s) ve exit=101'i kaydetti.

### 4.2 — miri crypto + ccr-codec (exit=0)
Her iki crate stub (`lib.rs`, test yok) → `0 passed; 0 failed`. CLAUDE.md §7 mevcut miri CI hedefi bu iki crate.

### 4.3 — ASan (nightly, exit=0)
`kama-noise-wire` AddressSanitizer altında derlendi ve koştu: **90 passed, 0 failed**; çıktıda hiçbir `AddressSanitizer`/`ERROR:`/`SUMMARY:` satırı yok. (`-Zbuild-std` KULLANILMADI — görevdeki tam komut; std instrumente edilmedi, crate instrumente edildi.) trybuild UI + doc-test de bu koşuda ok.


---

## ADIM 5 — Fuzz fizibilitesi (geçici fuzz crate; 3 hedef × 120s)

`cargo +nightly fuzz init` `crates/kama-noise-wire` içinde koşuldu (init exit=0); üretilen manifest hatalı crate-adı (`crypto-fuzz`, yanlış path-dep) verdiği için `fuzz/Cargo.toml` elle doğru yazıldı (deps: kama-noise-wire path=.., adaptive-transport path=../../adaptive-transport, zeroize, libfuzzer-sys). 3 hedef derlendi (`cargo +nightly fuzz build`, **exit=0**, default sanitizer=address).

Hedef API uyarlaması (görev tarifi → gerçek imza): `KamaNoiseWireProtector::new` gerçek imzası `&Zeroizing<[u8;32]>` alır → `Zeroizing::new([0x11;32])`/`([0x22;32])`; `decode_control_body(kind=data[0], id=1u32, body=&data[1..])` (ControlId=u32); `AckFrame::decode(data)` (adaptive-transport::ack).

Her hedef `-rss_limit_mb=2048 -max_total_time=120` ile koştu. **ADIM 4 miri ile eşzamanlı koştular → exec/s ALT-SINIRDIR (CPU çekişmesi).**

| hedef | exit | süre | toplam run | exec/s (final) | cov | ft (edge) | crash/OOM/timeout |
|---|---|---|---|---|---|---|---|
| fuzz_open | 0 | 121.4s | 6,955,766 | 57,485 | 592 | 831 | YOK |
| fuzz_control_decode | 0 | 121.6s | 59,516,427 | 491,871 | 260 | 383 | YOK |
| fuzz_ack_decode | 0 | 121.6s | 27,730,309 | 229,176 | 136 | 250 | YOK |

Crash/OOM/timeout: **hiçbiri**. `fuzz/artifacts/` boş (crash girdisi yok). Üç hedef de `Done ... runs in 121 second(s)` ile temiz bitti.

Ham DONE satırları:
```
# fuzz_open
#6955766	DONE   cov: 592 ft: 831 corp: 23/16252b lim: 4096 exec/s: 57485 rss: 460Mb
Done 6955766 runs in 121 second(s)
# fuzz_control_decode
#59516427	DONE   cov: 260 ft: 383 corp: 106/3083b lim: 4096 exec/s: 491871 rss: 507Mb
Done 59516427 runs in 121 second(s)
# fuzz_ack_decode
#27730309	DONE   cov: 136 ft: 250 corp: 42/2215b lim: 4096 exec/s: 229176 rss: 478Mb
Done 27730309 runs in 121 second(s)
```


---

## ADIM 6 — Statik kapılar

Tümü **exit=0**.

| # | komut | exit | süre (s) | sonuç |
|---|---|---|---|---|
| 6.1 | cargo fmt --all -- --check | 0 | 0.3 | çıktı yok (fmt temiz) |
| 6.2 | cargo clippy --workspace --all-targets --all-features -- -D warnings | 0 | 11.0 | warning/error yok |
| 6.3 | cargo clippy --workspace --all-targets --no-default-features -- -D warnings | 0 | 0.7 | warning/error yok |
| 6.4a | cargo audit | 0 | 9.2 | 1 allowed-warning (unmaintained) |
| 6.4b | cargo deny check | 0 | 2.7 | advisories/bans/licenses/sources = ok (warning'ler var) |
| 6.5 | cargo test --workspace --locked | 0 | 4.1 | 261 passed, 0 failed (lock drift yok) |

### 6.1 fmt
`cargo fmt --all -- --check` → çıktı yok, exit=0.

### 6.2 / 6.3 clippy
Her iki clippy koşusu (`--all-features` ve `--no-default-features`), `-D warnings` altında **exit=0**; hiçbir clippy warning/error satırı yok (yalnız `Checking`/`Finished` satırları).

### 6.4a cargo audit (ham)
```
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 1177 security advisories
    Scanning Cargo.lock for vulnerabilities (206 crate dependencies)
Crate:     sodiumoxide
Version:   0.2.7
Warning:   unmaintained
Title:     sodiumoxide is deprecated
Date:      2021-10-22
ID:        RUSTSEC-2021-0137
URL:       https://rustsec.org/advisories/RUSTSEC-2021-0137

warning: 1 allowed warning found
```
exit=0 (allowed warning; vulnerability=0).

### 6.4b cargo deny check (özet)
Son satır: `advisories ok, bans ok, licenses ok, sources ok`. Uyarılar (fail değil):
```
warning[duplicate]: found 2 duplicate entries for crate 'chacha20'
warning[duplicate]: found 2 duplicate entries for crate 'cpufeatures'
warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
warning[duplicate]: found 2 duplicate entries for crate 'windows-sys'
warning[license-not-encountered]: "BSD-2-Clause" unmatched license allowance (deny.toml:14)
warning[license-not-encountered]: "MPL-2.0" unmatched license allowance (deny.toml:19)
```

### 6.5 cargo test --workspace --locked
exit=0; `261 passed; 0 failed` → `--locked` altında Cargo.lock ile tutarlı (drift yok).


---

## ADIM 7 — netem harness fizibilitesi (KOŞULMADI; yalnız rapor)

### 7.1 docker / docker compose

- `which docker` → `/mnt/c/Program Files/Docker/Docker/resources/bin/docker` (Windows Docker Desktop shim, PATH'te).
- `docker --version` → **rc=1**, çıktı: `The command 'docker' could not be found in this WSL 2 distro. We recommend to activate the WSL integration in Docker Desktop settings.`
- `docker info --format {{.ServerVersion}}` → aynı "could not be found" mesajı; **daemon erişilemez**.
- `docker compose version` → boş çıktı; compose plugin çalışmıyor.
- Sonuç (ölçüm): bu WSL2 distro'sunda çalışan bir docker daemon / compose YOK.

### 7.2 Container'da NET_ADMIN + tc/netem

- Host `tc`: `/usr/sbin/tc` MEVCUT (iproute2).
- Kernel netem modülü: `sch_netem` `lsmod`'da yüklü değil ama modül dosyası VAR:
  `/lib/modules/6.18.33.2-microsoft-standard-WSL2/kernel/net/sched/sch_netem.ko` (modinfo: "Network characteristics emulator qdisc").
- Mevcut shell yetenekleri: `/proc/self/status` `CapEff: 0000000000000000` (bu host shell'de NET_ADMIN yok; harness tc'yi container içinde `cap_add: [NET_ADMIN]` ile çalıştırır — `docker-compose.yml` her iki servise `cap_add: [NET_ADMIN]` verir).
- Kuru sonuç: tc + sch_netem.ko host'ta erişilebilir; ancak harness container-tabanlı (docker-compose) ve docker daemon yok → harness bu ortamda koşturulamaz.

### 7.3 Harness'in beklediği ama depoda BULUNMAYAN dosyalar

- `results/` dizini **YOK** (depoda). Depoda hiç `*.jsonl` yok (`find . -name '*.jsonl'` = 0).
- `compare.py` / `analyze.py` / `report.py` / `run-bench.sh` / `run-kama-matrix.sh` runtime'da `results/` üretir/okur; bunların HİÇBİRİ commit'li değil. Beklenen (glob) baseline dosyaları (7 profil):
  - quinn baseline: `results/{p1_loss50,p2_loss80,p3_loss95,p4_jitter,p5_revpath,p6_blackout,p7_bbr}_{cubic,bbr}_r*.jsonl`  → hepsi **0 dosya**
  - kama matris: `results/kama_{profil}_r*.jsonl` → hepsi **0 dosya**
  - özet: `results/{profil}_{cc}_summary.txt` → hepsi **0 dosya**
  - metadata/çıktı: `results/run-metadata.json`, `results/analysis.json`, `results/kama-vs-quinn.json` → **MISSING**
- Yani netem karşılaştırması için hiçbir baseline/HAM ölçüm deposunda yok; harness sıfırdan koşum gerektirir (docker + NET_ADMIN + tc).

### 7.4 compare.py / analyze.py okuduğu dosya yolları

`RESULTS = <repo>/results` (her ikisinde `os.path.dirname(__file__)/../results`).

`compare.py`:
- okur: `results/kama_<profile>_r*.jsonl` (kama), `results/<profile>_cubic_r*.jsonl` (quinn baseline v1)
- yazar: `results/kama-vs-quinn.json`
- PROFILES = `[p7_bbr, p4_jitter, p6_blackout, p1_loss50, p5_revpath, p2_loss80, p3_loss95]`; DEADLINES_S = `[10,30,120]`

`analyze.py`:
- okur: `results/<profile>_<cc>_r*.jsonl` (cc ∈ {cubic,bbr}), `results/run-metadata.json` (varsa)
- yazar: `results/analysis.json`

(ek: `report.py` okur `results/<profile>_<cc>_summary.txt` + `results/<profile>_<cc>_r*.jsonl` + `results/run-metadata.json`.)


---

## ADIM 8 — Zamanlama (ADIM 2 + ADIM 3 duvar-saati)

Not: tüm koşular tek makinede; ADIM 2 kurulum (cargo-audit/deny derlemesi) ile kısmen eşzamanlı koştu, ADIM 5 fuzz ADIM 4 miri ile eşzamanlı koştu → o pencerelerdeki süreler CPU-çekişmesi içerir (aşağıda işaretli). Süreler her komutun kendi (build+run) duvar-saatidir; ardışık cargo çağrıları target/ cache'ini paylaşır (ilk komut derler, sonrakiler yeniden-kullanır).

| adım | komut | exit | süre (s) |
|---|---|---|---|
| 2.1 | cargo test --workspace | 0 | 74.4 |
| 2.2 | cargo test --workspace --all-features | 0 | 100.7 |
| 2.3 | cargo test --workspace --no-default-features | 0 | 5.5 |
| 2.4 | cargo test --workspace --all-features --release | 0 | 129.3 |
| 2.5 | cargo test --workspace -- --ignored | 0 | 12.2 |
| 2.6 | cargo test --workspace --doc | 0 | 1.0 |
| 2.7 | cargo test -p kama-noise-wire --test trybuild | 0 | 10.6 |
| 2.8 | cargo test -p kama-noise-wire --all-features --test trybuild | 0 | 2.2 |
| 3.1 | llvm-cov --summary-only | 0 | 18.2 |
| 3.2 | llvm-cov --show-missing-lines | 0 | 8.3 |
| 3.3 | llvm-cov --json | 0 | 8.1 |
| 3.4 | llvm-cov +nightly --branch --summary-only | 0 | 28.4 |
| 3.5 | cov_analysis.py | 0 | 0.1 |

## ORTAM

| alan | değer |
|---|---|
| klonlanan commit | `bb61c2da2898b975ca52f836381c55aa36e886b9` (main) |
| rustc (stable, pin) | rustc 1.96.0 (ac68faa20 2026-05-25) |
| cargo (stable) | cargo 1.96.0 (30a34c682 2026-05-25) |
| rustc (nightly) | rustc 1.99.0-nightly (8ab9fdff5 2026-07-30) |
| miri | 0.1.0 (8ab9fdff5a 2026-07-30) |
| cargo-llvm-cov | 0.8.7 |
| cargo-nextest | 0.9.140 |
| cargo-fuzz | 0.13.2 |
| cargo-audit | 0.22.2 |
| cargo-deny | 0.20.2 |
| işletim sistemi | Ubuntu 22.04.3 LTS (WSL2) |
| çekirdek | Linux 6.18.33.2-microsoft-standard-WSL2 |
| CPU | 11th Gen Intel Core i7-11800H @ 2.30GHz |
| çekirdek (nproc) | 6 |
| RAM | 15 GiB (+ 8 GiB swap) |
| sistem libsodium | libsodium.so.23 (1.0.18) runtime var; libsodium-dev YOK → sodiumoxide vendored derleme |
| docker | çalışan daemon YOK (WSL2 integration kapalı) |


---

