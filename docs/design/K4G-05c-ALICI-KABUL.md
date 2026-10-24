# K4G-05c — Alıcı kabul bütünlüğü (R1 + R2 + budama) — tasarım notu

## 0. Künye
**K4G-05c** (CEVAP 9 §3) · **rev3** = rev2 (`18e6de95`) + CEVAP 11 / 11-EK / 12 / 13 (öncelik 13 > 12 > 11-EK > 11 > 10). Onay C12 §1 ile geçerli; rev3 SHA'sı ve Ek A hash'i ayrıca istenir. Taban main `1b1425d2` + 05b. Satır atıfları `1b1425d2`'dendir; 05b'ye bağlı atıflar `05b-int:` önekiyle `k4g/05b-int` @`20f1df53`'ten. P0: `k4g/p0-taban` @`df7c9e62`. **TASLAK — kod 05b merge'ünü bekler;** ölçülmedi. Tarihçe Ek E'de (MUST NOT IMPLEMENT).

## 1. Değişmez
> Alıcının tutamadığı Reliable birim ACK'lenmez (SACK'te de, FEC RECOVERED'da da); ACK'lenmiş Reliable birim, oturum `Closed` olana dek uygulamaya eksiksiz, tam bir kez ve gönderim sırasıyla teslim edilir.

`Closed` anında kuyrukta kalan ACK'li birimler beyanlı sınır (ii)'de kalır (plan:392); DUR-18 yalnız (i)'yi kaldırdı.

## 2. Mekanizma (bugün; atıflar D.0)
- **R2:** ACK kabulden bağımsız gider; dolu kanalda ACK'li payload düşer. `p0_r2_full_channel_never_acks_undelivered`, `p0_r2_dropped_unit_blocks_followers`.
- **R1:** Reorder eksik birimi atlar; gönderen mesafeyi sınırlamaz. `p0_r1_reorder_never_skips_missing_reliable`.
- **Kayıt/payload:** v1'de runtime, motorun teyit etmediği geç ACK'te de payload'u siler; envanterde 1–3, 5, 7–8. satırlar (ENVANTER.md). Bu, C13 §3 gereği **05b'de** kapanır (05b-int `drain_confirmed!` :2078-2099). 05c'ye kalan: K_MAP budamasının açılması ve birim başına O(1) runtime kaydı.

## 3. Tasarım [tasarım]
**3.1 R2 — tek kabul kararı** (`server_task`). S1 kipinde oturum başına `AcceptState {floor, above ⊆ (floor, floor+cap]}` tutulur (D.4). Yan etkisiz `admit(u)`:
- `u < floor ∨ u ∈ above` → **Duplicate**: ACK'lenir, payload atılır.
- `u − floor > cap` → **Refuse(Window)**, `reorder_skip`.
- Kanal kuralı tutmazsa → **Refuse(Full)**, `full_refusals`.
- Aksi → **Accept**.

Refuse'da datagram `Receiver`'a, `BlockAsm`'e ve kanala girmez (`NotReady` deseni): SACK boşluğu kalır, gönderen yeniden yollar. Accept'te `await`'siz sıra: `on_datagram` → `permit.send` → `AcceptState`. `on_datagram` birimi döndürmezse izin bırakılır, karar Duplicate olur. Dördü birlikte ya da hiçbiri; `delivery_dropped` yapısal 0.

- **Ayrılı yer:** R = min(`max_peers`, C − 1) izinlik sıra başı rezervi ayrılır (C = `delivery_queue`). Floor birimi rezervi boşsa rezervden alır; diğer kabuller rezervi boş oturumların payını (R − U) korumak zorundadır. Kota: kanalda ≤ C − R + 1, Reorder'da ≤ cap. Döngüsel bekleme yoktur (D.2); adalet sözü yoktur (DUR-05c-8 (a)).
- **Oturum geçişi:** yenilemede ya da supersession'da `h` (yalnız-alım drain) ve `h'` iki ayrı oturumdur, her biri kendi rezervini tutar. U ≤ R ve I (F ≥ R − U) korunur; ikisinin de sıra başı garantisi sürer (test D.5).
- **Receiver girdisi = çağrının kendisi** (DUR-05c-1 (a′)). `seen_count` spec yaması bu PR'da (Ek A.5).
- **FEC RECOVERED:** ESI listeye yalnız Accept/Duplicate'te girer; üç RECOVERED çıkışı da bu kurala uyar (`runtime.rs:3649`, `:3734`, `:3799`). İki kol için 3.6'ya bakın.

**3.2 R1.** `u − next > cap` → birim düşer + `reorder_window_violations` (S1'de yapısal 0); atlama kalkar. Gönderende kapı `next_unit + c − 1 − O < cap`; tutmazsa `Backpressure` + `distance_holds`. Pay = 0 (DUR-05c-2 (a), D.3).

**3.3 Kayıt ve payload sahipliği.** **05b'de (C13 §3):** payload ve `unit_acked!` yalnız `Sender::take_confirmed_units()` çıktısıyla silinir; teyit üretmeyen geç ACK'te payload kalır (05b-int `runtime.rs:2078-2124`, testler `k4g05b_runtime_ownership.rs`). **05c'de kalan (DUR-30):**
- (i) Motor K_MAP budaması açılır: 05b-int `sender.rs` `own_on_sent` :6330, `K_MAP` :1644. Mekanizma `87b4cf49`'daki gibi, 4fec0556'da kapatılmıştı. Hangi kaydın budanacağı DUR-05c-14'tür (3.4).
- (ii) Runtime'da birim başına O(1) kayıt: `seq_to_unit` kalkar, birim başına liste yerine `UnitTxLog {emisyon sayısı, son seq}` gelir. Yalnız store'daki birim için tutulur ve teyit çıktısıyla düşer → girdi ≤ store.len().
- (iii) `late_ack_after_prune_keeps_payload` yeşil şarttır: budanmış seq'in geç ACK'i teyit etmez, payload yerinde kalır, yeniden gönderim gövdeli gider.
- Kanca: DUR-23 (b) **05b'de** karşılandı (`SenderRecoveryDiag::{seq_to_unit_len, unit_seqs_len}`, 05b-int `runtime.rs:819-820`, `live_diag`). 05c'de `seq_to_unit_len` yapısal 0 olur, `unit_seqs_len` = `UnitTxLog` girdisi; ek kanca yok.

**3.4 Budama gerilemesi → DUR-05c-14.** Budama açılınca iki mevcut test kırmızıya döner: `dr_cs1_acceptance::f3_di12ad_equal_timestamp_ordering` (C/D kolu ResolvedByDataAck → TimedOut) ve `ek4_a2_liveness::external_pto_drives_loss_declaration_and_differs_from_empty_batch` (v1 ARQ yolu kaybolur). Budama kapalıyken ikisi test değişmeden yeşildir (BUDAMA-UYARISI.md). Ortak desen: budama birimin **orijinal Data kaydını** kuruluş kopyaları yüzünden düşürüyor. C13 §4'e göre bu, kasıt kanıtlanmadıkça gerileme şüphesidir. Seçenekler Ek C'de; öneri **(A)**: orijinal Data kaydı budanmaz, budama orijinal dışındaki en eski kaydı seçer, sınır ≤ K_MAP aynı kalır. Her seçenekte iki test **değişmeden** yeşil şarttır (C hariç).

**3.5 Parçalı mesaj → DUR-05c-12 (yeniden yazıldı; öneri A).** S1 yolunda transport parçalaması yapısal olarak kapalıdır: `ClientTransport::build` → `FragmenterOn` (`transport.rs:498-500`; ürün profili `runtime.rs:266` `None`). Ürün yolundaki tek parçalı mesaj dosyadır ve uygulama katmanında reddedilir (ANALIZ.md §2). `MsgAsm` yalnız S1-dışı bench/test yoludur. Öneri A, iki teste dayanır: `FragmenterOn` kilit testi (kapı bugün testsiz) ve dosya davranış testi; notta beyan. B ayrı karttır, sahip isterse.

**3.6 FEC dolu kanalda iki kol (11-EK §5, C13 §7).**
- **Kol 1 (zorunlu):** kaynak yeniden gönderimi sonradan ulaşır → ARQ ile tamamlanır (test D.5).
- **Kol 2:** kaynak kaybolmaya devam eder, onarım yeterlidir; FEC üzerinden tamamlanma **PoC garantisi değildir**. 05c'de reddedilen kurtarımın bloğu düşer (`runtime.rs:3321`), kalıcı kaynak kaybında birim gelmez [tasarım çıkarımı]. KATMAN 6 beyanlı sınırı: "dolu kanalda FEC kurtarımı kaybolabilir; ARQ telafi eder; kaynak kalıcı kayıpsa oturum o birimde bekler". Kol 2 denenir ve raporlanır; tek başarılı koşu sınırı kaldırmaz. "FEC altında dayanıklılık sağlandı" denmez.

## 4. Dosyalar · YAMA · kimlik
- **Kod:** `crates/client-core/src/{runtime.rs, transport.rs}`; `crates/adaptive-transport/src/sender.rs` (yalnız K_MAP budaması, DUR-05c-14 seçeneğiyle); `crates/adaptive-transport/src/receiver.rs` yalnız doc.
- **Test:**
  - `crates/client-core/tests/{runtime_e2e.rs, k4g_p0_receiver.rs (P0), ctx_first_dispatch.rs (literal), session_keyed_slots.rs (doc), k4g05b_runtime_ownership.rs (+ `late_ack_after_prune_keeps_payload`)}`
  - `crates/adaptive-transport/tests/k4g05b_ownership.rs` (taşınan iddialar, §5)
  - `crates/kama-node/src/node/tests.rs`
  - `crates/kama-integration/tests/seam_real_adapter/main.rs` (`FragmenterOn` kilidi)
  - kama-app dosya davranış testi (DUR-05c-12 A)
  - `dr_cs1_acceptance.rs` ve `ek4_a2_liveness.rs` yalnız DUR-05c-14 (C) seçilirse değişir.
- **Belge:** Ek A (A.1–A.5).
- **Dokunulmaz:** `s1.rs`, `s3.rs`, `handles.rs`, protector, tel/codec/`tv_*`, UniFFI, kama-app ürün kodu, `api_surface_v0`, kimlik/freeze ve FROZEN dosyalar. INT-02 düzeneği K4G-05e'dedir.
- **Çakışma (C9 §6):** `runtime.rs`/`transport.rs` 05c bitene dek A şeridine kapalı; A3 bu PR'ı bekler.
- **Kimlik:** 05b'nin `kama-v1.1-full` beyanına 05c satırı (DUR-05c-11 (a)).
- **YAMA hedefleri:** **A.1** KM-SEAM-AT-001 §4 (sıralı teslim sözü `:151`) · A.2 plan:532 · A.3 plan:392 · A.4 HANDOFF:20 · **A.5** engine-spec §2.2 `seen_count` (main :216 / 05b-int :220).

## 5. Eski ve taşınan testler
- **Eski testler (Ek B):** `transport.rs:1412-1428`, `runtime_e2e.rs:255-305` (:288), `runtime.rs:3976-4017`, `kama-node/src/node/tests.rs:415-456`. İddia değişir; 2×2'nin 4. hücresi derlenir ve KIRMIZI beklenir.
- **05b'den taşınanlar** (`k4g05c/05b-den-tasinan/`; 05c kapısında aktif, `#[ignore]` yok; davranışın neden 05c'de olduğu):
  - `k4g05b_kmap_bound_and_pruned_late_ack` — sınadığı budama ve budanmış kaydın geç ACK'i yalnız 05c'de var, çünkü budama 05c'de açılır.
  - D1 kol 1 (`…_kmap_live_records`) — "canlı kayıt ≤ K_MAP" iddiası budama olmadan sağlanamaz; budama 05c'de.
  - D2 kol 3 (`…_kmap_live_records`) — aynı neden: sınırı budama kurar ve budama 05c'de.
  - `k4g05b_accept_p0_mirror_horizon_out_declared_red` — budanmış-kayıt kolu 05c'de `late_ack_after_prune_keeps_payload` olarak gerçek runtime'da koşar. TTL kolu 05b'de etkin bir davranıştır ve orada kalır (05b-int `late_ack_after_ttl_keeps_payload` :775). Çelişki notu Ek C'de.
- **Budamaya bağlı iki mevcut test** (3.4) DUR-05c-14 (A) altında değişmeden yeşil şarttır.

## 6. Kanıt
- P0 üçlüsü 05c tabanında (main + 05b) kırmızı (yeşilse DUR) → yeşil.
- `late_ack_after_prune_keeps_payload`, taşınan dört iddia ve iki budama testi yeşil.
- `unit_records_bounded_per_unit`: her örnekte birim kaydı ≤ store.len() ve `seq_to_unit_len` = 0. Tabanda kırmızı beklenir: 05b'de birim başına liste yeniden gönderimle büyür.
- Mevcut testler + `tv_*` yeşil.
- Yeni testler (D.5): FEC kol 1 · oturum geçişinde rezerv · kanal rezervi · kontrol yolu · gönderen mesafe · pencere ihlali · `FragmenterOn` kilidi · dosya davranışı.
- **INT-02 `ek-tuketici-durak`** (DUR-05c-3 kararı; düzenek K4G-05e):
  - Durak 30 s; eşzamanlı dosya ≥ 1 025 birim.
  - Taban kolunda (05c'siz) `delivery_channel_drops > 0`; geçerlilik `full_refusals > 0`.
  - Hüküm 3 tohumda 3/3: teslim %100, `delivery_channel_drops = 0`, `reorder_skip = 0`, dedup doğru, kontrol yolu çalıştı. Kontrol yolu = ACK; heartbeat PoC'de yok.
  - Bütün kollarda `reorder_window_violations = 0` ve `confirmed_units_overflow = 0`.
  - Ayrıca öneri: 31–100 B/s'de tek dosya ölçümü (DUR-05c-12 olgu b).

## 7. Sıra · süre · risk
- **Sıra:** 05b merge → rebase → P0 → kırmızılar → kod → mutasyon → kapılar → INT-02.
- **Süre:** `[SÜRE-BEKLİYOR]`.
- **Riskler (D.6):**
  - Duraklama gönderene kayıp gibi görünür (hız düşmez).
  - Rezerv R izni paylaşımlı havuzdan alır.
  - FEC kol 2 sınırı.
  - DUR-05c-14 (A)'da orijinal kayıt uçuş muhasebesinde kalır [çıkarım].
  - Örtük kipte pencere ve rezerv yok.

---

## Ek A — yama metinleri

Biçim: üstü çiz, silme (`~~eski~~` + yeni) + tarihli YAMA KAYDI; `scripts/ci/word_coverage.py` kapısı yeşil olmalı (bu revizyonda koşulmadı). `<ONAY-TARİHİ>` = "K4G-05c ONAYLANDI" tarihi; `<DUR-05c-5 CEVAP TARİHİ>` = DUR-05c-5 kararının tarihi. A.2'deki 2026-10-24 sahibin DUR-18 tarihidir.

### A.1 — `docs/seams/KM-SEAM-AT-001.md`

(i) Satır 8'in ardına, 2026-10-08 kaydının önüne (en yeni kayıt en üstte):

> **YAMA KAYDI — <ONAY-TARİHİ> [K4G-05c alıcı kabul bütünlüğü — Abkarada kararı CEVAP PAKETİ 9, 2026-10-24, DUR-16 (b) / DUR-18 — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §4'e, `recv()` sıralama maddesinin ardına **teslim bütünlüğü (INV-ACK-DELIVER-1)** maddesi, §6'ya **H-ACK-DELIVER** satırı eklendi: alıcının tutamadığı `Reliable` birim ACK'lenmez (SACK ve FEC RECOVERED dahil); ACK'lenmiş `Reliable` birim, oturum `Closed` olana dek eksiksiz, tam bir kez ve gönderim sırasıyla teslim edilir; gelen kutusu oturum başına sınırlıdır ve her oturumun sıradaki eksik birimine teslim kanalında yer ayrılıdır; gönderende birim-kimliği mesafesi `max_inflight_units`'e ulaşınca `send()` → `Backpressure`. `Closed` anında kuyrukta kalan ACK'li birimler plan KATMAN 6 beyanlı sınır (ii) olarak kalır. INV-ACK-DELIVER-1 plan Ek 2026-10-11 (3)'te yalnız `NotReady` kolu için doğdu; bu madde onu yer ve pencere kollarına genişletir. Trait bloğu, `TransportError` kümesi ve `crates/kama-contracts/src/s1.rs` DEĞİŞMEDİ (yalnız ekleme). Üstü çizili metin yok.

(ii) §4, satır 151'in (`recv()` sıralaması) ardına yeni madde:

- **[<ONAY-TARİHİ> — K4G-05c] Teslim bütünlüğü (INV-ACK-DELIVER-1):** Transport alıcıda tutamadığı `Reliable` birimi **ACK'lemez**: ne SACK'te ne FEC kurtarım teyidinde (RECOVERED). "Tutamadığı" üç durumu kapsar: oturumu hazır değil, gelen kutusunda yer yok, ya da birim oturumun kimlik penceresinin (`max_inflight_units`) dışında. Birim gönderende kalır ve yeniden gönderilir. ACK'lenmiş `Reliable` birim, oturum `Closed` olana dek `recv()`'den **eksiksiz ve tam bir kez**, yukarıdaki sırayla çıkar; sıralama tamponu eksik birimi atlamaz. Gelen kutusu oturum başına sınırlıdır ve her oturumun sıradaki eksik birimi için teslim kanalında bir yer ayrılıdır: sırasız birimler ve başka oturumlar bu yeri alamaz. `Closed` anında kuyrukta kalan ACK'li birimler düşer: plan KATMAN 6 beyanlı sınırı (ii), bu madde onu kapatmaz. Gönderen, en yeni birim ile en eski teyitsiz birim arasındaki kimlik mesafesi pencereye ulaşınca yeni birim almaz: `send()` → `Backpressure` (K9: yapısal hata; Application bekler). Bu madde zamanlayıcı ya da deadline getirmez (H10 korunur).

(iii) §6 tablosunun sonuna (satır 174'ün ardına):

```
| H-ACK-DELIVER | **[<ONAY-TARİHİ> — satır eklendi; kural §4 teslim bütünlüğü]** Dolu gelen kutusu ya da pencere dışındaki birim ACK'lenmez ve gönderende kalır; ACK'li birim oturum `Closed` olana dek tam bir kez ve sırayla teslim edilir; sıradaki eksik birim, sırasız birimler ve başka oturumlar teslim kanalını doldururken de kabul edilir. **mock: kapsam dışı (MockTransport teslim yolunu içermez) · real: client-core testleri (K4G-05c)** |
```

### A.2 — `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:532`

Eski satır:

```
| DUR-K4G-01 (#114/#116 üç beyanlı sınır) | KATMAN 6 kartı + INT-02 beyanı; (i) için `delivery_channel_drops` sayacı K4G-11 `metrics()`'e, INT-02'de 0 olmalı; (iii) epoch geçişinde ölçülür | K4G-16, K4G-05 |
```

Yeni satır:

```
| DUR-K4G-01 (#114/#116 üç beyanlı sınır) | KATMAN 6 kartı + INT-02 beyanı; ~~(i) için `delivery_channel_drops` sayacı K4G-11 `metrics()`'e, INT-02'de 0 olmalı;~~ (i) **2026-10-24: beyanlı sınır kaldırıldı → K4G-05c** [YAMA KAYDI 2026-10-24 — K4G-05c — DUR-18 CEVAP (sahip)] (`delivery_channel_drops` `metrics()`'te kalır; INT-02'de 0 şartı yapısal sağlanır); (iii) epoch geçişinde ölçülür | K4G-16, K4G-05, K4G-05c |
```

### A.3 — `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:392` (yalnız DUR-05c-5 (a) ile)

Satır içindeki parça:

```
(i) teslim kanalı taşması (`runtime.rs` ~:2321 `try_send`; INV-ACK-DELIVER-1'i ihlal edebilir → sayaç `delivery_channel_drops` K4G-11 `metrics()`'te, INT-02'de 0 olmalı)
```

yerine:

```
~~(i) teslim kanalı taşması (`runtime.rs` ~:2321 `try_send`; INV-ACK-DELIVER-1'i ihlal edebilir → sayaç `delivery_channel_drops` K4G-11 `metrics()`'te, INT-02'de 0 olmalı)~~ (i) [YAMA KAYDI <DUR-05c-5 CEVAP TARİHİ> — K4G-05c — DUR-18 + DUR-05c-5 (a) CEVAP: beyanlı sınır (i) kaldırıldı]
```

### A.4 — `docs/transport-v1/TRANSPORT-CRYPTO-HANDOFF.md` (yalnız DUR-05c-9 (a) ile)

(i) Satır 8-9'daki "Kaynak: …" paragrafının ardına:

> **YAMA KAYDI — <ONAY-TARİHİ> [K4G-05c — DUR-05c-9 (a) CEVAP]:** §10.1 "kuyruklar" satırı: dolu teslim kuyruğunda birim artık düşmez, ACK'siz reddedilir; offer kuyruğu ayrıca birim-kimliği mesafesiyle durur. Üstü çizili metin silinmedi.

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

Değişen fikstür değil, kusuru sabitleyen iddiadır. Bu yüzden DUR-8 (a)'nın (1) "iddia ve eşik aynı kaldı" koşulu ve (2)'deki 2×2'nin 4. hücresi ("eski kod + yeni test") burada tersine işler. Dayanak: B.1/B.2 için C9 §3 + DUR-18; B.3/B.4 sahibin adlandırmadığı ayna testleridir (DUR-05c-10). Yeni iddialar eski kodda derlenir: yeni alan ya da dönüş tipi kullanmazlar, `full_refusals` iddiaları ayrı yeni testlerdedir, `push` `()` döner. Böylece 4. hücre sayısal kırmızı üretir. Her test için hücreler: eski kod + eski test yeşil · yeni kod + eski test KIRMIZI · yeni kod + yeni test yeşil · eski kod + yeni test KIRMIZI (beklenen; çıktı PR'da).

| Test (yeni ad) | Neden yeşildi | Düzeltmeyle kırmızı | Yeni iddia (fikstür aynı) | Eski kod + yeni test |
|---|---|---|---|---|
| B.1 `transport.rs:1323-1339` `reorder_is_bounded_and_advances_past_a_gap_only_over_cap` (→ `…_never_skips_a_gap`) | atlama :386-390 (`[1,2,3,4]`) | 4 pencere dışı, `out` boş | `out` boş, `held.len() ≤ 3` | `out = [1,2,3,4]` |
| B.2 `runtime_e2e.rs:255-305` (→ `delivery_overflow_refuses_without_ack_not_grown`) | :288 `acked == 40`, :297-300 `delivery_dropped > 0` | durakta ≤ 2 teyit, düşüş 0 | durakta teyit ≤ kanal (4), `delivery_dropped == 0`, drenajda kanal çalışır; eksiksizlik P0'da | 40 teyit, 36 düşüş |
| B.3 `runtime.rs:3334-3372` (→ `overflow_is_refused_and_drop_mirror_stays_zero`) | 15 düşüş canlı + kapanış | `wait_until(15)` zaman aşımı | `delivery_dropped()` canlı/kapanış 0; drenajda `delivered == 16` | canlı 15 |
| B.4 `kama-node/src/node/tests.rs:415-456` (→ `delivery_channel_full_refuses_and_node_metrics_stay_zero`) | demetler ACK'lenip düşer, `delivery_channel_drops > 0` | `until` zaman aşımı | Alice `delivery_queue + 1` demet `Ok` ya da `Backpressure` alana dek basar; Bob döngüye girince PttAudio sayısı = `Ok` sayısı, `delivery_channel_drops == 0` (içerik aynı → sıra iddiası yok) | PttAudio < `Ok` |

Belge ve mekanik: B.3 aynanın canlılığını artık ayırt etmez (sayaç yapısal 0; beyan), atıf veren `transport.rs:1341-1344` belgesi güncellenir. `transport.rs:564-567`, `runtime.rs:631`, `:2260-2266`: "yapısal 0; > 0 kabul kararının atlandığını gösterir". `session_keyed_slots.rs` (e): davranış aynı. `AcceptState` `retire_closed_slots` ile AYNI adımda silinmek zorundadır, aksi hâlde sonda birim 0'ı erken yeniden teslim eder; belge "yuvanın dedup penceresi" → "oturumun kabul durumu" olur. `runtime.rs:3284` `PeerSlot::new` imza değişirse mekanik güncellenir. `kama-node/src/event.rs:209` geri çekilen DUR-K4G-01 (i)'ye atıf verir; kama-node ürün kodu bu kartta dokunulmaz → K4G-16 belge kalemi.

---

## Ek C — DUR adayları (DUR-05c-n; öneri kalın)

| DUR | Konu | Seçenekler |
|---|---|---|
| DUR-05c-1 | Kart "`Receiver`'a kabul girdisi" | **(a) çağrı = kabul; `receiver.rs` yalnız doc; `seen_count` "alıcı motorunun kabul ettiği DATA" olur, engine-spec §2:216 metni değişmez, sapma INT-02 raporunda beyan** · (a′) (a) + engine-spec §2:216 YAMA · (b) `on_datagram(.., Admission)` · (c) tentative/commit (D.1) |
| DUR-05c-2 | Pay (kart "kopya işi sınırı (3) × …") | **(a) pay = 0 (D.3)** · (b) sahibin formülü: pay = 3c (N = 3, DUR-13) → kapı `next_unit + c − 1 − O < cap − 3c`; bedel: pencere 3c daralır |
| DUR-05c-3 | **Sahibe soru:** `ek-tuketici-durak` "50 TEXT akarken" kanalı dolduramaz (50 < 1024), kol R2'yi tetiklemez | (a) durak boyunca eşzamanlı dosya ≥ `delivery_queue` + 1 = 1 025 birim (≥ 1 025 × 1 013 B ≈ 1,04 MB); profil 30 s'de bunu taşımalı (≈ 35 birim/s ≈ 0,3 Mbit/s); taban kolu (05c'siz aday, 1 tohum): `delivery_channel_drops > 0` beklenir, 0 ise DUR; 05c'li kol: `full_refusals > 0` (geçerlilik) · (b) C9 §5 metni aynen (kol boş geçer) |
| DUR-05c-4 | engine-spec §14:970 "en eski drop (SACK ile kurtarılır)" ↔ kart "en yeni pencere-dışı ACK'siz" | **(a) spec dokunulmaz: S1 `Reorder` motor havuzu değildir; "SACK ile kurtarılır" ancak birim ACK'lenmemişse doğru, §3 R2 Refuse bunu sağlar** · (b) engine §14 YAMA |
| DUR-05c-5 | plan:392 (i) aynı sınırı tekrarlar | **(a) A.3** · (b) yalnız :532 |
| DUR-05c-6 | Sayaçların INT-02'ye ulaşması | **(a) `NodeMetrics`/UniFFI değişmez; INT-02 düzeneği ayrı iş (öneri K4G-05e): `kama-node/src/probe.rs` `NodeProbe` (test-util) + `node.rs` `probe()` dört sayaç; `int02_node.rs --arm ek-tuketici-durak`; `scripts/netem/int02_{run.sh,analyze.py}` hükümleri** · (b) `NodeMetrics`'e alan (K4G-15 `poc-uniffi-v0` yüzeyi değişir) |
| DUR-05c-7 | K_MAP erişimi | **kapandı:** 05b `pub const K_MAP` (`adaptive_transport::sender`) tanımlar; runtime onu kullanmaz (silme `take_confirmed_units` ile) |
| DUR-05c-8 | Ayrılı yer: R ve adalet | **(a) R = min(`max_peers`, C − 1) (ürün 64/1024); yalnız sıra başı garantisi, paylaşımlı havuzda adalet sözü yok** · (b) ek oturum kotası ⌊(C − R)/`max_peers`⌋ = 15 (hızlı yolda yapay ret) |
| DUR-05c-9 | HANDOFF §10.1 "kuyruklar" satırı bayatlar | **(a) A.4** · (b) dokunulmaz, K4G-16 belge PR'ına |
| DUR-05c-10 | B.3/B.4 sahibin adlandırmadığı testler | **(a) Ek B'deki DUR-8 biçimiyle değişir** · (b) DUR |
| DUR-05c-11 | Kimlik/ADR | **(a) 05b'nin `kama-v1.1-full` beyanına eklenir (EVOLUTION altı beyanına 05c satırı, PR'da); T-hücre birleşik adayda** · (b) ayrı kimlik |
| DUR-23 (b) | Runtime tablo sayacı kancası (sızıntı bekçisi; sahibe DUR-23 olarak soruldu, öneri (b)) | (a) kanca yok, yalnız birim testi · **(b) kanca: `#[cfg(any(test, feature = "test-util"))] ClientHandle::unit_records_for_test() -> (seq dizini girdisi, birim kaydı)`; `client_task` döngü başında `Arc<[AtomicUsize; 2]>`'ye O(1) yazar (05c sonrası seq dizini 0, birim kaydı = `UnitTxLog` boyu); INT-02 örnek derlemesinde de etkin (test-util dev-dep)** |
| DUR-05c-12 | Parçalı mesaj kalıcı sınır ihlali (yapılandırma uyumsuzluğu, bozuk FINAL) | **(a) protokol ihlali: ACK + düşür + `frag_chunk_dropped`; değişmez "geçerli birim" için konuşur (beyan); yeniden gönderim döngüsü yok** · (b) fail-closed yuva kapatma |
| DUR-05c-13 | Sızıntı testi 05c tabanında yeşil kalırsa | **(a) bekçi DUR-20 emriyle yazılır; kanıt 3439fccd+kanca kırmızısı; nota "yanlışlandı (05c tabanında): <test>"** · (b) bekçi ertelenir |

---

## Ek D — türetimler ve atıflar

**D.0 Mekanizma satır atıfları** [kod, P0].
- **R2:** `runtime.rs:2933-2961`: PN teslimden önce kaydedilir (`receiver.rs:233`), dedup teslimden önce ilerler (:2410), dolu kanalda payload düşer (:2380-2387), ACK koşulsuz (:2957). FEC ESI'si parse ve teslimden önce listeye girer (:2649). MsgAsm reddi ve tahliyesi ACK'li chunk düşürür (:2440, :2452-2455). P0: 40 birimin 36'sı ACK'li ama teslimsiz; arkadaki 12..15 hiç teslim edilmedi.
- **R1:** `transport.rs:386-390` boşluğu atlar, :382-384 geç eksik birimi sayaçsız atar; gönderen yalnız sayıyı sınırlar (`runtime.rs:1785`). P0: cap 3, `1,2,3,4,0` → 0 atıldı.
- **Sızıntı/teyit:** `runtime.rs:1468-1470` store'a bakmadan ekler; budama yalnız `store.remove(..).is_some()` dalında (:1687-1693, :1737-1744); bayat karar :1539-1549. Runtime geç ACK'te motordan bağımsız siler (:1736-1743); 05c merge'üne dek bu ara durum 05b'de beyanlıdır (B.2.2). `Closed`: `transport.rs:996-999` kapanmış ya da bilinmeyen oturumun teslimini düşürür.

**D.1 Receiver girdisi ve `seen_count`.** (a)'da kabul kararı `Receiver`'dan önce verilir; `on_datagram` çağrısı kararın kendisidir. Reddedilen datagram bugünkü `NotReady` kolu gibi (`runtime.rs:2805-2806`, :2821-2824) motora hiç girmez. Tutarlılık: PN yalnız Accept/Duplicate'te kaydedilir ve `AcceptState` unutmaz, bu yüzden yinelenen PN ⇒ kabul edilmiş birim; yinelenen PN'e Refuse çıkmaz. Bedel: reddedilen DATA `seen_count`'a girmez (engine-spec §2:216 "görülen toplam DATA"); gönderen durakta ileri-yön kaybı görür, kendi bakışından doğrudur. `NotReady` kolunda da aynısı bugün beyansız oluyor. (b) reddi `seen_total`'a katar ve gönderenin dönüş-yolu gözlemi (:1714-1722) bunu dönüş bozulması sanabilir. (c) `tick_ack`'i commit'e taşır, motor API'si bölünür ve `tv_*` riske girer.

**D.2 Ayrılı yer ve döngüsel bekleme yokluğu.** C = `delivery_queue`, R = min(`max_peers`, C − 1), F = boş izin (`delivery_tx.capacity()`), U = rezervi kullanımda olan oturum sayısı. Oturum s'nin rezervi, rezervle kabul edilen teslimi kanaldan alınana dek kullanımdadır. Sunucu rezervli teslimin oturum içi sıra numarasını defterde tutar; tüketici `try_next_delivery`'de alımı ve oturum sayacını aynı kilit altında yapar; sunucu F ve U'yu aynı kilit altında okur (U defterde O(1)). Kural: (i) u = floor_s ve s'nin rezervi boş → kabul, rezerv kullanımda; (ii) diğer her kabul F − 1 ≥ R − U ise.
- **Değişmez I: F ≥ R − U.** Başta F = C > R. (ii) I'yı korur. (i)'de U ≤ R − 1 iken F ≥ 1 olur; kabul sonrası F − 1 ≥ R − (U + 1). Tüketim F'yi artırır; rezervli teslim alınınca U azalır, F aynı adımda artar.
- **Kapsam:** aynı anda R oturuma kadar her oturumun sıra başına yer garantidir. R oturum veri alabilen oturum tavanıdır (yuva ≤ `max_peers`). R + 1'inci oturum yalnız tüketimi bekler. Kota: s'nin kanaldaki payı ≤ (C − R) + 1, Reorder'daki payı ≤ cap. Fiziksel kanal C değişmez (freeze: `transport_v1_freeze.rs:99`). R ≤ C − 1 olduğundan paylaşımlı havuz en az 1 izindir.
- **Döngüsel bekleme yok:** floor'un kabulü üç şeye bağlıdır: (a) pencere (floor daima içinde), (b) s'nin rezervinin boşalması (yalnız adaptörün kanalı boşaltmasına bağlı), (c) F ≥ 1 (I'dan). Boşaltma yalnız adaptörün sürülmesine ve `inbox < MAX_INBOX`'a bağlıdır; inbox yalnız sıralı birim tutar ve uygulama tüketimiyle boşalır. Hiçbir adım herhangi bir oturumun eksik birimine bağlı değildir, bekleme grafiği döngüsüzdür.
- **Reorder `held ≤ cap`:** tutulan her u kabul anında u ≤ floor_t + cap'i sağlar. floor_t altındaki birimler FIFO kanalda u'dan önce yer alır ve Reorder onları atlamadan çıkarır, dolayısıyla `next ≥ floor_t` ve `held ⊆ (next, next+cap]`. Kanal (≤ C) ve inbox (`MAX_INBOX`) düğüm geneli ek sınırdır.

**D.3 Pay.** 05c'den sonra payload yalnız `take_confirmed_units` çıktısıyla silinir. Bu çıktının kaynakları yalnız canlı kayıtla eşleşen ACK ve RECOVERED'dır (05b B.2.2); ikisi de alıcıda yalnız Accept/Duplicate'ten doğar. Dolayısıyla teyitli ⇒ kabul edilmiş, buradan O ≤ floor. Gönderilen her u ≤ O + cap − 1 < floor + cap olur, u pencere içindedir. ARQ, kopya ve prob mevcut kimliği taşır; parite kimliği ESI'dir. Kopya işi sınırı (3) mesafeyi büyütmez; ACK gecikmesi O'yu yalnız küçültür. (a)'da pay gerekmez; (b) mevcut sabitten türer, yeni sayı değildir, bedeli pencere daralmasıdır.

**D.4 Kabul durumunun kapsamı.** Birim-kimliği uzayı oturumundur: istemci her oturumda 0'dan başlar (`runtime.rs:1391`). Yuva anahtarlı durum `idle_hold` (105 s) emekliliğinde, adres değişiminde ve ek yuvada (`runtime.rs:754-756`) pencereyi 0'a çeker ve oturumu kilitler (INV-NO-IDLE-TEARDOWN'a aykırı). Bu yüzden S1 kipinde durum ve rezerv defteri oturum anahtarlıdır ve `retire_closed_slots` (:2509) ile aynı adımda silinir; boşta O(1), etkinken ≤ cap. Örtük kipte (`bind`/`bind_with_policy`, tek `IMPLICIT_SESSION`, her yuva ayrı istemci) durum yuva başına `delivered_seen`'de kalır. Aşağı akışta Reorder olmadığı için pencere ve rezerv yoktur; değişen yalnız dedup'un commit'te ilerlemesi ve dolu kanalda ACK'siz rettir. Kip iç kurucuya özel parametreyle geçer; pub imza değişmez.

**D.5 Yeni testler ve dosya ayrıntısı.**
- **Testler:**
  - FEC RECOVERED: `max_peers` 1 → R = 1, kanal K − 1 = 7, hedefin yalnız ilk emisyonu düşer. Durakta hedef teyitsiz kalır, `full_refusals > 0`; drenajda 8/8. ESI mutantı burada ölür.
  - Parçalı: runtime birim testi (`MSG_ASM_CAP`'e erişir). Tahliye 0; sınır ihlali yeniden gönderim döngüsü üretmez.
  - Kanal rezervi: iki oturum. A'nın seli kanalı doldururken B'nin floor'u kabul edilir; A'nın sırasız birimi rezervi almaz.
  - Kontrol yolu: dolu kanalda ACK emisyonu ≥ 1 olana dek, üst sınırlı bekleme; `mark_t0` döner.
  - Gönderen mesafe: küçük cap, birim 0 kalıcı düşer → `Backpressure`, `distance_holds > 0`.
  - Pencere ihlali: gönderen cap > alıcı cap → `reorder_skip > 0`, kayıp yok.
  - Mutasyon listesi PR gövdesinde.
- **Dosya ayrıntısı:**
  - `CtxDispatchStats` dört alan kazanır (`full_refusals`, `reorder_skip`, `reorder_window_violations`, `distance_holds`); `ctx_first_dispatch.rs:220-226` ve `runtime.rs:3261-3265` literalleri buna göre güncellenir.
  - `session_keyed_slots.rs` (e): yalnız belge (Ek B).
  - `runtime.rs:3284` `PeerSlot::new`: imza değişirse güncellenir.
  - `receiver.rs:192`: ön koşul cümlesi.
  - #136 `capture_recovery` `UnitTxLog`'u okur; `emission_decisions` doymaz.
  - `client_task`: `drive!` (`runtime.rs:1457-1459`) her `on_event` sonrası `take_confirmed_units()`'i drene eder; ACK (:1735-1744) ve RECOVERED (:1687-1693) yollarındaki doğrudan silme kalkar.
  - `store` BTreeMap'e geçer (O = `first_key_value`).
- **INT-02:** kontrol yolu ölçütü ACK ile sınırlıdır, çünkü heartbeat PoC'de yok (D18, `kama-node/src/event.rs:203`).

**D.6 Riskler ve çakışma.**
1. Duraklama gönderene kayıp gibi görünür (SACK boşluğu): ARQ/kopya/parite bandı harcanır, `loss_ppm` şişer; hız düşmez (INV-RATE-1). INT-02 retx/dup/fec baytını raporlar. Tel düzeyinde alıcı penceresi kapsam dışı (FROZEN tel).
2. Rezerv, R izni (ürün 64/1024) paylaşımlı havuzdan alır.
3. Reddedilen FEC kurtarımının bloğu düşer (:2676) ve birim yalnız ARQ ile gelir; kalıcı erasure + dolu kanal altında oturum kilitlenir [tasarım sınırı].
4. Mesafe kapısının bench/T-hücre davranışını değiştirmemesi beklenir [çıkarım]; `distance_holds` raporlanır, > 0 ise DUR.
5. Örtük kipte pencere ve rezerv yok (D.4).
6. Çakışma (C9 §6): `runtime.rs`/`transport.rs` 05c bitene dek A şeridine kapalı; test dosyalarına A şeridi dokunmaz; A3'ün runtime/transport entegrasyonu bu PR'ın merge'ünü bekler.
