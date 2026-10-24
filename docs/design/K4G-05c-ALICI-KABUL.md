# K4G-05c — Alıcı kabul bütünlüğü (R1 + R2 + sızıntı) — tasarım notu

## 0. Künye
Kart **K4G-05c** (CEVAP 9 §3) · taban `main` `3439fccd` (satırlar buradan) · **TASLAK: kod "K4G-05c ONAYLANDI" ve 05b rev4 merge'ünü bekler;** uygulanmadı, ölçülmedi. Ekler A–D. [P0] = `k4g/p0-taban` @`df7c9e62`.

## 1. Değişmez
> Alıcının tutamadığı Reliable birim ACK'lenmez (SACK'te de, FEC RECOVERED'da da); ACK'lenmiş Reliable birim uygulamaya eksiksiz, tam bir kez ve oturum içinde gönderim sırasıyla teslim edilir.

## 2. Mekanizma (bugün)
- **R2** [kod, P0]: `runtime.rs:2933-2961`: PN teslimden önce kaydedilir (`receiver.rs:233`), dedup teslimden önce ilerler (:2410), dolu kanalda payload düşer (:2380-2387), ACK koşulsuz (:2957). FEC ESI'si parse ve teslimden önce (:2649); MsgAsm reddi/tahliyesi ACK'li chunk düşürür (:2440, :2452-2455). `p0_r2_full_channel_never_acks_undelivered` (36/40 ACK'li, teslimsiz), `p0_r2_dropped_unit_blocks_followers` (12..15 hiç).
- **R1** [kod, P0]: `transport.rs:386-390` atlar, :382-384 geç eksik birimi sayaçsız atar; gönderen mesafeyi değil sayıyı sınırlar (`runtime.rs:1785`). `p0_r1_reorder_never_skips_missing_reliable` (cap 3, `1,2,3,4,0` → 0 atıldı).
- **Sızıntı** [kod]: `runtime.rs:1468-1470` store'a bakmadan ekler; budama yalnız `store.remove(..).is_some()` dalında (:1687-1693, :1737-1743) → teyit sonrası bayat karar (:1539-1549) kalıcı. Runtime P0'ı kancasız yazılamadı; motor kaynağı `p0_stale_decision_after_confirm` (05b).

## 3. Tasarım [tasarım]
**R2 — tek kabul kararı** (`server_task`). S1 kipinde oturum başına `AcceptState { floor, above ⊆ (floor, floor+cap] }`; floor = kabul edilmemiş en küçük birim; tam dedup; yuva emekliliği/adres değişimi sıfırlamaz (D.4). Yan etkisiz `admit(u)`: `u < floor ∨ u ∈ above` → Duplicate · `u − floor > cap` → Refuse(Window), `reorder_skip` · `delivery_tx.try_reserve()` başarısız → Refuse(Full), `full_refusals` · aksi → Accept(permit).
- **Refuse:** `NotReady` kolu gibi `Receiver`'a, `BlockAsm`'e, kanala girmez → SACK boşluğu, dedup durur, gönderen yeniden yollar. **Duplicate:** ACK, payload atılır. **Accept:** aynı `select!` kolunda `await`'siz `on_datagram` (ACK'e uygun) → `permit.send` (yer + payload) → `AcceptState` (dedup); izinden sonra başarısız adım yok → dördü birlikte ya da hiçbiri. Bloklayan `await` yok; SACK zamanlayıcısı duraklamada çalışır; `delivery_dropped` yapısal 0.
- **Receiver girdisi** = çağrının kendisi: `on_datagram` yalnız Accept/Duplicate ile; `receiver.rs`'e yalnız ön koşul cümlesi; tel/motor değişmez (D.1, C-1).
- **Ayrılı yer:** bütçe sayı değil floor'a bağlı kimlik penceresi; sırasız birim floor'un yerini tüketemez; paylaşımlı kanal yalnız adaptör boşaltmazken dolar, boşaltma eksik birime bağlı değil → döngüsel bekleme yok; Reorder `held ≤ cap` (D.2).
- **FEC RECOVERED / parçalı:** ESI yalnız Accept/Duplicate'te; MsgAsm'de yer yoksa chunk Refuse, tahliye kalkar (D.5).

**R1.** `Reorder::push`: `u < next` → kopya; `u − next > cap` → düşer + `reorder_skip` (savunma, S1'de erişilemez); atlama kalkar. Gönderende `runtime.rs:1785` koşuluna `next_unit + c − 1 − O < cap` eklenir (O = en eski teyitsiz, store boşsa doğru; c = mesaj başına azami chunk, ürün 1) → offer kuyruğu dolar → `send` → `Backpressure` (`transport.rs:1212`), `distance_holds`. cap = 4096 (`transport_v1_freeze.rs:98`). **Pay = 0:** teyit ⇒ kabul → O ≤ floor; kopya/ARQ/prob yeni kimlik üretmez, kopya işi sınırı (3) girmez (D.3, C-2).

**Sızıntı.** Kayıt yalnız `store.contains_key(unit)` iken; budama store sonucundan bağımsız; birim başına seq ≤ K_MAP (05b, 8) → ≤ 4096 × 8. Kanca **[DUR-23 (b)]**: `#[cfg(any(test, feature = "test-util"))] ClientHandle::unit_index_len_for_test()` (D.5).

## 4. Dosyalar · YAMA hedefi
- Kod: `crates/client-core/src/{runtime.rs, transport.rs}`; `crates/adaptive-transport/src/receiver.rs` yalnız doc (satırlar D.5).
- Test: `crates/client-core/tests/{runtime_e2e.rs, k4g_p0_receiver.rs (P0), ctx_first_dispatch.rs:220-226 (literal)}`, `crates/kama-node/src/node/tests.rs:415-456`.
- Başka dosya yok: `s1.rs`, tel/codec/`tv_*`, `sender.rs`, kama-node ürün kodu, `NodeMetrics`/UniFFI, kama-app, `api_surface_v0` dokunulmaz.
- **YAMA:** sıralı teslim sözü `docs/seams/KM-SEAM-AT-001.md:151` (§4); INV-ACK-DELIVER-1 yalnız planda (`KAMA-PLAN-4GUN-APP-KRIPTO.md:121`) ve kodda; engine-spec §4 :437 sıra garantisi vermez, transport-tdd §14 (:984-1005) INV'i taşımaz → **A.1 KM-SEAM-AT-001 §4** · A.2 plan:532 · A.3 plan:392 (C-5) · A.4 `TRANSPORT-CRYPTO-HANDOFF.md:20`.

## 5. Eski testler → Ek B
`transport.rs:1323-1339`, `runtime_e2e.rs:255-305` (:288), `runtime.rs:3334-3372`, `kama-node/src/node/tests.rs:415-456`, doc `transport.rs:564-567`; kusuru sabitleyen iddia değişir, 2×2'de "eski kod + yeni test" KIRMIZI beklenir.

## 6. Kanıt
- P0 üçlüsü 05c tabanında (main + 05b) kırmızı (yeşilse DUR) → yeşil.
- Yeni (D.6): FEC RECOVERED · parçalı · sızıntı bekçisi + `UnitSeqIndex` · kontrol yolu · gönderen mesafe · pencere ihlali; mutasyon: kabul kararı dalları (D.6); mevcut + `tv_*` yeşil (kırmızı = DUR; bilinen Ek B).
- **INT-02 `ek-tuketici-durak`:** Bob'un dinleyicisi 30 s bloklanır; Alice 50 TEXT + durak boyunca ≥ 2 MiB dosya (50 birim 1024'lük kanalı dolduramaz; C-3). 3 tohum 3/3: teslim %100, `delivery_channel_drops = 0`, `reorder_skip = 0`, çift teslim 0, durakta Bob'un ACK emisyonu > 0; geçerlilik `full_refusals > 0`.

## 7. Sıra · süre · risk · DUR
- 05b rev4 merge → rebase → P0 cherry-pick → kırmızı → kod → mutasyon → kapılar → INT-02 kolu. 05b önce: reddedilen birim ancak sahipliyse yeniden gider (INV-NO-ORPHAN; main'de D1 sonrası öksüz).
- ≈ 2,5 iş günü: kod + test 1,5 · mutasyon + kapılar 0,5 · INT-02 + rapor 0,5.
- Riskler (D.7): duraklama gönderene kayıp görünür (bant, `loss_ppm`; hız düşmez) · mesafe kapısı T-hücrede (0 beklenir) · 05b sonrası e2e sızıntı kaynağı kuruyabilir · oturumlar arası paylaşımlı kanal · örtük kipte pencere yok.
- DUR adayları: Ek C (C-1…C-7, DUR-23 (b)).

---

## Ek A — yama metinleri

Biçim: üstü çiz, silme (`~~eski~~` + yeni) + tarihli YAMA KAYDI; `scripts/ci/word_coverage.py` kapısı yeşil. `<ONAY-TARİHİ>` = "K4G-05c ONAYLANDI" tarihi. A.2/A.3'teki 2026-10-24 sahip karar tarihidir (DUR-18), yer tutucu değildir.

### A.1 — `docs/seams/KM-SEAM-AT-001.md`

(i) Satır 8'in ardına, 2026-10-08 kaydının önüne (en yeni kayıt en üstte):

> **YAMA KAYDI — <ONAY-TARİHİ> [K4G-05c alıcı kabul bütünlüğü — Abkarada kararı CEVAP 9 DUR-16 (b) / DUR-18]:** §4'e, `recv()` sıralama maddesinin ardına **teslim bütünlüğü (INV-ACK-DELIVER-1)** maddesi eklendi: alıcının tutamadığı `Reliable` birim ACK'lenmez (SACK ve FEC RECOVERED dahil); ACK'lenmiş `Reliable` birim eksiksiz, tam bir kez ve gönderim sırasıyla teslim edilir; sıralama tamponu eksik birimi atlamaz; gönderende birim-kimliği mesafesi `max_inflight_units`'e ulaşınca `send()` → `Backpressure`. INV-ACK-DELIVER-1 plan Ek 2026-10-11 (3)'te yalnız `NotReady` kolu için doğdu; bu madde onu yer (dolu gelen kutusu) ve pencere kollarına genişletir. Trait bloğu, `TransportError` kümesi ve `crates/kama-contracts/src/s1.rs` DEĞİŞMEDİ (yalnız ekleme). Üstü çizili metin yok.

(ii) §4, satır 151'in (`recv()` sıralaması) ardına yeni madde:

- **[<ONAY-TARİHİ> — K4G-05c] Teslim bütünlüğü (INV-ACK-DELIVER-1):** Transport alıcıda tutamadığı `Reliable` birimi — oturumu hazır değil, gelen kutusunda yer yok ya da birim oturumun kimlik penceresinin (`max_inflight_units`) dışında — **ACK'lemez**: ne SACK'te ne FEC kurtarım teyidinde (RECOVERED). Birim gönderende kalır ve yeniden gönderilir. ACK'lenmiş `Reliable` birim `recv()`'den **eksiksiz ve tam bir kez**, yukarıdaki sırayla çıkar: sıralama tamponu eksik birimi atlamaz; gelen kutusu sıradaki eksik birimin yerini sırasız birimlere vermez. Gönderen, en yeni birim ile en eski teyitsiz birim arasındaki kimlik mesafesi pencereye ulaşınca yeni birim almaz: `send()` → `Backpressure` (K9: yapısal hata; Application bekler). Bu madde zamanlayıcı ya da deadline getirmez (H10 korunur).

### A.2 — `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:532`

Eski satır:

```
| DUR-K4G-01 (#114/#116 üç beyanlı sınır) | KATMAN 6 kartı + INT-02 beyanı; (i) için `delivery_channel_drops` sayacı K4G-11 `metrics()`'e, INT-02'de 0 olmalı; (iii) epoch geçişinde ölçülür | K4G-16, K4G-05 |
```

Yeni satır:

```
| DUR-K4G-01 (#114/#116 üç beyanlı sınır) | KATMAN 6 kartı + INT-02 beyanı; ~~(i) için `delivery_channel_drops` sayacı K4G-11 `metrics()`'e, INT-02'de 0 olmalı;~~ (i) **2026-10-24: beyanlı sınır kaldırıldı → K4G-05c** [YAMA KAYDI 2026-10-24 — DUR-18 CEVAP, sahip] (`delivery_channel_drops` `metrics()`'te kalır; INT-02'de 0 şartı yapısal sağlanır); (iii) epoch geçişinde ölçülür | K4G-16, K4G-05, K4G-05c |
```

### A.3 — `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:392` (yalnız Ek C-5 (a) ile)

Satır içindeki parça:

```
(i) teslim kanalı taşması (`runtime.rs` ~:2321 `try_send`; INV-ACK-DELIVER-1'i ihlal edebilir → sayaç `delivery_channel_drops` K4G-11 `metrics()`'te, INT-02'de 0 olmalı)
```

yerine:

```
~~(i) teslim kanalı taşması (`runtime.rs` ~:2321 `try_send`; INV-ACK-DELIVER-1'i ihlal edebilir → sayaç `delivery_channel_drops` K4G-11 `metrics()`'te, INT-02'de 0 olmalı)~~ (i) [YAMA KAYDI 2026-10-24 — DUR-18 CEVAP, sahip] beyanlı sınır kaldırıldı → K4G-05c
```

### A.4 — `docs/transport-v1/TRANSPORT-CRYPTO-HANDOFF.md`

(i) Satır 8-9'daki "Kaynak: …" paragrafının ardına:

> **YAMA KAYDI — <ONAY-TARİHİ> [K4G-05c — Abkarada kararı CEVAP 9 DUR-18]:** §10.1 "kuyruklar" satırı: dolu teslim kuyruğunda birim artık düşmez, ACK'siz reddedilir; offer kuyruğu ayrıca birim-kimliği mesafesiyle durur. Üstü çizili metin silinmedi.

(ii) Satır 20, eski:

```
| kuyruklar | bounded: `offer_queue 1024` (dolu → `QueueFull`), `delivery_queue 1024` (taşan teslim SAYILIR, sessiz düşmez) |
```

yeni:

```
| kuyruklar | bounded: `offer_queue 1024` (dolu → `QueueFull`; **[<ONAY-TARİHİ> — K4G-05c]** en yeni ile en eski teyitsiz birim arasındaki kimlik mesafesi `max_inflight_units`'e ulaşınca offer alınmaz, kuyruk dolar), `delivery_queue 1024` ~~(taşan teslim SAYILIR, sessiz düşmez)~~ **[<ONAY-TARİHİ> — K4G-05c]** (dolu → birim kanala girmez ve ACK'lenmez, `full_refusals` sayılır; teslim düşmez, `delivery_dropped` yapısal 0) |
```

---

## Ek B — değişecek eski testler (DUR-8 biçimi)

Ortak: dördünde de değişen fikstür değil, kusurlu davranışı sabitleyen **iddiadır**. Bu yüzden 2×2'nin dördüncü hücresi ("eski kod + yeni test") KIRMIZI beklenir: yeni iddia kusuru yakalar. DUR-8 (a)'nın 4. koşulu burada tersine işler; dayanak C9 §3 ("eski testler … DUR-8 biçiminde beyanlı") ve DUR-18. PR'da her hücre çıktısıyla verilir; mutasyon koşusu yeni iddiaların 3.1/3.2 mutantlarını öldürdüğünü gösterir.

| Test | eski kod + eski test | yeni kod + eski test | yeni kod + yeni test | eski kod + yeni test |
|---|---|---|---|---|
| B.1 | yeşil | KIRMIZI | yeşil | KIRMIZI (beklenen) |
| B.2 | yeşil | KIRMIZI | yeşil | KIRMIZI (beklenen) |
| B.3 | yeşil | KIRMIZI | yeşil | KIRMIZI (beklenen) |
| B.4 | yeşil | KIRMIZI | yeşil | KIRMIZI (beklenen) |

**B.1 `crates/client-core/src/transport.rs:1323-1339` `reorder_is_bounded_and_advances_past_a_gap_only_over_cap`.** Yeşil, çünkü kod `held.len() > cap` olunca `next`'i ilk tutulana atlatır (:386-390) ve test bu atlamayı iddia eder (`[1, 2, 3, 4]`, "tavan aşılınca en küçük boşluk atlanır"). Düzeltmeyle kırmızı: atlama dalı kalkar, 4 pencere dışında kalır (4 − 0 > 3), düşer ve `out` boş kalır. Fikstür aynı kalır (cap 3; 1, 2, 3 sonra 4). Yeni iddia: 4'ten sonra `out` boş, `held ≤ 3`, `push` sonucu pencere-dışı. Eksik birimin sonradan gelişi P0 R1 testindedir (`…, 0, 4` → `[0, 1, 2, 3, 4]`). Yeni ad: `reorder_is_bounded_and_never_skips_a_gap`. Eski kod + yeni test: `out = [1, 2, 3, 4]` → kırmızı.

**B.2 `crates/client-core/tests/runtime_e2e.rs:255-305` `delivery_overflow_is_counted_not_grown`.** Yeşil, çünkü kanal (4) doluyken birim düşer ama ACK'lenir (:288 `acked == 40`, "taşma ACK yolunu ETKİLEMEMELİ"), `delivery_dropped > 0` (:297-300). Düzeltmeyle kırmızı: dolu kanalda birim ACK'siz reddedilir, durakta en çok 4 teyit gelir; :288 ve :298 düşer. Fikstür aynı kalır (kanal 4, 40 birim, önce durak sonra drenaj). Yeni iddia: durakta teyit ≤ 4 ve `full_refusals > 0` ("bellek büyümez" korunur: alıcı en çok 4 payload tutar); drenajdan sonra 40 birimin hepsi tam bir kez teslim, `delivery_dropped == 0`, `delivered == 40`. Yeni ad: `delivery_overflow_refuses_without_ack_not_grown`. Eski kod + yeni test: durakta 40 teyit, 36 düşüş → kırmızı.

**B.3 `crates/client-core/src/runtime.rs:3334-3372` `live_value_equals_the_overflow_counter_before_shutdown`.** Yeşil, çünkü `TINY_QUEUE` = 1 ve 16 birimle 15 düşüş hem canlı aynada hem kapanışta görünür. Düzeltmeyle kırmızı: düşüş olmaz, `wait_until(dropped == 15)` 20 s sonunda başarısız olur. Ayna mekanizması (döngü başı yayın, :2746-2748) aynı kalır. Yeni iddia: taşma koşulunun gerçekleştiğini canlı `full_refusals > 0` gösterir (boş geçiş yok); `delivery_dropped()` canlı ve kapanışta 0; drenajdan sonra `delivered == 16`. Yeni ad: `overflow_is_refused_and_drop_mirror_stays_zero`. Eski kod + yeni test: canlı değer 15 → kırmızı.

**B.4 `crates/kama-node/src/node/tests.rs:415-456` `delivery_channel_overflow_reaches_node_metrics`.** Yeşil, çünkü Bob sürülmezken PTT demetleri ACK'lenip kanaldan düşer; önce `delivery_dropped() > 0`, sonra `metrics().delivery_channel_drops > 0` olur. Düzeltmeyle kırmızı: düşüş yapısal 0 olduğundan iki `until` de zaman aşımına uğrar. Fikstür aynı kalır (Bob elle sürülür ve durur; Alice demet basar). Bekleme koşulu ve iddia değişir: Alice `Backpressure` alana ya da Bob'un `ctx_dispatch_stats().full_refusals > 0` olana dek basar. Bob ürün döngüsüne alınınca Alice'in kabul edilmiş (`Ok`) her demeti Bob'a sırayla ve tam bir kez ulaşır, `metrics().delivery_channel_drops == 0` kalır. Yeni ad: `delivery_channel_full_refuses_and_node_metrics_stay_zero`. Eski kod + yeni test: demetler düşer, sıra eksik → kırmızı.

**B.5 doc `crates/client-core/src/transport.rs:564-567`.** "sıfırdan büyükse ACK'lenmiş birim uygulamaya ulaşmadan düşmüştür (app drenaj sözleşmesi ihlali)" cümlesinin yerine: "K4G-05c'den sonra yapısal 0: kanal doluyken veri alıcısı birimi ACK'siz reddeder (`ctx_dispatch_stats().full_refusals`). Sıfırdan büyük değer kabul kararının atlandığını gösterir (hata)." Aynı düzeltme `runtime.rs:631` ve `:2260-2266`'ya da uygulanır. `kama-node/src/event.rs:209-211` dokunulmaz ("INT-02'de 0 olmalı" doğru kalır).

---

## Ek C — DUR adayları

| # | Konu | Seçenekler | Öneri |
|---|---|---|---|
| C-1 | `Receiver`'a kabul girdisinin biçimi (kart "Nereye"de `receiver.rs` kabul girdisi der) | (a) çağrı = kabul; `receiver.rs` yalnız doc · (b) `on_datagram(.., Admission)` · (c) tentative/commit | (a). Gerekçe Ek D.1 |
| C-2 | Pay (kart: "kopya işi sınırı (3) × …") | (a) pay = 0, türetimle · (b) sahip pozitif bir pay ister; sayının türetimi sahipten gelir | (a). Ek D.3 |
| C-3 | INT-02 `ek-tuketici-durak` tarifi: 50 TEXT 1024'lük kanalı dolduramaz | (a) durak boyunca eşzamanlı ≥ 2 MiB dosya (≥ 2 070 birim, birim başına ≤ 1 013 B); profil 30 s'de ≥ 1 025 birim taşımalı (≈ 35 birim/s ≈ 0,3 Mbit/s); geçerlilik `full_refusals > 0` · (b) yalnız 50 TEXT: kol boş geçer | (a) |
| C-4 | engine-spec §14:970 `rx_reorder_pool … fazla out-of-order → en eski drop (SACK ile kurtarılır)` ↔ kart "en yeni pencere-dışı ACK'siz düşer" | (a) spec dokunulmaz: S1 `Reorder` motor havuzu değildir (motor payload tutmaz); "SACK ile kurtarılır" ancak düşen birim ACK'lenmemişse doğrudur ve 3.1 bunu sağlar; notta beyan · (b) engine §14'e YAMA (plan:29 "spec değişir" kuralı) | (a) |
| C-5 | plan:392 (i) aynı beyanlı sınırı KATMAN 6 listesinde tekrarlar | (a) A.3 ile aynı YAMA KAYDI · (b) yalnız :532 | (a) |
| C-6 | Yeni sayaçların `NodeMetrics`/UniFFI yüzeyine girmesi | (a) girmez; INT-02 `ClientTransport::ctx_dispatch_stats()`'i #136 teşhis yazıcısından okur · (b) `NodeMetrics`'e 3 alan (K4G-15 `poc-uniffi-v0` yüzeyi değişir) | (a) |
| C-7 | K_MAP'in client-core'dan erişimi | (a) 05b sabiti `pub` (05b PR'ında) · (b) client-core'da kopya sabit: magic-constant yasağı → reddedilir | (a); `pub` değilse DUR |
| DUR-23 (b) | Sızıntı bekçisi için test-util kancası | ne: salt-okur `(seq_to_unit girdisi, Σ unit_seqs)`; nerede: `runtime.rs` `ClientHandle::unit_index_len_for_test`, `client_task` döngü başı yazımı; `#[cfg(any(test, feature = "test-util"))]`; ürün derlemesinde alan yok | (b) |

---

## Ek D — gerekçeler

**D.1 Receiver girdisi.** Seçim (a): kabul kararı `Receiver`'dan önce verilir ve `on_datagram` çağrısı kararın kendisidir. Reddedilen datagram bugünkü INV-ACK-DELIVER-1 mekanizmasındaki gibi (`NotReady`, `runtime.rs:2805-2806`, :2821-2824) motor için hiç gelmemiştir; `seen_count` gönderene ileri-yön kaybı bildirir, bu da gönderenin bakışından doğrudur. Tutarlılık şarta bağlıdır: PN tekrarı ⇒ birim kabul edilmiş, çünkü PN yalnız Accept/Duplicate'te kaydedilir ve `AcceptState` unutmaz. Bu yüzden yinelenen PN'e Refuse kararı çıkmaz. (b) reddi `seen_total`'a katar; gönderenin dönüş-yolu gözlemi (:1714-1722) "görüldü, ACK yok" durumunu dönüş bozulması sanabilir. (c) ACK üretimini (`tick_ack`) commit'e taşır, motor API'si ikiye bölünür ve `tv_*` riske girer. (b) ve (c) motora (`adaptive-transport`; ADR-17 belirlenimi, `tv_*`) mantık sokar; (a) tel, ACK_FRAME_V2 ve motor davranışını değiştirmez.

**D.2 Ayrılı yer ve `held ≤ cap`.** `above ⊆ (floor, floor+cap]` olduğundan floor konumu hiçbir sırasız birimce tüketilemez. Sıradaki eksik birim (u = floor) pencere nedeniyle reddedilemez, yalnız paylaşımlı kanal doluysa reddedilir. Kanal yalnız adaptör onu boşaltmazken dolar (düğüm durağı, `inbox ≥ MAX_INBOX` ya da iki `drive` arası). İkisi de uygulamanın tüketimine bağlıdır, eksik birime değil: inbox yalnız sıralı birim tutar. Döngüsel bekleme yoktur. Adaptörde: tutulan her u, kabul anında u ≤ floor_t + cap'i sağlar. floor_t altındaki birimler FIFO kanalda u'dan önce yer alır ve Reorder onları atlamadan çıkarır, dolayısıyla `Reorder.next ≥ floor_t`. Buradan `held ⊆ (next, next+cap]`, yani `|held| ≤ cap` (P0 R1'in `held ≤ CAP` iddiası) çıkar. Oturum başına payload ≤ cap + 1 birim; kanal (1024) ve inbox (`MAX_INBOX` 1024) düğüm geneli sınırdır.

**D.3 Pay türetimi.** 05c'den sonra teyit yalnız ACK ve RECOVERED'dan gelir; ikisi yalnız Accept/Duplicate'ten doğar; `store` yalnız bu iki yolla silinir (`runtime.rs:1687`, :1737). Yani teyitli ⇒ kabul edilmiş. Kabul edilmemiş en küçük birim (floor) teyitsizdir, buradan O ≤ floor çıkar. Gönderilen her u ≤ en yeni ≤ O + cap − 1 ≤ floor + cap − 1 < floor + cap olur, u pencere içindedir. ARQ, kopya ve prob mevcut kimlikleri taşır; parite kimliği ESI'dir (ayrı uzay). Kopya işi sınırı (3) kimlik mesafesini büyütmez. ACK gecikmesi O'yu yalnız küçültür (gönderen daha tutucu olur). Gereken pay 0'dır; pozitif bir pay ölçülmemiş yeni sayı olurdu. Parçalı kipte c − 1 bir pay değil, mesajın son chunk'ının kimliğidir.

**D.4 Kabul durumunun kapsamı.** Birim-kimliği uzayı oturumundur: istemci her oturumda 0'dan başlar (`runtime.rs:1391`). Yuva anahtarlı durum üç durumda pencereyi 0'a çeker ve oturumu kilitler: `idle_hold` (105 s) emekliliği, adres değişimi (NAT) ve ek yuva (`runtime.rs:754-756`). Kilitlenme INV-NO-IDLE-TEARDOWN'a aykırıdır; bu yüzden S1 kipinde durum oturum anahtarlıdır, `retire_closed_slots` (:2509) ile birlikte silinir ve oturum tablosuyla sınırlıdır. Örtük kipte (IdentitySeam, tek tutamaç `IMPLICIT_SESSION`) her yuva ayrı istemcidir ve kimlik uzayları çakışır. Orada durum yuva başına kalır, `delivered_seen` bugünkü ömrünü korur. Aşağı akışta Reorder olmadığı için pencere uygulanmaz; değişen yalnız dedup'un commit'te ilerlemesi ve dolu kanalda ACK'siz rettir (D.7-5). Kip `bind_with_policy`/`bind_expecting` iç kurucusuna özel parametreyle geçer; pub imza değişmez.

**D.5 Ayrıntılar.**
- **FEC RECOVERED** (`fec_ingest`, `runtime.rs:2571-2697`): kurtarılan her birim parse edilir (:2651-2658), sonra `admit`'ten geçer. ESI `recovered_esis`'e yalnız Accept/Duplicate'te girer; bugün parse'tan önce giriyor (:2649). Refuse'da o ESI için RECOVERED gitmez, gönderen birimi ARQ ile yollar; blok yine düşülür (:2676). Reddedilen DATA'nın sembolü `BlockAsm`'e girmez; Duplicate'inki bugünkü gibi girer.
- **Parçalı mesaj** (yalnız runtime; S1 `FragmenterOn` ile reddeder, `transport.rs:482-483`): yer = MsgAsm yeri. `MsgAsm::insert` (:2328-2346) yan etkisiz ön-denetim ve commit olarak ikiye bölünür; bugün `total_len` ve `buffered` ret kararından önce yazılıyor. Yeni mesaj ∧ `msgs.len() = MSG_ASM_CAP` → Refuse(Full); tahliye (:2452-2455) kalkar, `msg_asm_evicted` yapısal 0 olur. Sınır ön-denetimi başarısızsa → Refuse; `frag_chunk_dropped` artık ACK'siz reddi sayar. Mesajı tamamlayan chunk kanal izni de ister.
- **Sayaçlar:** `CtxDispatchStats` (`runtime.rs:815-826`) şu alanları kazanır: `full_refusals`, `reorder_skip`, `distance_holds`. Hepsi canlı ve düğüm genelidir (`not_ready_drops` ailesi). `ServerStats`/`ClientStats`'a alan eklenmediği için `api_surface_v0` pini değişmez (emsal `runtime.rs:637-639`). Mekanik literal güncellemeleri: `runtime.rs:3261-3265`, `ctx_first_dispatch.rs:220-226`.
- **Sızıntı:** bakım `UnitSeqIndex` adlı tek iç tipe taşınır (`record(seq, unit, in_store)`, `confirm(unit)`, `len()`); ekleme :1468-1470'ten, budama :1687-1693 ve :1737-1744'ten buraya gelir. K_MAP budaması 05b madde 11 ile aynı ufku kullanır: ufuk dışı geç ACK birimi teyit etmez. Kanca: `client_task` döngü başında `(seq_to_unit girdi sayısı, Σ unit_seqs)` değerini `Arc<[AtomicUsize; 2]>`'ye yazar (ayna deseni :2746-2748); yalnız `#[cfg(any(test, feature = "test-util"))]` altında.
- **Gönderen:** `store` HashMap'ten BTreeMap'e geçer (:1380); O = `first_key_value`, O(log n). Koşul :1785'teki `store.len() < cap`'in yanına eklenir. `distance_holds`, kapının offer'ı tuttuğu dönemleri sayar.
- **Kip:** `bind_with_policy` (:2157) iç kurucuya yuva kapsamıyla, `bind_expecting` (:2187) oturum kapsamıyla girer. Oturum durumu `retire_closed_slots` (:2509) ile birlikte silinir.
- **Doc düzeltmeleri:** `runtime.rs` :631, :758-764, :1381, :1740, :2260-2266, :2279-2283, :2364; `transport.rs` :69-73 (INV kapsamı genişler), :372-373, :564-567 (Ek B.5); `receiver.rs:192` (ön koşul cümlesi).

**D.6 Yeni testler ve mutasyon.**
- `fec_recovered_refused_when_full_is_not_confirmed` (`runtime_e2e.rs` `fec_seam` deseni; statik r; alıcı kanalı 1; tüketici durur): hedef birimin **yalnız ilk** DATA emisyonu düşer. Kalıcı erasure kullanılmaz, çünkü reddedilen kurtarımı ARQ tamamlamalıdır (D.7-6). İddialar: RECOVERED yalnız kabul edilen birimi teyit eder; drenajdan sonra hepsi teslim edilir, kopya yoktur. Tabanda kurtarılan birim RECOVERED ile teyit edilip kanalda düştüğü için kırmızı beklenir.
- `frag_messages_refused_not_evicted` (`max_message_bytes` açık; `MSG_ASM_CAP` + 1 eşzamanlı çok parçalı mesaj; dolu kanal): `msg_asm_evicted = 0`; her mesaj bütün olarak ve tam bir kez teslim edilir.
- `unit_index_is_empty_after_confirm`: KAYNAK fikstürünün runtime eşleniği (geç orijinal ACK ile teyit, ardından teyit sonrası karar). Store boşken kanca `(0, 0)` döner. 3439fccd'de kırmızı beklenir; 05b tabanında kaynak kurursa yeşil kalabilir, bu durum beyan edilir. Yanında `UnitSeqIndex` birim testleri: store dışındaki birimin kararı eklenmez; birim başına K_MAP'i aşan kayıtta en eskisi budanır; teyitte tam temizlik olur.
- `server_control_path_runs_while_delivery_stalled`: kanal ≥ `ACK_CADENCE_FLOOR` (≈ 8,4 s) boyunca dolu kalır; alıcının ACK emisyonu sürer, `mark_t0` döner.
- `sender_distance_gate_backpressures`: küçük `max_inflight_units`, birim 0 kalıcı düşer. `send` → `Backpressure` döner, `distance_holds > 0` olur, alıcıda `reorder_skip = 0` kalır.
- `receiver_window_refuses_out_of_window_without_ack`: gönderen cap > alıcı cap, birim 0 geçici düşer. Pencere dışı birimler ACK'siz kalır, `reorder_skip > 0` olur; birim 0 gelince hepsi teslim edilir, kayıp yoktur.
- **Mutasyon** (ağaç dışı): Refuse'u `Receiver`'a besle · dedup'u izinden önce ilerlet · ESI'yi kabulden önce ekle · pencere karşılaştırmasında ±1 ya da kaldır · Reorder atlamasını geri koy · gönderen mesafe koşulunu kaldır · `contains_key` koşulunu kaldır · koşullu budamayı geri koy · K_MAP budamasını kaldır · MsgAsm tahliyesini geri koy. Her mutant en az bir testte kırmızı olmalı.

**D.7 Riskler.**
1. Duraklama gönderene kayıp gibi görünür (SACK boşluğu): ARQ/kopya/parite bandı harcanır, `loss_ppm` şişer, FEC kolu açılabilir; hız düşmez (INV-RATE-1). INT-02 kolunda retx/dup/fec bayt raporlanır, hüküm verilmez. Tel düzeyinde alıcı penceresi kapsam dışıdır (FROZEN tel).
2. Mesafe kapısı yalnız en eski teyitsiz birim ≥ 4 095 birim gerideyken ateşler [çıkarım]. Bench/T-hücre davranışını değiştirmediği `distance_holds = 0` ile raporlanır.
3. 05b `confirmed_units` sonrası teyit sonrası bayat karar e2e üretilemeyebilir; bekçinin ayırt ediciliği o zaman `UnitSeqIndex` birim testine dayanır.
4. Paylaşımlı teslim kanalı (1024): bir oturumun seli, adaptör boşaltana dek diğer oturumun sıradaki birimini bekletebilir. Etki geçicidir; her `drive` kanalı boşaltır.
5. Örtük kip (bench, yalnız runtime): pencere yoktur. Değişen tek şey dedup'un commit'te ilerlemesi ve dolu kanalda ACK'siz rettir; bench kanalı sürekli boşalttığı için ateşlemez [çıkarım].
6. Reddedilen FEC kurtarımı: blok düşülür (:2676), birim ARQ ile gelir. Kalıcı erasure + dolu kanal birleşiminde birim ancak sonraki bir kurtarımla gelebilir [tasarım sınırı].
