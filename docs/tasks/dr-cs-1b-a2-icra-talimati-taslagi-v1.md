# DR-CS-1b / A2 — FAZ-BAZLI İCRA-TALİMATI TASLAĞI v1 [K-6 ADIM-8; ONAY BEKLİYOR]

**Statü:** TASLAK — bağlayıcı talimatı Abkarada verir (roadmap-§7 döngüsü: Claude-taslak → Abkarada-
inceleme → bağlayıcı-talimat). Bu taslak onaylanmadan **PRODUCTION IMPLEMENTATION NO-GO devam eder.**
Normatif-taban (çelişkide sıra): Abkarada-mühür/karar-metinleri → v2-karar-amendmanı (K-1..K-7) →
[CAL]-freeze-r2 [SEALED—SCOPE-BOUNDED] → tasarım-v2 → tasarım-v1 (referans-bölümleri).
Değişiklik-yüzeyi: **MUTLAK-SINIR v1** (adım-7 belgesi; DUR-B-2-onaylı hali). A1 SEALED — dokunulmaz.

> **BAĞLAYICI ÇERÇEVE (devralınan):** DR-CS-1b cold-start ürün-alıcı DR'ıdır; "ARQ-vs-FEC" dili yasak;
> 1200B-yasak-iddialar geçerli; merge/default-flip yasak; pre-registered değerler outcome'dan seçilemez.

---

## 0. FAZ HARİTASI + KAPILAR (genel görünüm)

```text
A2-F0  iskelet + flag-propagasyonu + saat-sınıfı           [MUTLAK-SINIR onayı = DUR-B-2 sonrası]
A2-F1  ColdRxState: defter + cap'ler + iade-YAPISAL-zinciri + zeroize + memory-model-testi
       [REPLAY_PAIR_CAP ★onay-kapılı]
A2-F2  patience (P-1..P-6) + S-B + tombstone-K-1-ürün-kapanışı + stateless-echo-ürün-yolu
       [SESSION_IDLE_HOLD/TOMBSTONE_GRACE/T_ECHO_MIN ★onay-kapılı]
F-CID-0  ★ ayrı-kapı: producer-inventory (RECOVERED=normatif-bound-istisnası) → hard-türetim →
       CAL-10-freeze (Abkarada-onay)
A2-F3  fixed-window dedup (YALNIZ CAL-10-mühür-sonrası) + g5-amendmanı (DUR-B-4a) + Closing-entegrasyon
A2-F4  notification-contract (rx-emisyon [★kota-onaylı] + sender-Tetik-2 [DUR-B-3]) + 1/1/1-pinler
       + CAL-10-doğrulama-kapısı (F4.5)
A2-F5  pre-reg-freeze (DUR-B-5; tüm-F2-adayları donmuş-ŞART) → plateau (F-A/B/C) + W-3-65-ürün-rerun
       + DI-10-ürün-varyantları
DUR-SON  kabul-matrisi + hüküm + status-board
```
Her faz-sonu: fmt/clippy(-D, iki-config)/test yeşil + flag-OFF-özdeşlik + A1-pinler + yüzey-denetimi
(MUTLAK-SINIR §3) + DUR-A faz-raporu. Pre-reg/gerçeklik çelişkisi → DUR-B (A1-disiplini).

## 1. A2-F0 — KOMPOZİSYON-KÖKÜ İSKELETİ + FLAG-PROPAGASYONU

```text
F0.1  experimental-cold-start feature'ı kama-engine + kama-noise-wire + kama-integration
      Cargo.toml'larına eklenir (workspace-tutarlı); İLK commit'te her-crate flag-OFF-özdeşlik pini
      (test-pin; derlenmiş-özdeşlik nerede ölçülebilirse ek-kanıt).
F0.2  kama-integration üretimleştirme: ReceiverHost iskeleti (typed-error stub'lar;
      FeatureNotImplemented-deseni; reachable todo!() YASAK).
F0.3  kama-engine ColdRxState iskeleti: dört-eksen defter tipleri + [CAL]-SEALED sabitler
      (freeze-r2 değerleriyle bire-bir; tuning-yasak yorum-pinli) + CapExceeded/5-diag/
      ParamNeverArrived/InvariantViolation typed-varyantları (stub).
F0.4  ZM-3: patience/hold/grace timer'ları §16.2-tablosuna saat-sınıfıyla KAYDEDİLİR (doc-commit;
      receiver-monotonic; ColdRxState saat-tutmaz — now enjekte).
F0.5  Kabul: iki-modda derleme + tüm-mevcut-testler yeşil + yüzey-denetimi temiz.
```

## 2. A2-F1 — DEFTER + CAP'LER + İADE-ZİNCİRİ + ZEROIZE + MODEL-KAPISI

```text
F1.1  Rezervasyon-defteri (v1-§4.1 semantiği, v2-§3-eksen-eşlemesiyle): admit-cold-shard →
      {SHARDS_PER_BLOCK ön-kapı → PREPARAM-alt-kapı (min=192) → session-eksenleri → global-eksenler}
      ATOMİK; typed-outcome {Admitted, Rejected(kind)}; kalıcı-retention-öncesi.
F1.2  SEALED-cap'ler (freeze-r2 birebir): OPEN_BLOCK=8 · SHARDS_PER_BLOCK=24 ·
      MAX_SHARDS_BY_BLOCK_SHAPE=192 (#5b; 8×24-türetim-kaydı) · TOTAL_SHARD=192 · PREPARAM=192 ·
      RETAINED_BYTE=480KiB · GLOBAL_COLD_BYTE_CAP=15MiB (#3; #1-GLOBAL_COLD_RETAINED F5.2-bound'udur) ·
      SESSION_COUNT=64 · TOMBSTONE_REGISTRY=64.
F1.3  C-8 5-diag alanı + sayaçlar (GlobalColdBudgetExceeded · SessionColdBudgetExceeded ·
      ColdAdmissionRejectedByGlobalBudget · active_sessions_at_global_exhaustion ·
      distinct_authenticated_peers_at_global_exhaustion).
F1.4  İade-zinciri — YALNIZ YAPISAL-ZİNCİR (verify-düzeltmesi; K-1-ürün-kapanışı F2.4'te): Closing →
      precompute-sealed-CLOSE-ACK (send-cipher canlıyken; HOST; protector-drop_send_state = A-4-amendman-
      kapılı MUTLAK-SINIR-dilimi) → teardown (send-state + ColdRxState-release + zeroize) →
      minimal-tombstone-kurulum → grace-sonu (unregister dahil) mekanizması; PLACEHOLDER-grace ile
      MEKANİK test (değer-bağımsız). PersistentReplayGuard-KALIR; **REPLAY_PAIR_CAP Abkarada-onaysız
      KODLANAMAZ** (§9) — onay-öncesi restart.rs-dilimi typed-stub. K-1 7-pin'in ÜRÜN-kapanışı ve
      TOMBSTONE_GRACE-bağımlı pinler AÇIKÇA F2.4-teslimatıdır (F1-kabulüne "K-1-pin-kapanışı F2'de" şerhi).
F1.5  Zeroize: PreParamHold drop/evict + eviction/timeout-path (fec-core-§9 P0-9).
F1.6  ★ MEMORY-MODEL-TESTİ (freeze-r2-§8 kapısı): ölçülen SHARD_RETAINED_COST(1024) ≤ 1536 B assert;
      aşarsa DUR-B (cap'ler yalnız AŞAĞI yeniden-türetilir).
F1.7  Adversarial cap-testleri: her-eksen tek-tek doldurulup typed-reject + iade-sonrası-yeniden-admit;
      sessiz-eviction-erişilemezlik (A-1-dalı ölü-kod-pini).
```

## 3. A2-F2 — PATIENCE + S-B + TOMBSTONE

```text
F2.1  P-1..P-6 (v2-§2 NON-RENEWABLE formu): block_deadline = first_authenticated_shard_time +
      RX_COLD_ABANDON(profil) — MUTLAK; sonraki-shard progress-günceller, deadline'ı ASLA
      resetlemez/uzatmaz; probe/control NE block- NE session-anchor'ı besler; P-6 pre-PARAM
      blok-deadline'ı (ParamNeverArrived-diag).
F2.2  RX_COLD_ABANDON: F0-(viii)-donmuş profil-tablosu (65/300B→75s; 1200B→105s-tavan/S-B);
      f3_m6-formül-kontratının ürün-varyantı test-pinlenir.
F2.3  S-B session-hold: SESSION_IDLE_HOLD [CAL-adayı; profil-türevi + ZM-2-4-decade] — yalnız
      canlı-blok-yokken authenticated-DATA anchor-yeniler.
F2.4  Tombstone-hibrit K-1 7-pin ÜRÜN-KAPANIŞI (F1.4-yapısal-zincirin üstüne; TOMBSTONE_GRACE-onayı
      sonrası): unauthenticated→cached-response-YOK · authenticated-CLOSE-retry→AYNI-cached-ciphertext
      (byte-for-byte; re-seal-YOK) · yeni-cold-alloc→typed-reject · send-cipher-grace'te-YAŞAMAZ ·
      recv-cipher-yalnız-grace · grace-expiry→route+recv+cached-ACK-düşer · registry-dolu→fail-closed-
      typed-diag. TOMBSTONE_GRACE [CAL-adayı] ≤ SESSION_IDLE_HOLD.
F2.5  STATELESS-ECHO ÜRÜN-YOLU (verify-eksik-madde): ReceiverHost-demux'ta probe→echo (A-2 caller-
      dispatch; g10c/g10d ürün-varyantı: sabit-buffer, sıfır-alloc, O(1)) + T_ECHO_MIN(profile)
      [CAL-adayı: max(security_min, serialization+margin); değer-önerisi → ★onay → kod].
F2-KABUL (adlandırılmış pinler): sonraki-shard-deadline-UZATMAZ · probe/control-anchor-BESLEMEZ
      (block+session) · pre-PARAM-blok-mutlak-süre + ParamNeverArrived · S-B-yalnız-canlı-blok-yokken ·
      f3_m6-ürün-varyantı · K-1-7-pin · echo-O(1)-working-set.
```

## 4. ★ F-CID-0 — CONTROL-ID KAPISI (Abkarada-mühür-kararının şartı; fixed-window-ÖNCESİ)

```text
F-CID-0.1  LOGICAL-CONTROL PRODUCER INVENTORY: control_id-tüketen TÜM üreticiler (ACK/SACK-split ·
           RECOVERED · BLOCK_COMPLETE/ABORT · PARAM_SET/ACK · SESSION_CLOSE/RENEW · retry-yolları)
           kod-satır-referanslı envanter.
           ★ RECOVERED-İSTİSNASI (verify-BLOCKER-çözümü): receiver-tarafı RECOVERED-emisyonu üretimde
           HENÜZ YOK (F4.1'de doğar; harness-yolu üretim-sayılmaz) → envantere kod-satırı yerine
           NORMATİF-BOUND ile girer: RECOVERED-rate ≤ SACK-return-supply-bütçesi (kota/T_SPACING —
           mevcut receiver-sabitleri). [Alternatif — Abkarada-seçimi: F4.1'i F-CID-0-öncesine almak;
           taslak normatif-bound-yolunu öneriyor.]
F-CID-0.2  HARD-türetim + ENFORCE: MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION · MAX_CONTROL_ID_BURST ·
           MAX_CONTROL_REORDER_ALLOWANCE · MAX_RETRY_REORDER_HORIZON — emisyon-yoluna enforce-sabitleri
           olarak bağlanır (gözlem-değil-kanıtlı-max; A-3-monoton-atayıcı bu fazda doğar).
F-CID-0.3  required_window = RATE × RETRY_REORDER_HORIZON + BURST + REORDER_ALLOWANCE hesabıyla
           CAL-10-ADDENDUM belgesi → hash+timestamp → ★ ABKARADA-ONAYINA sunulur.
F-CID-0.4  CAL-10-mühürsüz A2-F3'e GEÇİLMEZ.
```

## 5. A2-F3 — FIXED-WINDOW DEDUP (CAL-10-sonrası) + g5-AMENDMANI

```text
F3.1  ControlDedupWindow (per-session/yön; high-water + sabit-bitmap; pn.rs-deseni) — CAL-10-değeriyle.
F3.2  ControlIdBelowDedupWindow ayrık-diag (DuplicateAckOnly-yanıtına ek).
F3.3  admit()-imzası değişmez; probe/echo caller-dispatch (A-2) + debug_assert.
F3.4  g5 DUR-B-4a-isimli-amendmanı uygulanır (probe-admit satırları g10c-lehine; kalan-11 dokunulmaz).
F3.5  Closing-teardown-entegrasyonu (v2-§4-(2b); F1.4-ileri-referansının kapanışı): pencere Closing'de
      düşer; CLOSE-retry'ların dedup'a DEĞİL tombstone-yoluna düştüğü test-pinlenir.
F3-KABUL: window-sınır test-matrisi (id>high-water / window-içi-işaretli / window-içi-temiz /
      window-altı+diag-sayacı) · g5-amendman-sonrası kalan-11-yeşil · F3.5-tombstone-yolu-pini.
```

## 6. A2-F4 — NOTIFICATION-CONTRACT (D-6/K-2)

```text
F4.1  Receiver-emisyonu: RECOVERED (0x12) bütçesi SACK-return-supply-defterinden (kota/T_SPACING-içi).
F4.2  ★ DUR-B-3-ONAYI SONRASI sender-Tetik-2 (MUTLAK-SINIR §2.4 dilimi): TimedOut-sonrası
      RecoveredControl → PostTimeoutNotice{session/dh_epoch, cold_epoch, block_id, cause, proof_time};
      first-valid-proof-wins; take_post_timeout_notice() pull-API.
F4.3  1/1/1-pinler (SEALED): NOTICE_BOUND=1 · MAX_COLD_BLOCKS_PER_EPOCH=1 (yalnız BootstrapColdBlock-
      sınıfı; kimlik=(session_or_dh_epoch,direction,cold_epoch,block_id)) · MAX_ACTIVE_COLD_EPOCHS=1
      (HashMap-yasak; tek-CURRENT-slot). Aynı-epoch/farklı-blok → InvariantViolation + DUR-B;
      stale-proof → IGNORE + diag; duplicate → idempotent-no-op; close → sender-tarafı release.
F4.4  Kanıt-şartları: N8-bit-özdeşlik (fires-vs-noop BtlBw) + tek-terminal-pinleri + flag-OFF-özdeş.
F4.5  ★ CAL-10-DOĞRULAMA-KAPISI (verify-BLOCKER-çözümünün ikinci-yarısı): F4.1-emisyon-yolu CAL-10
      hard-rate/burst enforce-sabitlerine BAĞLANIR; kod-satır-referansı F-CID-0-envanterine işlenir;
      gerçekleşen-emisyon normatif-bound'u (F-CID-0.1-istisnası) AŞIYORSA → DUR-B + CAL-10-yeni-versiyon
      (mühür-sonrası-değişiklik=yeniden-onay kuralı).
```

## 7. A2-F5 — PRE-REG-FREEZE → PLATEAU + W-3-65-ÜRÜN-RERUN

```text
F5.1  ★ DUR-B-5: pre-registration-freeze (hash+timestamp; kanonik-prosedür freeze-r2-§9-deseni):
      W-3-65-beklenen-zincirler (B1-amendmanlı sınıf-ayrımı) + plateau süre/tekrar-parametreleri +
      hücre-matrisi. Koşu-sonrası-değişiklik YASAK.
F5.2  Plateau (freeze-r2-§7 bound'ları): F-A/F-B/F-C; BOUND_COLD_GLOBAL=15MiB · BOUND_COLD_SESSION=
      480KiB · BOUND_SIDE_STATE=1MiB · toplam ≤16MiB; defter-sayaç==model-assert + RSS-eğim≈0.
F5.3  W-3-65 ÜRÜN-alıcı rerun (v2-§7.2 matrisi; cap-hücreleri SEALED-değerlerle [CAL]-placeholder-
      çözümlü): 65B-sınıfı TimedOut→RecoveredByForwardRecoveryControl→PostTimeoutNotice{RecoveredControl};
      300B-kontrol DataAckClocked; cap-stres varyantı; 3-tekrar; A1-enstrüman-zinciri aynen.
F5.4  DI-10 ürün-varyantları: f4_di10/g10c/g10d ürün-alıcı üstünde.
```

## 8. DUR-SON — KABUL + HÜKÜM

```text
Kabul-matrisi: F0..F5 kapıları + roadmap-A2-kabulü (DI-10-ürün + W-3-65-ürün-rerun + plateau).
Hüküm (v1-§8.3 sabit-formatları TAM-METİN; üçüncü/bulanık sonuç YASAK):
  TAM   : END-TO-END PRODUCT COLD-START FIX VALIDATED
  KISMİ : PRODUCT COLD-START PARTIAL — [EXACT-eksik-hücre-listesi]
  RED   : PRODUCT COLD-START NOT VALIDATED — [EXACT-fail-nedeni]
Default-flip HER-DURUMDA AYRI.
DUR-B-4b: roadmap:61-63-düzeltmesi (SUPERSEDED-by-B1 üst-not) bu DUR-SON hükmüne bağlanır.
Status-board güncellemesi DUR-SON-yetkisiyle.
```

## 9. AÇIK [CAL]-ADAYLARI (bu talimat-döngüsünde donacaklar; SEALED-set-DIŞI) — SABİT-BAŞINA FAZ+ONAY

| Sabit | Uygulama-faz-maddesi | Donma/onay-anı |
|---|---|---|
| SESSION_IDLE_HOLD | F2.3 | F2-içi değer-önerisi → ★Abkarada-onay → kod (F5-pre-reg-freeze'den ÖNCE donmuş olmak ZORUNDA — plateau/W365 bağımlı) |
| TOMBSTONE_GRACE | F2.4 | aynı (≤ SESSION_IDLE_HOLD kısıtıyla; F1.4 placeholder-grace-mekanik, değer F2'de) |
| T_ECHO_MIN(profile) | F2.5 | aynı (formül v2-§7; ZM-2-4-decade) |
| **REPLAY_PAIR_CAP** (verify-eklendi) | F1.4 (restart.rs-dilimi) | **F1.4-implementasyonundan ÖNCE** değer-önerisi → ★onay → kod; onaysızken typed-stub |
| **RECOVERED-emisyon-kota-oranı** (verify-eklendi; v1-soru-5) | F4.1 | F4.1-öncesi değer-önerisi → ★onay (SACK-return-supply-defter-payı; F-CID-0-normatif-bound'uyla tutarlı) |
| PARAM_SET_RETRY_CADENCE/DEADLINE | **AÇIK-SORU (talimat-onayı-ÖNCESİ Abkarada'ya):** sender-tarafı dokunuş DUR-B-3-ailesine mi girer, ayrı-dilim mi, A2-DIŞI-mı? | yüzey-kararıyla birlikte |
| CAL-10 (CONTROL_DEDUP_WINDOW) | F-CID-0 (§4) | CAL-10-addendum → ★onay; F3-önkoşulu |

Kural: ★-onaysız hiçbir aday-değer koda girmez (pre-registered-değerler-outcome'dan-seçilemez çerçevesi);
tüm F2-adayları F5-DUR-B-5-freeze'inden ÖNCE donmuş olmalıdır.
