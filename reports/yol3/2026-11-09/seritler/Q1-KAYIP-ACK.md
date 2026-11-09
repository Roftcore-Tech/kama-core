# Q1 — quinn kayıp/ACK farkı: A-1, A-2, A-5 kodu (2026-11-09)

Dal `yol3/q1-quinn-ack` · taban `yol3/m1-e2a-goster` 46c0d30d · worktree `/home/ryuzaki/kama-wt/yol3-q1` · SHA **507417b7** (`b033b279` adaptive-transport + `507417b7` client-core), origin'e itildi.
Kapsam: `QUINN-FARKI-KOD-KARSILASTIRMA-2026-11-09.md` (sha256 85c4d27e…) §3-A maddeleri **A-1, A-2, A-5**.
Eşleme tablosu o belgede (§1); burada yalnız bu üç maddenin kodu. A-3/A-4 (kova tavanı, ProbeUp) başka şeritte:
`sender.rs`'in o bölgelerine dokunulmadı. quinn kaynağı: `quinn-proto 0.11.18` (MIT/Apache-2.0; kopyalanan her
kural yorumda dosya + fonksiyon + lisans atfıyla).

Etiketler: [ölçüldü] · [kod] · [türetilmiş] · [çıkarım].

## 1. Bayraklar (hepsi `loss::E2aFlags`, varsayılan KAPALI, kapalıyken bit-aynı)

| Madde | Bayrak | Nasıl açılır | Okuyan |
|---|---|---|---|
| A-1 | `gap_ack` (m5; tipli hata `GapAckNotImplemented` kaldırıldı) | `E2aFlags::new(.., gap_ack = true)` | `Receiver` |
| A-2 | `return_prior_gated` | `.with_return_prior_gated(true)` | `Sender` (kayıp tabanı) + `Receiver` (timer cadence) |
| A-5 | `pto_rfc` (+ mevcut `pto_probes = 2`) | `.with_pto_rfc(true)` | `TailRecovery` (sender üzerinden) |

`E2aFlags::new` imzası değişmedi (6 argüman); yeni kollar `with_*` ile (bool, hatasız). Ürün yolu: `RuntimeConfig.e2a_flags`
→ gönderici (`set_e2a_flags`, M1'den) **ve** server ucunun eş-başı `Receiver`'ı (`client-core/src/runtime.rs:3556-3560`, yeni; varış baytı `:3565-3566`).

## 2. Madde madde

### A-1 — boşlukta / geç varışta anında ACK

- quinn [kod]: `connection/spaces.rs:725` `immediate_ack_required |= is_out_of_order(..)`; `:736-748`
  `reordering_threshold = 1` → `pn < prev_largest || missing_in_interval(prev_largest, pn)` (RFC 9000 §13.2.1).
- Kama [kod]: `receiver.rs:367-374` `out_of_order = gap_ack ∧ have_recv ∧ (seq < largest ∨ seq > largest+1)`
  (kayıt güncellenmeden önce); `:399-403` `tick_ack(now, out_of_order)` + sayaç. Hata varyantı kaldırıldı: `loss.rs:754`. En büyüğün üstünde seq alınmamış olduğundan
  "arada eksik" ⟺ `seq > largest + 1`. İlk datagram sınıflandırılmaz.
- Belgeden fark: belge "has_gap() yeni boşluk ürettiyse" diyor; quinn satırı **geç varışı da** (boşluğu dolduran
  paket) kapsıyor — quinn'e sadık olan alındı. Geç varışta anında ACK, sahte kayıp ilanının (un-loss) erken çözülmesini sağlar [çıkarım].
- Gözlem sayacı: `Receiver::diag_e2a().gap_ack_emits` (frekans yolunun zaten üreteceği ACK'ler sayılmaz).
- Maliyet [türetilmiş]: ek ACK ≤ sıra dışı varış sayısı; %20 kayıpta en çok ≈ her 5 datagramda +1 (frekans yolu zaten 2'de 1).

### A-2 — 31 B/s önseli rejim kapılı

Gönderici [kod] `sender.rs:3485-3537` (`cadence_loss_floor` :3492, `loss_floor_return_bw` :3505, `expected_ack_interval_with` :3526):
- `cadence_loss_floor()` = 1,5 × `expected_ack_interval_with(loss_floor_return_bw())`.
- `loss_floor_return_bw()`: bayrak kapalı ∨ pace < 1 kB/s (`E2A_RETURN_REGIME_MIN_BPS`, `loss.rs:738`) → 31 B/s
  (bugün); rejimde → `ReturnPathObserver::measured_return_bw_bps()` (> 0 ise) yoksa pace; alt sınır 31 B/s.
- Tüketiciler yalnız kayıp-tabanı ailesi: zaman-eşiği tabanı, tombstone T_loss, legacy onay ufku. `expected_ack_interval()`
  (ack_epoch/W pencereleri, `ack_confidence`, ProbeUp/Drain) önselde kalır — A-3/A-4 bölgesine yan etki yok.
- `return_path.rs:13-16, :201-205`: salt-gözlem modülüne tek istisna notu + `measured_return_bw_bps()` erişimcisi.

Alıcı [kod] `receiver.rs:88-98` (parametreli formüller), `:138-141` (halka), `:233-325` (setter, `observe_arrival_bytes`, `arrival_rate_bps`, `regime_return_bw`, `cadence_floor_ns`, `t_spacing_ns`), `:446` (kota aralığı), `:456-471` (bekleyen-bilgi cadence + sayaç):
- Rejim kapısı = ileri yön **varış debisi** (göndericinin pace'inin alıcıdaki vekili; alıcı pace'i bilemez).
  Ölçüm: son 8 varışın `(an, bayt)` halkası, debi = baytlar / zaman açıklığı (aralık-oranı EWMA'sı değil; yığılmada
  sonsuza gitmez). Runtime besler: `client-core/src/runtime.rs:3565-3566` `observe_arrival_bytes(n, now)`.
- Rejimde rb = `set_return_bw_hint` (varsa) yoksa varış debisi; bekleyen-bilgi cadence'i =
  `clamp(K_ARR·last_gap, max(25 ms, 65 B/rb), max(25 ms, 65 B/(0,25·rb)))`, kota aralığı `max(25 ms, 65 B/rb)`.
  25 ms = `E2A_ACK_CADENCE_MIN_NS` (`loss.rs:743`; quinn `transport_parameters.rs:60` max_ack_delay = 25 ms). Altında sabitler.

Belgeden bilinçli sapmalar (gerekçeli):
1. **Boşta tekrar tabanı önselde kaldı** (yeni bilgi yokken 8,39 s). Belge "cadence = max(25 ms, ·)" diyor; o kola da
   uygulansa rejimde yeni veri olmadan 25 ms'de bir tekrar SACK'i olurdu (40 ACK/s × 65 B = 2,6 kB/s, eş ömrü boyunca)
   [türetilmiş]. quinn yeni veri yokken hiç ACK üretmez [kod: `spaces.rs` `acks_sent` + `on_max_ack_delay_timeout`].
2. **Ölçülen debinin alt sınırı 31 B/s.** `measured_return_bw_bps` ACK debisidir (alıcının ACK ritmiyle sınırlı alt
   sınır). Seyrek ACK'te ≈ 65 B / 8,39 s ≈ 7,7 B/s; alt sınırsız kullanılırsa taban bugünkünden uzar [türetilmiş].
3. **Y = 0,25 dönüş bütçesi korundu** (spec §16.1); rejimde 12,5 kB/s'de iki formül de 25 ms'ye oturur, fark yalnız
   rejim sınırında (1 kB/s: 260 ms ↔ 65 ms).
4. **Alıcı ipucu runtime'da beslenmiyor**: alıcı kendi çıkış yolunu ölçemez (ACK'in ACK'i yok); doğru kaynak aynı
   düğümdeki ters yön göndericisinin BtlBw'si olurdu, eş eşlemesi client/server görevleri arasında ayrı kart.

tt_rfc (M1) ile ilişki — çakışma yok [kod]: `tt_rfc` açıkken zaman eşiği ve tombstone T_loss tabanı zaten `g`
(`sender.rs` `time_loss_inputs`); A-2 bunları değiştirmez (test `q1_a2_with_tt_rfc_only_confirm_horizon_remains`).
`tt_rfc` açıkken A-2'nin göndericide kalan tek etkisi **legacy onay ufku** (`LossConfirmHorizon::CadenceLegacy`,
Suspected→Confirmed; `loss_windowed_ppm` → fuzzy/FEC kararları). A-2'nin alıcı kolu `tt_rfc`'den bağımsızdır.

Gösterim hücresi a_r12k_d150_l20_foto150k:
- pace (alice→bob, 5–60 s, PreConverge dışı) [ölçüldü, G2 jsonl `pr`]: K1 medyan 12,0 kB/s, örneklerin %4,3'ü < 1 kB/s;
  K0 medyan 9,6 kB/s, %21'i < 1 kB/s. srtt medyanı K1 331 ms, K0 395 ms. → gönderici çoğunlukla rejimde.
- Taban (tt_rfc kapalı, U ≈ 1200 B, pace 12 kB/s) [türetilmiş]: bugün 1,5·(0,2 s + 2,097 s) ≈ 3,4 s; A-2 ölçümsüz
  (rb = pace) 1,5·(0,2 + 0,005) ≈ 0,31 s → eşik = max(9/8·0,331, 0,31) ≈ 0,37 s ≈ quinn. A-2 **ölçümlü** (ürün runtime'ı
  `ReturnPathObserver`'ı besliyor) rb = ACK debisi; 350–650 B/s ise taban 0,44–0,57 s (≈ 1,3–1,7 RTT). Hücredeki gerçek
  `measured_return_bw_bps` jsonl'de yok — ölçülmedi.
- Alıcı: dosya sırasında varış ≈ pace → rejim (bekleyen-bilgi cadence 25 ms); PTT-yalnız evrede 80 B/200 ms ≈ 0,4–0,65
  kB/s → önsel (bugünkü davranış) [türetilmiş].

### A-5 — PTO: çapa son ack-eliciting gönderim, tur tavanı yok, 2 prob

- quinn [kod]: `connection/mod.rs:1834-1870` `pto_time_and_space` (çapa `time_of_last_ack_eliciting_packet`, `:1860-1864`;
  backoff `2^min(pto_count,16)`, `:1835`, `:4056`); `:1683-1689` PTO başına 2 prob; `paths.rs:331-333`
  `pto_base = srtt + max(4·var, 1 ms)`; `:1562` ACK'te `pto_count = 0`.
- Kama [kod] `tail_recovery.rs`: `:119-128` alanlar + `PTO_RFC_GRANULARITY_NS`; `:158-188` `set_rfc_mode`,
  `note_ack_eliciting_sent`, `pto_deadline_ns`; `:200-207` taban `max(4·var, 1 ms)` (yalnız rfc); `:269-297` rfc'de
  tur tavanı yok, vade = son ack-eliciting gönderim + PTO·2^k, ayrı "ACK ilerlemesinden bir PTO" koruması yok
  (ilerleme `probe_index`'i sıfırlar = quinn `pto_count = 0`).
- `sender.rs`: `:2210-2214` ve `:2221-2223` bayrak iki kurulum sırasında da makineye taşınır; `:2716-2723` uyanış
  vadesi tek kaynaktan (`pto_deadline_ns`; kapalıyken eski `armed_at + current_pto` ifadesi); `:6436-6437`
  `track_sent` her ack-eliciting gönderimi (veri/retx/prob/kopya) çapaya yazar.
- 2 prob: M1'in `pto_probes = 2`'si kullanılır; A-5 tam kol = `pto_rfc` + `pto_probes = 2` (test ikisini birlikte koşar).
- Korunan sınır: `max_backoff_pto_ns` (validated_v1'de 30 s) geri çekilme tavanı olarak kalır → tavansız turlar en çok
  30 s'de bir prob [kod]. RFC'de tavan yok, quinn üssü 16'da keser; Kama'da bağlantıyı idle-timeout değil bu tavan sınırlar.

## 3. Kapalıyken bit-aynılık argümanı

- A-1: `out_of_order` bayrak kapalıyken `false` → `tick_ack(now, false)` = eski çağrı. `freq_due` yalnız sayaç için.
- A-2: `cadence_floor_for(31)`/`t_spacing_for(31)` eski sabit ifadelerinin AYNISI (`const`; test sabit değerleri pinler:
  8 387 096 774 / 2 096 774 193 ns); `expected_ack_interval_with(31)` eski tamsayı ifadesi. Varış halkası ve ipucu
  yalnız bayrak açıkken okunur.
- A-5: `rfc_mode = false` → `base_pto_ns`, `on_timer`, `pto_deadline_ns` eski dallar; `last_ack_eliciting_send_ns`
  yazılır ama okunmaz.
- Tel baytları, altın vektörler, `s3.rs`/`handles.rs`/`protector.rs`: dokunulmadı. INV-RATE-1: hiçbir kol pace'i
  değiştirmez (A-2 pace'i yalnız OKUR). INV-ENGINE-SOC: kripto/anahtar yok. `todo!()` yok.

## 4. Testler [ölçüldü: yerel koşu, `cargo-lock.sh`, toolchain 1.96.0, `--locked`]

| Komut | Sonuç |
|---|---|
| `test -p adaptive-transport --lib` | **187 geçti / 0 kaldı** (M1 tabanı 166 + 21 yeni) |
| `test -p adaptive-transport --test e2a_goster --test ek4_a2_liveness --test receiver_ack_policy --test tv_ack --test tv_loss --test tv_arq --test tv_rtt --test tv_pace --test sim_determinism --test sender_loss_path --test emergency_and_accessors --test cold_start_liveness --test burst_loss_recovery --test sender_diag_contract` | 14 ikili, **78 geçti / 0 kaldı** / 1 yok sayıldı (önceden `#[ignore]`) |
| `test -p client-core --lib` | **46 geçti / 0 kaldı** |
| `clippy -p adaptive-transport --all-targets -- -D warnings` | temiz |
| `clippy -p client-core --all-targets -- -D warnings` | temiz |
| `fmt --all -- --check` | temiz (rc 0) |

Koşulmadı: adaptive-transport'un kalan entegrasyon ikilileri (soak/sweep sınıfı), diğer crate'ler, release, netem.

Yeni birim testleri (iki kollu, sahte saat, deterministik):
- A-1 (`receiver::q1_tests`): `a1_gap_ack_immediate_on_new_gap_and_late_arrival` (kapalı `[F,T,F,T,F]` ↔ açık
  `[F,T,T,T,F]`), `a1_gap_ack_reports_the_gap`, `a1_in_order_stream_is_identical_in_both_arms` (sıralı akışta tüm ACK +
  timer baytları aynı), `a1_first_datagram_not_classified_and_duplicate_unchanged`; `sender::..::e2a_flags_default_is_today_and_new_validates` (gap_ack artık geçerli).
- A-2 alıcı: `a2_prior_formulas_reproduce_shipped_constants`, `a2_arrival_rate_ring`,
  `a2_in_regime_pending_cadence_is_25ms_class` (kapalı 1000 ms ↔ açık 820 ms), `a2_below_regime_is_identical_to_off`
  (400 B/s'de çıktı dizisi bayt-aynı), `a2_off_ignores_measurements_entirely`, `a2_hint_replaces_symmetric_proxy_in_regime`,
  `a2_idle_repeat_floor_stays_on_prior_in_regime`.
- A-2 gönderici (`sender::..::e2a_goster`): `q1_a2_loss_floor_regime_gated`, `q1_a2_below_regime_and_low_measurement_fall_back_to_prior`,
  `q1_a2_with_tt_rfc_only_confirm_horizon_remains`.
- A-5 (`tail_recovery::q1_pto_rfc_tests`): `off_is_default_and_base_pto_unchanged`, `anchor_is_last_send_not_ack_progress`,
  `rfc_has_no_round_cap_and_backs_off_from_each_probe`, `rfc_backoff_bounded_by_ceiling_and_progress_resets`;
  gönderici: `q1_a5_pto_rfc_set_order_independent_and_off_is_default`, `q1_a5_anchor_last_send_fires_earlier_and_rounds_uncapped`
  (kapalı 2 tur ↔ açık > 2 tur, ilk prob 120 ms), `q1_a5_full_arm_is_pto_rfc_plus_two_probes`.
- Mevcut testler ve M1'in bit-aynı iz testi (`tests/e2a_goster.rs` `e2a_flags_off_trace_is_bit_identical_to_base`) değişmeden geçer.

## 5. Ölçüm önerisi (mini-Ö; netem orkestratörde)

Taban K1 (gösterimin en iyi kolu). Önerilen kollar, tek-değişken sırasıyla:
1. **K1 + A-1** (`gap_ack`) — en ucuz, alıcı tarafı.
2. **K1 + A-1 + A-2** (`return_prior_gated`) — tt_rfc KAPALI: A-2 zaman eşiği tabanını da taşır.
3. **K1 + A-1 + A-2 + A-5** (`pto_rfc` + `pto_probes = 2`).
4. Kontrol: **K1 + tt_rfc + A-1 + A-2 + A-5** — tt_rfc'nin üstünde A-2 yalnız onay ufku + alıcı cadence'i ekler.
Okunacaklar: PTT p95, ilan→retx aralığı, `gap_ack_emits`, `regime_timer_emits` (alıcı), `measured_return_bw_bps`
(göndericide hangi A-2 dalının çalıştığı — §2 ölçülmedi notu), prob sayısı ve sahte prob oranı.

## 6. Açık kalemler

- Alıcı ipucunun (`set_return_bw_hint`) runtime kaynağı yok (§2 A-2 sapma 4).
- `measured_return_bw_bps` aralık-oranı EWMA'sı; yığılmış ACK'te yukarı sapar [kod: `return_path.rs:157-164`] →
  A-2 göndericisini daha agresif yapar (kayıp hızı düşürmez; bedel yalnız fazladan retx). Ölçümde `measured` ile
  pace yan yana okunmalı.
- Benim ilk taslağımdaki iki ek kol (alıcı max_ack_delay zamanlayıcısı, sabit paket eşiği) kapsam güncellemesiyle
  çıkarıldı: ilki rejimdeki A-2 25 ms cadence'iyle örtüşür; ikincisi belgede "Eşdeğer — Koru".
