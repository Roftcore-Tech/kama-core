# G-IMPL-S3 — Okuma paketi (Crypto IMPLEMENTATION-GO)

| Alan | Değer |
|---|---|
| Kapı | **G-IMPL-S3 — Crypto IMPLEMENTATION-GO.** İnsan kapısıdır: yalnız Abkarada'nın plan §0 "İnsan kapısı" satırında tanımlı birebir mesajıyla geçilir. |
| Durum | **BEKLİYOR.** Bu belge kapıyı geçirmez; kapı geçilmedi. |
| Kart | K4G-04 — `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` v1.1 (Ek 2026-10-11 (3)); kararlar D16, D17 |
| Hazırlayan | Claude Code, şerit L-GATE, 2026-10-11. Kapıyı geçirme yetkisi yoktur (K4G-04 SINIR). |
| Taban | `origin/main` `0156e941`. Bağımlı kartlar (merged): K4G-01 #114 `5dbd0b80`, K4G-02 #113 `2f705d32`, K4G-01b #116 `b58717b7`. Tabanda ayrıca merged: K4G-00 #111 `b57dd096`, K4G-03 #112 `3ca59b1d`, K4G-12 #115 `0156e941`. |
| Satır atıfları | `b58717b7`'ye (K4G-01b merge'ü) göredir. Atıf yapılan dosyalar `0156e941`'de değişmedi (`git diff --stat b58717b7 0156e941`). |
| Kapının tanımı | BL `docs/sprint_plan/SPRINT-BACKLOG-FREEZE.md:330-331`: "Gerçek kod aynı vektörleri ve aynı conformance-core'u geçiyor; sözleşme dışında davranış yok." · BL:514: "**IMPLEMENTATION-GO** = gerçek kod aynı vektörleri tüketiyor + aynı conformance-core'u geçiyor + sözleşme dışı davranış icat etmiyor." |
| Kritik yol | Plan §3: K4G-01 → K4G-04 → insan kapısı G-IMPL-S3 → K4G-05 (INT-02). |

## 1. Kaynak paket — PR #104 gövdesi (aynen)

Kaynak: [PR #104](https://github.com/Roftcore-Tech/kama-core/pull/104) "L-A5c PR-2: A5.4 epoch makinesi ürün yolunda + A5.5 gerçek SessionControl", merge `5272da20` (2026-10-07T22:32:26Z). Metin `gh pr view 104 --json body --jq .body` çıktısıdır. Hiçbir satırı değiştirilmeden, her satırın başına `> ` (boş satıra `>`) eklenerek alıntılanmıştır. Gövde 73 satırdır; `--jq` çıktısı jq'nun eklediği son satır sonuyla 74 satır sayılır.

Alıntıdaki sayılar, SHA'lar ve "✔" işaretleri #104'ün o anki (`df2c3f32`) beyanıdır. Sonradan değişenler §1.1'de ayrıca yazılıdır; alıntıya dokunulmamıştır.

> **Şerit:** L-A5c PR-2 — A5.4 epoch makinesi + A5.5 oturum kontrolü (iş emri 2026-10-06 §3). Taban main `b6790269` (PR-1 #103 sonrası). Bu PR'la L-A5c tamamlanıyor; ardından **İNSAN KAPISI G-IMPL-S3**.
>
> **Dosyalar:**
> - `src/session_control.rs` (yeni, 1006)
> - `src/epoch_machine.rs` (yeniden: N_g sabit, T_g zamanı)
> - `src/protector.rs`: `control` alanı; tutamaç basma / belirli tutamaçla kurulum; legacy ≤2 anahtar denemesi; açma yolundan düzleme bildirim; `new()` yalnız test, `Default` yok
> - `src/lib.rs` (+1)
> - `tests/a5c_session_control.rs` (yeni)
> - `tests/noise_conformance_core/{target,main,epoch,s3_support,s3_retry}.rs`
> - `tests/{sprint4c_epoch,epoch0_discard}.rs` (O3)
> - `scripts/ci/noise-seam-coverage.json` (O3)
> - `scripts/ci/noise-conformance.sh`
> - `.github/workflows/ci.yml` (O3, real adımı)
> - `docs/sprint_plan/{G-S3-KAPI-RAPORU,SPRINT-BACKLOG-FREEZE}.md`
>
> ## Kabul ölçütü ↔ kanıt
> | Ölçüt (iş emri §3 L-A5c) | Kanıt |
> |---|---|
> | N_g tam 64 (max() yok) | `GRACE_N_G`; kurucu bütçesi kaldırıldı; hücre 5b (iki zarf) |
> | monotonic zaman girdisi + T_g = 30 s, hangisi önce | `on_time(now)` (K2 `MonotonicClock`); hücre 6, 7 |
> | legacy zarfta ≤2 anahtar denemesi, commit tek pencereye | `protector.rs` legacy açma yolu (epoch-1 önce); `last_key_trials` legacy (1, 2), extended (1, 1); hücre 8, 9 |
> | onaylayan ilk epoch-1 paketi N_g'ye #1 | `on_valid_epoch1_received`; hücre 1, 4, 5 |
> | grace kapanınca epoch-0 DISCARD, yeniden açılmaz | hücre 5 (paket + zaman sonrası Discarded) |
> | DhEpochMachine ürün yoluna bağlanır (tutkal kalkar) | `session_control.rs` + `protector.rs` `open`; epoch matrisi gerçek ürün yolunda (`real_epoch_legacy` / `real_epoch_extended`) |
> | Initiate: çıktı önbellekli, byte-özdeş retry; ikinci Initiate farklı ephemeral | H-RETRY Alice (real ×2) |
> | AcceptInit: aynı init_counter → mevcut oturum + önbellekli SSR1 | H-RETRY Bob (real ×2); byte-özdeş olmayan "retry" → `Failed` |
> | yeni init_counter + mevcut oturum → `ProvisionalAndReply`; onay → `Confirmed(new)` | `a5c_session_control::renew_cutover_and_provisional_supersession…` (iki zarf) |
> | glare: dest_hash küçük olan kazanır; kaybeden pending'ini zeroize eder | `glare_yields_a_single_session` |
> | bayat INIT onaylanmaz, PROVISIONAL_TTL ile düşer | `stale_provisional_is_dropped_silently_after_ttl` |
> | AcceptResponse: adaylar en-yeni-önce, ≤ cap; yanlış aday = bozuk paket | H-DEMUX 7/7 (real ×2) |
> | Close → Closed | H-HANDLE ×2 (real ×2) |
> | Renew (SESSION_MAX_AGE ya da cap → RenewRequired; cutover Ready(h') → Closed(h, Superseded)) | renew/supersession testi; `session_max_age_signals_renew_exactly_once` |
> | HALF_OPEN / PROVISIONAL TTL | `pending_init_expires_with_key_hygiene`; bayat provisional testi |
> | KDF / SSI1 / SSR1 / deprecated / ed25519-neg vektörleri tüketilir; C4 pending kapanır | #97 + #103; C4 §7.2 |
> | noise_conformance_core (H-DEMUX 7, epoch matrisi 9, DECLARED_RED BOŞ) + S3 yedi satırı --target real YEŞİL | `noise-conformance.sh --target real` (aşağıda); `DECLARED_RED = []` |
> | cargo deny | hosted `security` |
>
> ## YORUM (uygulamadan önce beyan)
> 1. **Ayrı modül:** A5.5 `session_control.rs`'te. Eski Sprint-4 `SessionManager` (`session.rs`) kama-integration'ın `receiver_host.rs`'inde kullanılıyor; yerinde bırakıldı.
> 2. **Ürün kurulumu:** `NoiseWireProtection::with_control(ControlConfig{identity, directory, clock, entropy, envelope})`. Eş dizini ve saat ZORUNLU (G1/K2). `new()` artık yalnız test-util; dizinsiz ürün yolu açmamak için `Default` bilerek yok.
> 3. **Ready zamanı:** Bob `AcceptInit` anında, Alice `Established`'da. Bekleyen oturum hub tutamacı gelince epoch-0 için kurulur (0-RTT, ccr §0.7-A "epoch-0-usable"). Tutamaç bağlanmadan oturum mühürlemez.
> 4. **early_unit:** boş olmayan `early_unit` → `Failed` (sahip kararı S1: Transport saklar). Boş olmayan `early_data` / `response_data` taşıyan INIT/RESPONSE de reddedilir; S3'te teslim yolu yok, sessiz kayıp yerine ret.
> 5. **Glare kaybedeni:** bekleyen INIT'i olaysız silinir. Transport niyetini eşle eşler; o eşle doğan oturum `Ready` olarak gelir.
> 6. **Bekleyen INIT zaman aşımı:** `HALF_OPEN_TTL` (7 gün) dolunca `Closed(h, KeyHygiene)`. S1'de `ConnectAbandoned{KeyHygiene}`'e karşılık gelir.
> 7. **Alım grace'i iki rolde:** Alice'in epoch-0 alımı da, Bob epoch-1'e geçtikten sonra gelen ilk geçerli epoch-1 paketiyle grace'e girer (aynı N_g/T_g). Aksi hâlde Alice'in epoch-0 alım anahtarı oturum boyunca canlı kalırdı.
> 8. **Drain:** yerini yenisine bırakan oturum yalnız alım yapar. N_g geçerli paket ya da T_g dolunca `Closed(h, Superseded)`.
> 9. **`init_counter`:** eş başına, bellekte, 0'dan. Süreç yeniden başlarsa sayaç sıfırlanır; bayat/yeniden sayaçlı INIT'ler K10 provisional yolundan geçer ve onaylanmazsa TTL ile düşer.
> 10. **`machine` hedefi kaldırıldı** (koşucu + ci.yml). Aynı hücreler `real_epoch_*` altında. ci.yml'de `--target real` artık **0 ZORUNLU** (O3).
>
> ## G-IMPL-S3 okuma paketi (dosya · satır · tükettiği vektör · örten satır)
> | Dosya | Satır | Vektör | Conformance / test |
> |---|---|---|---|
> | `kdf.rs` | 749 | `ccr-kdf-v1` 12/12 | birim testleri (#97) |
> | `protector.rs` | 1201 | `ccr-ssd1-v1` (real_ssd1) | H-CTX, H-PN-OWN, H-FEC-ORDER, H-NO-MUTATE, H-REPLAY-POISON (real paket ×2); epoch 8–9 |
> | `replay.rs` | 91 | `ccr-ssd1-v1` | H-REPLAY-POISON, H-NO-MUTATE; `replay_rejects_u64_max` |
> | `envelope.rs` | 388 | `ccr-ssd1-v1` | S10 diferansiyeli (`a5b_protection`) |
> | `handshake.rs` | 694 | `ccr-ssi1-v1` 16/16, `ccr-ssr1-v1` 8/8 | `a5c_handshake_vectors` |
> | `session_control.rs` | 1006 | — | H-RETRY, H-HANDLE, H-NO-MUTATE (kontrol), H-DEMUX (real kontrol ×2); `a5c_session_control` 8 |
> | `epoch_machine.rs` | 172 | — | epoch matrisi `real_epoch_*` 10 + 2 (×2); `sprint4c_epoch`, `epoch0_discard` |
> | `ed25519_strict.rs` | 158 | `ccr-ed25519-neg-v1` 10/10 | `a5c_handshake_vectors` |
> | `deprecated.rs` | 63 | `ccr-deprecated-neg-v1` 7/7 | `a5c_handshake_vectors` |
> | `peer_dir.rs` / `clock.rs` / `entropy.rs` | 184 / 102 / 126 | — | birim testleri |
>
> ## Kapı sonuçları (yerel; ci.yml satırlarıyla)
> Baş: `df2c3f32`.
> - `cargo fmt --all -- --check` ✔ · `cargo clippy --all-targets --all-features -- -D warnings` ✔ · `--no-default-features` ✔
> - `cargo test -p kama-noise-wire --all-features` ✔ (380 geçti, 0 kaldı) · `-p client-core --all-features` ✔ (72/0) · `-p kama-integration --all-features --test pipeline` ✔
> - `noise-conformance.sh`: mock ✔ (18) · **real ✔ (66 test, beyanlı kırmızı hücre 0)** · vectors ✔ (3) · identity ✔ (1)
> - `seam-conformance.sh --target mock` ✔ · `seam-api-drift.py` ✔ · `--seam s3` ✔ · `layer_check.py` ✔ · `no_inline_expected.py --min-files 1` ✔ · `word_coverage.py` ✔ (C4 + BL)
> - `cargo deny`: yerelde kurulu değil → hosted `security`
> - Hosted CI (taban main b6790269): 12/12 ✔ — static · test · security · miri · asan · msan · aead-differential · conformance · **seam conformance (S3 real artık 0 zorunlu)** · vectors · silme kapısı · ci verdict.
>
> 🤖 Generated with [Claude Code](https://claude.com/claude-code)

### 1.1 Alıntıdan sonra değişenler

Alıntı düzeltilmedi; aşağıdaki notlar ondan ayrıdır.

- **YORUM 9 (`init_counter`) çakışan sayaç için yanlıştı; K4G-02 düzeltti** ([PR #113](https://github.com/Roftcore-Tech/kama-core/pull/113)). #113 gövdesindeki düzeltme, aynen:

  > 9. ~~**`init_counter`:** eş başına, bellekte, 0'dan. Süreç yeniden başlarsa sayaç sıfırlanır; bayat/yeniden sayaçlı INIT'ler K10 provisional yolundan geçer ve onaylanmazsa TTL ile düşer.~~
  >
  > **[K4G-02 düzeltmesi]** Çakışan sayaç için yanlıştı. Yanıtlayanın önbelleğinde aynı `(eş, init_counter)` varken gelen farklı baytlı INIT provisional yola girmez: `AcceptInit`'in H-RETRY sınaması glare'den ve AEAD'den önce `Failed` döner (test 4(b)). Provisional yoldan yalnız önbellekte olmayan sayaç geçer. K4G-02 ile `init_counter` cihaz başına, kalıcı (`CounterStore::reserve_next`) ve monotondur; yeniden başlayan başlatıcı `n`'den büyük değerle gelir.

- **Paket tablosundaki satır sayıları** `5272da20`'e aittir. `b58717b7`'de yalnız `session_control.rs` değişti (1006 → 1074, K4G-02 #113) ve `counter_store.rs` eklendi (136, K4G-02 #113). Tablodaki diğer dosyalar (`kdf.rs`, `protector.rs`, `replay.rs`, `envelope.rs`, `handshake.rs`, `epoch_machine.rs`, `ed25519_strict.rs`, `deprecated.rs`, `peer_dir.rs`, `clock.rs`, `entropy.rs`) değişmedi.
- **`kama-noise-wire/src`'de #104 sonrası öteki değişiklikler** (`git diff --stat 5272da20 b58717b7`): `control.rs`, `control_id_gate.rs`, `inner.rs`, `retry.rs` (L-D #106: `Control::Ack/Sack` silimi); `seam_mock.rs` (L-T1 #109 test fikstürleri; K4G-02 #113 fikstür çağrı yeri); `lib.rs` (K4G-02 #113). `protector.rs` K4G-01 ve K4G-01b'de değişmedi (#114 ve #116 "Dokunulan dosyalar").
- **Kapı sonuçları:** alıntıdakiler #104 başı `df2c3f32` içindir (ör. `kama-noise-wire` 380/0). Güncel kapı sonuçları §2'dedir.
- **Hosted CI:** alıntıdaki "12/12 ✔" #104 zamanına aittir. 2026-10-11 itibarıyla hosted GitHub Actions faturalandırma kilidindedir. #114, #113 ve #116 plan §4.2 **CI-KİLİT İSTİSNASI** ile, yani orkestratörün yerel tam kapısıyla merge edildi.

## 2. Bağımlı kartlar — PR'lar ve kapı sonuçları

Üç PR da plan §4.2 CI-KİLİT İSTİSNASI ile merge edildi. Kapı, orkestratörün yerel tam kapısıdır (`ci.yml` PR adımları birebir, `k4g-gate.sh`). Kilit kalkınca `core-main` main'de elle tetiklenecek; kırmızıysa ACİL DUR ve revert. Kaynak her PR gövdesinin "Orkestratör kapısı — CI-KİLİT İSTİSNASI ile merge" bölümüdür (`gh pr view <n>`); sayılar oradan kopyalandı.

### 2.1 K4G-01 — Bağlam-önce dağıtım (D1 v1.1; ACİL DUR kartı)

| Alan | Değer |
|---|---|
| PR | [#114](https://github.com/Roftcore-Tech/kama-core/pull/114), merge `5dbd0b80` (2026-10-11T17:47:20Z) |
| Kapı HEAD | `87c8affe` |
| Yerel tam kapı | 21 adımın 21'i `exit=0`: layer_check, no_inline, word_coverage, fmt, clippy (all-features / no-default-features), `ctx_first_dispatch`, `--lib ctx_first_tests`, `client-core`, trybuild listesi, test_fast, conformance öz-test + koşu, seam öz-test, seam mock / real, noise mock / identity / vectors / real, `cargo deny` |
| Testler | `ctx_first_dispatch` 10/10 · `--lib ctx_first_tests` 4/4 · `test_fast` 1266/0 (taban `6be4ff44`: 1252/0 → +14 yeni test) |
| Entegrasyon | `cargo test -p kama-integration --all-features`: 372/0, HEAD `26853832`. O noktadan `87c8affe`'ye yalnız `tests/ctx_first_dispatch.rs` değişti; #114'e göre üretim kodu bayt-özdeş. |
| Mutasyon | 6/6 öldü: M1 istemci Hub→Session · M2 istemci her Session · M3 sunucu Hub→Session · M4 yuva bağı yok · M5 beklenen küme yok · M6 NotReady→Ready. Mutasyonsuz kontrol yeşil. |
| Adverseryal inceleme | 3 tur (15 + 14 ajan, 0 hata) |
| Sayaçlar | `CtxDispatchStats { ctx_mismatch_drops, hub_frames_dropped, not_ready_drops }`; `not_ready_drops` güvenlik sayacı değildir (#114) |

### 2.2 K4G-02 — `init_counter` kalıcılığı (D2)

| Alan | Değer |
|---|---|
| PR | [#113](https://github.com/Roftcore-Tech/kama-core/pull/113), merge `2f705d32` (2026-10-11T18:30:36Z) |
| Kapı HEAD | `68396e31` (taban `5dbd0b80`) |
| Yerel tam kapı | 22 adımın 22'si `exit=0` (`cargo test -p kama-integration --all-features` dahil) |
| Testler | `k4g02_counter_store` 6/6 (4(a), 4(b), 4(c), madde-2 ×2, ayırma öncesi ret) · `--lib counter_store` 3/3 · `client-core` `ctx_first_dispatch` 10/10 · `test_fast` 1275/0 (main 1266 + 9 yeni) · `cargo deny` ok |
| Entegrasyon | 372/0 (37 ikili) |
| Mutasyon | PR gövdesinde mutasyon koşusu kaydı yok |
| Adverseryal inceleme | 5 ajan, 0 hata; blocker/major yok; 3 nit düzeltildi (`68396e31`) |
| Trait | `pub trait CounterStore: Send + Sync { fn reserve_next(&self) -> Result<u64, CounterStoreError>; }` |

### 2.3 K4G-01b — Eş yuvası ve `Reorder` oturuma anahtarlanır (Ek (3))

| Alan | Değer |
|---|---|
| PR | [#116](https://github.com/Roftcore-Tech/kama-core/pull/116), merge `b58717b7` (2026-10-11T19:42:51Z) |
| Kapı HEAD | `472c6af0` (taban `2f705d32`) |
| Yerel tam kapı | 23 adımın 22'si `exit=0`. **Adım 10 (`cargo test -p client-core --all-features`) `exit=101`.** Açık kayıt aşağıda. |
| Testler | `session_keyed_slots` 5/5 (a–e) · `ctx_first_dispatch` 10/10 · `--lib ctx_first_tests` 5/5 · `test_fast` 1281/0 · `cargo deny` ok |
| Entegrasyon | `kama-integration` 372/0 |
| Mutasyon | 6/6 öldü: M1 tek ortak Reorder · M2 adres-yalnız yuva araması · M3 kapanış emekliliği yok · M3a yalnız bakım-tick · M4 `poll_event` Reorder silinmez · M5 dolu tabloda aktif yuva çıkarma. Mutasyonsuz kontrol yeşil; (e) 5/5 tekrar yeşil. |
| Adverseryal inceleme | 5 ajan, 0 hata; 1 minor (bakım-tick yolu ölçülmüyordu → test (e)) ve 2 nit düzeltildi |
| Adım 10 kırmızısı | Önceden var olan `runtime_e2e::arq_identity_is_the_pn_not_the_interim_seq_field` (#106, L-D) yalıtılmış koşuda bir kez düştü ("kalıcı silinen birim teslim edilemez"). Aynı test aynı koşunun `test_fast`'ında yeşil. Tekrar ölçüm: dalda 10/10, main `2f705d32`'de 10/10; adım 10 iki kez yeniden koşuldu: 98/0, 98/0. #116'ya göre K4G-01b'nin gerilemesi değildir. Olası mekanizma (ürün profilinde FEC parity'si "silinen" birimi kurtarır) **doğrulanmadı**. #116 takip önerisini (testte FEC kapatılsın ya da FEC çerçeveleri de silinsin) ayrı karta ve Abkarada kararına bırakır. |
| Yuva anahtarı | `(SocketAddr, SessionHandle)`; `Reorder` `SessionHandle` ile anahtarlı, oturum `Closed` olunca silinir (#116 "Yuva anahtarı ve emeklilik kuralı") |

## 3. kani koşusu (D17)

- **Karar.** Plan D17: G-IMPL-S3'ten önce elle bir kani koşusu yapılır ve sonucu bu pakete eklenir. Kırmızıysa **DUR** (plan §4.4); paket yine hazırlanır ve kırmızı beyan edilir.
- **Önceki durum.** Kani adımı #89'dan (`20430a0d`, 2026-10-05) bu PR'a dek derlenmiyor: `crates/kama-noise-wire/Cargo.toml:64-67`'deki `noise_conformance_core` test hedefi `required-features = ["test-util"]` taşır. Son zamanlanmış koşu [38129681429](https://github.com/Roftcore-Tech/kama-core/actions/runs/38129681429) (core-weekly, `schedule`, 2026-10-11T09:48:25Z, main `6be4ff44`): `kani` işi `cargo kani -p kama-noise-wire --tests` adımında şu hatayla düştü ve **hiçbir ispat koşmadı**:

  ```
  error: target `noise_conformance_core` in package `kama-noise-wire` requires the features: `test-util`
  Consider enabling them by passing, e.g., `--features="test-util"`
  ```

  Aynı koşuda `fuzz-deep` yeşil, `branch-coverage` ratchet adımı kırmızı ("branch kapsamı 81.27 % < taban 82.0 %"; plan §4.2 Ek (3): taban değişmez, K4G-16'da yeniden ölçülür).
- **Bu PR'daki değişiklik.** DUR-K4G-04 CEVAP (Ek (3)), Abkarada onaylı CI kararı: `.github/workflows/weekly.yml:50` → `cargo kani -p kama-noise-wire --tests --features test-util`. Başka workflow değişikliği yok.
- **Elle koşu yolu.** Hosted GitHub Actions faturalandırma kilidinde olduğu için `workflow_dispatch` (`only=kani`) koşulamadı. Plan K4G-04 NE 1 (Ek (3)) gereği kani orkestratör tarafından yerelde koşuldu.
- **Sonuç: YEŞİL. 9 ispatın 9'u doğrulandı, 0 başarısız.** Bu, `--features test-util` ile yapılan ilk tam ispat koşusudur; 2026-10-11'deki zamanlanmış koşu 38129681429 derleme hatasıyla düşmüş, ispat koşmamıştı. Koşu kırmızı olmadığı için plan §4.4'ün "kani koşusu kırmızı" DUR tetikleyicisi oluşmadı.

| Alan | Değer |
|---|---|
| Komut | `cargo kani -p kama-noise-wire --tests --features test-util` (`weekly.yml:50`'nin bu PR'daki hâli) |
| Koşu yeri | Orkestratör, yerel. Hosted CI faturalandırma kilidindedir. |
| Ağaç | main `0156e941` (K4G-12 #115 sonrası; K4G-01 #114, K4G-02 #113, K4G-01b #116 dahil). Bu PR yalnız `weekly.yml` ve `docs/gates/` dosyalarını değiştirir; ispat edilen kod bu daldakiyle aynıdır. |
| Ortam | Kani Rust Verifier 0.68.0 (cargo eklentisi); toolchain `nightly-2026-08-21-x86_64-unknown-linux-gnu` |
| Sonuç | `Complete - 9 successfully verified harnesses, 0 failures, 9 total` (doğrulama ~47 s) |
| Harness'ler (hepsi `VERIFICATION:- SUCCESSFUL`) | `commit_never_panics_after_precheck`, `precheck_never_panics`, `near_max_expected_never_panics`, `bounded_expected_never_panics`, `reconstruct_does_not_panic_on_contract_violation`, `reconstruct_is_deterministic`, `reconstruct_stays_within_one_span`, `reconstruct_preserves_low_bits`, `reconstruct_never_panics` |
| Kaynakla tutarlılık | Dokuz ad, `crates/kama-noise-wire/tests/kani_pn_proofs.rs`'teki dokuz `#[kani::proof]` fonksiyonunun adlarıyla birebir aynıdır (K4G-04 şeridi, `grep`). `weekly.yml:11`: "kani 9/9". |
| Kırmızıysa neden | Yok (yeşil) |
| Hosted koşu | Kilit kalkınca `gh workflow run weekly.yml -f only=kani --ref main` ile alınacak; koşu kimliği sonradan eklenecek. |

## 4. Bilinen açıklar

| # | Açık | Kanıt | Kapanış / durum |
|---|---|---|---|
| 1 | **KS-DOS-1.** Yanıtlayan tarafta eş başına provisional oturum tavanı yok. `HALF_OPEN_CAP` (64) yalnız eş başına yanıt önbelleğini kırpar (`crates/kama-noise-wire/src/session_control.rs:78-80`, `:806-817`). İstismar için dizindeki bir eşin anahtarı gerekir. | Plan §1.2 A3, D3; PoC-KAPSAM-001 §5 | Bu sprintte yapılmaz (D3); KATMAN 6'ya K4G-16 ile yazılır. `protector.rs:651-653`'teki "Tablo bu kapılarla sınırlıdır" cümlesi KS-DOS-1 yolu için yanlıştır; K4G-16'da düzeltilir. |
| 2 | **HubChannel no-op.** Gerçek hop anahtarı yok. Tutamacı kayıtlı olmayan ve son 16 baytı sıfır olan datagram AEAD'siz `HubChannel`'a atfedilir (`protector.rs:645-650`; `open_hub` `:984-1004`). K4G-01 (#114) sonrası runtime bu bağlamı hiçbir Session dağıtıcısına sokmaz; `on_hub_frame` yalnız `hub_frames_dropped`'ı artırır. "Session ≠ HubChannel anahtarı" yarısı `scripts/ci/noise-seam-coverage.json` `H-CTX/key` satırında `real: declared`. | BL KATMAN 6 madde 2 (BL:527); PoC-KAPSAM-001 §3, §5 | M4 (HubChannel gerçek hop anahtarı) |
| 3 | **0-RTT yok:** INIT'e gömülü `early_unit` yok. Boş olmayan `early_unit` → `Failed` (alıntıda #104 YORUM 4). Transport `early_unit`'i saklar ve `Ready`'de ilk `send()` ile yollar (#109; S1 kararı 2026-10-08). | Plan K4G-16 NE 1 (KATMAN 6 listesi); plan §5 | KATMAN 6 "INIT'e gömülü 0-RTT (`early_unit`)", K4G-16 ile yazılır; LinkChanged → `SESSION_INIT_REF` hub sprintinde (D15) |
| 4 | **INIT retry yok (T3).** Transport INIT'i yeniden göndermez; #109: "INIT byte-özdeş yeniden gönderimi yok (T3)". Crypto'nun önbellekli, bayt-özdeş INIT çıktısı vardır (H-RETRY, alıntıda); onu kullanan yeniden gönderim kadansı yoktur. | #109 "Beyanlı boşluklar / yorumlar"; PoC-KAPSAM-001 §5 ("INIT yeniden gönderim kadansı yok") | KATMAN 6 "T3 INIT retry", K4G-16 ile yazılır; hub sprinti |
| 5 | **Extended 26 B zarfın bağımsız Python vektörü yok.** Extended zarf yalnız mevcut koruyucuya karşı diferansiyel testle sınanır (S10, sahip kararı 2026-10-08): `crates/kama-noise-wire/tests/a5b_protection.rs:4`, `s10_extended_*` testleri; alıntıdaki tabloda `envelope.rs` satırı. | Plan §5; plan K4G-16 NE 1 | KATMAN 6 "extended 26 B zarf için bağımsız Python vektörü (S10)", K4G-16 ile yazılır |
| 6 | **#114'ün beyanlı sınırları:** INV-ACK-DELIVER-1'in `Ready`/`NotReady` kapsamı dışında kalan ACK'li kayıp yolları: (a) teslim kanalı taşması; (b) `Closed` sonrası ACK'li kuyruk; (c) provisional `NotReady` + Karn. Metin aşağıda aynen. | #114 "Beyanlı istisnalar" 3(a)–(c); #116 "#114'ün beyanlı sınırları (a)/(b)/(c) — bu PR'da UYGULANMADI" | **K4G-01b kapsamına önerildi, Abkarada kararı bekleniyor.** K4G-01b (#116) bunları uygulamadan merge edildi; #116 tasarımın bunları kötüleştirmediğini beyan eder. |
| 7 | **K4G-01b beyanlı açık:** hattaki saldırgan, gözlediği geçerli datagramı sahte adresten gerçeğinden önce ileterek `max_peers` içinde ek yuva açabilir; S1'de çift teslim yok. Metin aşağıda aynen. | #116 "INT-02 beyan satırı (hazır)"; plan K4G-01b NE 4 | M4 hop anahtarı / adres doğrulama (PoC-KAPSAM-001 §5) |

**Satır 6'nın kaynağı, #114 "Beyanlı istisnalar" madde 3 (aynen):**

> 3. **INV-ACK-DELIVER-1 — bu PR'ın kapsamı DIŞINDA kalan ACK'li kayıp yolları (iddia daraltılır; koşulsuz INV denmez):**
>    - (a) **Teslim kanalı taşması:** `runtime.rs` `push_delivery` (`delivery_tx.try_send`) kanal doluyken (≥ 1024 teslim; S1 uygulaması `recv()`'i boşaltmazsa) ACK'lenmiş birim düşer (`delivery_dropped`); `delivered_seen` yeniden gönderimi dedup eder → ACK'li kayıp. **Tabanda da var** (bu PR'ın yolu değil). *K4G-01b kapsamına önerildi — Abkarada kararı.*
>    - (b) **Yerel `close` / `Closed(h)` sonrası:** teslim kanalında bekleyen, ÖNCE ACK'lenmiş teslimler düşer (geri basınçta ≤ 1024'e kadar büyür); `Superseded` oturumun `Closed`'ı okunup oturum tablodan çıkarılırken veri alıcısında açılmış ama henüz kanala konmamış birim için dar bir yarış penceresi de bu sınıftadır. *K4G-01b kapsamına önerildi — Abkarada kararı.*
>    - (c) **Provisional onay + Karn / PreConverge tabanı:** provisional oturumda onayı getiren datagram (ve onay işlenene dek gelenler) `NotReady` olarak düşer; ilk uçuştaki DATA düşerse gönderen srtt oluşana dek PreConverge'in 31 B/s tabanında kalabilir (Karn: yeniden gönderim RTT örneği vermez; büyük birim birikiminde saniyeler mertebesi). Ölçüm INT-02'de. *K4G-01b kapsamına önerildi — Abkarada kararı.*

**Satır 7'nin kaynağı, #116 "INT-02 beyan satırı (hazır)" (aynen):**

> **K4G-01b beyanlı açık:** hattaki saldırgan gözlediği geçerli (AEAD'li) datagramı sahte bir adresten gerçeğinden ÖNCE ileterek veri alıcısında `max_peers` (64) içinde ek bir `(adres, oturum)` yuvası açabilir (gözlenen datagram başına bir yuva; o datagramın ACK'i sahte adrese gider, gönderen ARQ ile yeniden yollar; S1'de çift teslim YOK — oturum anahtarlı `Reorder`). Tablo bu yolla dolarsa yeni meşru yuvalar `dropped_new_peers` ile reddedilir (ek yuvalar `idle_hold` = 105 s ya da bağlı oturum kapanınca boşalır). Kapanış: M4 hop anahtarı / adres doğrulama (PoC-KAPSAM-001 §5).

Bu tablodaki kalemler G-IMPL-S3 değerlendirmesi için beyan edilmiştir. PoC'nin bütün beyanlı açıkları `docs/decisions/PoC-KAPSAM-001.md` §5'tedir.

Abkarada'nın 'G-IMPL-S3 GEÇTİ' mesajı bekleniyor.
