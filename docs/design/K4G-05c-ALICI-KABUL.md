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
- **Belge:** Ek A (A.1–A.6). [K4G-05c kod] `scripts/ci/seam-coverage.json`'a §6 H-ACK-DELIVER satırının beyanı (`not_implemented`; seam drift kapısı §6 satır listesini beyanla eşitler).
- **Dokunulmaz:** `s1.rs`, `s3.rs`, `handles.rs`, protector, tel/codec/`tv_*`, UniFFI, kama-app ürün kodu, `api_surface_v0`, kimlik/freeze ve FROZEN dosyalar. INT-02 düzeneği K4G-05e'dedir.
- **Çakışma (C9 §6):** `runtime.rs`/`transport.rs` 05c bitene dek A şeridine kapalı; A3 bu PR'ı bekler.
- **Kimlik:** 05b'nin `kama-v1.1-full` beyanına 05c satırı (DUR-05c-11 (a)).
- **YAMA hedefleri:** **A.1** KM-SEAM-AT-001 §4 (sıralı teslim sözü `:151`) · A.2 plan:532 · A.3 plan:392 · A.4 HANDOFF:20 · **A.5** engine-spec §2.2 `seen_count` (main :216 / 05b-int :220) · **A.6** transport-layer-tdd §12 AMENDMENT-12 kayıt sınırı paragrafı (DUR-05c-14 (A) seçim kuralı + tanıma garantisi; CEVAP 14).

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

Biçim: üstü çiz, silme (`~~eski~~` + yeni) + tarihli YAMA KAYDI; `scripts/ci/word_coverage.py` kapısı yeşil olmalı (bu revizyonda koşulmadı). `<ONAY-TARİHİ>` = rev3'ün uygulandığı PR'ın tarihi. A.2/A.3'teki 2026-10-24 sahip karar tarihidir (DUR-18; DUR-05c-5 (a), CEVAP 10). A.3–A.5 CEVAP 10 kararlarıyla kesinleşti. A.6 CEVAP 14 (DUR-05c-14 (A, koşullu)) ile eklendi; A.6'nın CEVAP PAKETİ 14 tarihi (2026-10-25) sahip karar tarihidir.

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

### A.3 — `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:392` (DUR-05c-5 (a), CEVAP 10)

Satır içindeki parça:

```
(i) teslim kanalı taşması (`runtime.rs` ~:2321 `try_send`; INV-ACK-DELIVER-1'i ihlal edebilir → sayaç `delivery_channel_drops` K4G-11 `metrics()`'te, INT-02'de 0 olmalı)
```

yerine:

```
~~(i) teslim kanalı taşması (`runtime.rs` ~:2321 `try_send`; INV-ACK-DELIVER-1'i ihlal edebilir → sayaç `delivery_channel_drops` K4G-11 `metrics()`'te, INT-02'de 0 olmalı)~~ (i) [YAMA KAYDI 2026-10-24 — K4G-05c — DUR-18 + DUR-05c-5 (a) CEVAP (sahip): beyanlı sınır (i) kaldırıldı]
```

### A.4 — `docs/transport-v1/TRANSPORT-CRYPTO-HANDOFF.md` (DUR-05c-9 (a), CEVAP 10)

(i) Satır 8-9'daki "Kaynak: …" paragrafının ardına:

> **YAMA KAYDI — <ONAY-TARİHİ> [K4G-05c — Abkarada kararı CEVAP PAKETİ 10, 2026-10-24, DUR-05c-9 (a) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §10.1 "kuyruklar" satırı: dolu teslim kuyruğunda birim artık düşmez, ACK'siz reddedilir; offer kuyruğu ayrıca birim-kimliği mesafesiyle durur. Üstü çizili metin silinmedi.

(ii) Satır 20, eski:

```
| kuyruklar | bounded: `offer_queue 1024` (dolu → `QueueFull`), `delivery_queue 1024` (taşan teslim SAYILIR, sessiz düşmez) |
```

yeni:

```
| kuyruklar | bounded: `offer_queue 1024` (dolu → `QueueFull`; **[<ONAY-TARİHİ> — K4G-05c]** en yeni ile en eski teyitsiz birim arasındaki kimlik mesafesi `max_inflight_units`'e ulaşınca offer alınmaz, kuyruk dolar), `delivery_queue 1024` ~~(taşan teslim SAYILIR, sessiz düşmez)~~ **[<ONAY-TARİHİ> — K4G-05c]** (dolu → birim kanala girmez ve ACK'lenmez, `full_refusals` sayılır; teslim düşmez, `delivery_dropped` yapısal 0) |
```

### A.5 — `specs/adaptive-transport-engine-spec.md` (DUR-05c-1 (a′), CEVAP 10)

(i) Dosya başındaki YAMA KAYDI bloğunun en üstüne (en yeni kayıt en üstte; 05b'nin 2026-10-24 kayıtlarının önüne):

> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05c — `seen_count` anlamı; sahip kararı CEVAP PAKETİ 10, 2026-10-24, DUR-05c-1 (a′) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §2.2 ACK_FRAME_V2 `seen_count` satırına not eklendi: `seen_count` = alıcı motorunun **kabul ettiği** DATA; kabul kararı istemci runtime'ındadır (v1.1 · K4G-05c). Kabul edilmeyen datagram (oturumu hazır değil, gelen kutusunda yer yok, kimlik penceresi dışında) sayılmaz. Alan genişliği, kodlama ve çerçeve DEĞİŞMEDİ. Üstü çizili metin yok.

(ii) §2.2 V2 ölçüm ekinde, `seen_count` satırını izleyen yorum satırının (`#   seen≪sent = forward-loss; …`; main :217, 05b sonrası :221) ardına, aynı kod bloğu içinde:

```
                           #   [v1.1 · K4G-05c] seen_count = alıcı motorunun KABUL ETTİĞİ DATA; kabul kararı
                           #   istemci runtime'ındadır (kabul edilmeyen datagram sayılmaz).
```

### A.6 — `specs/transport-layer-tdd.md` §12 [AMENDMENT-12] (DUR-05c-14 (A, koşullu), CEVAP 14 §3)

(i) Dosya başındaki YAMA KAYDI bloğunun en üstüne (en yeni kayıt en üstte; 05b'nin 2026-10-24 kaydının önüne):

> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05c — K_MAP budama seçim kuralı ve geç-ACK tanıma garantisi; sahip kararı CEVAP PAKETİ 14, 2026-10-25, DUR-05c-14 (A, koşullu) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §12 [AMENDMENT-12] "Kayıt sınırı ve geç-ACK tanıma" paragrafında budama cümlesi (K4G-05b'de budama KAPALI) ve tanıma garantisi üstü çizildi; yerine: budama açık (runtime'ın birim başına O(1) kaydıyla aynı değişiklikte); birim başına uçuş + tombstone kaydı TOPLAMI ≤ K_MAP, orijinal dahil; aşılırken orijinal Data kaydı DIŞINDAKİ en eski kayıt budanır. Orijinal yalnız K_MAP budamasından muaftır; tombstone TTL'i ve tombstone tavanı tahliyesi ona da uygulanır, daha geniş saklama garantisi yoktur. Motorun hâlâ tuttuğu kaydın geç ACK'i birimi teyit eder; unutulmuş (budanmış, süresi dolmuş, tahliye edilmiş) kaydınki teyit etmez, payload korunur. K_MAP = 8 değişmedi; tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Üstü çizili metin silinmedi.

(ii) Aynı paragrafta (05b sonrası satır 926–930) eski satırlar:

```
> Budama — hangi kaydın seçileceği ve uçuş muhasebesi dahil — K4G-05c'de tanımlanır ve runtime'ın birim başına
> O(1) kaydıyla aynı değişiklikte açılır; K4G-05b'de `K_MAP` ve teyit çıktısı tanımlıdır,
> budama KAPALIDIR. Geç ACK, seq'i birimin son K_MAP iletimi arasındaysa tanınır — o kayıt TTL/tahliyeyle daha önce
> silinmemişse; tanıma garantisi bütün silme yollarıyla birlikte tanımlıdır (kayıp ilan edilmiş kayıt için tombstone
> TTL'i (K_TOMB × T_loss) ve tombstone tavanı v1 aynen; K_MAP budaması K4G-05c). Tanınmayan geç ACK veri kaybı
```

yerine (satır 931 ve sonrası değişmez):

```
> ~~Budama — hangi kaydın seçileceği ve uçuş muhasebesi dahil — K4G-05c'de tanımlanır ve runtime'ın birim başına
> O(1) kaydıyla aynı değişiklikte açılır; K4G-05b'de `K_MAP` ve teyit çıktısı tanımlıdır,
> budama KAPALIDIR.~~ **[<ONAY-TARİHİ> — K4G-05c, CEVAP PAKETİ 14, 2026-10-25, DUR-05c-14 (A)] Budama AÇIKTIR**
> (runtime'ın birim başına O(1) kaydıyla aynı değişiklikte): birimin uçuş ve tombstone kayıtlarının TOPLAMI
> ≤ K_MAP, orijinal dahil; aşılırken birimin ORİJİNAL Data kaydı DIŞINDAKİ en eski kaydı ilan edilmeden budanır
> (uçuş muhasebesinden ve kayıp dedektörü haritasından düşer; ledger ve kayıp sayaçları değişmez). Orijinal Data
> kaydı YALNIZ K_MAP budamasının hedefi olmaz; tombstone TTL'i ve tombstone tavanı tahliyesi ona da uygulanır —
> daha geniş bir saklama garantisi yoktur. ~~Geç ACK, seq'i birimin son K_MAP iletimi arasındaysa tanınır — o kayıt
> TTL/tahliyeyle daha önce silinmemişse; tanıma garantisi bütün silme yollarıyla birlikte tanımlıdır (kayıp ilan
> edilmiş kayıt için tombstone TTL'i (K_TOMB × T_loss) ve tombstone tavanı v1 aynen; K_MAP budaması K4G-05c).~~
> **[<ONAY-TARİHİ> — K4G-05c]** Geç ACK, motorun o iletim için hâlâ tuttuğu kayıtla eşleşirse birimi teyit eder:
> seq birimin orijinal Data kaydıysa ya da orijinal dışındaki en yeni kayıtları arasındaysa (toplam ≤ K_MAP) ve o
> kayıt tombstone TTL'iyle (K_TOMB × T_loss) ya da tombstone tavanı tahliyesiyle daha önce silinmemişse (v1 aynen,
> orijinal dahil); tanıma garantisi bütün silme yollarıyla birlikte tanımlıdır. Unutulmuş (budanmış, süresi dolmuş
> ya da tahliye edilmiş) kaydın geç ACK'i birimi teyit etmez. Tanınmayan geç ACK veri kaybı
```


---

## Ek B — değişecek eski testler (DUR-8 biçimi)

Değişen fikstür değil, kusuru sabitleyen iddiadır. Bu yüzden DUR-8 (a)'nın (1) "iddia ve eşik aynı kaldı" koşulu ve (2)'deki 2×2'nin 4. hücresi ("eski kod + yeni test") burada tersine işler. Dayanak: B.1/B.2 için C9 §3 + DUR-18; B.3/B.4 için DUR-05c-10 (a), CEVAP 10. Yeni iddialar eski kodda derlenir: yeni alan ya da dönüş tipi kullanmazlar, `full_refusals` iddiaları ayrı yeni testlerdedir, `push` `()` döner. Böylece 4. hücre sayısal kırmızı üretir. Her test için hücreler: eski kod + eski test yeşil · yeni kod + eski test KIRMIZI · yeni kod + yeni test yeşil · eski kod + yeni test KIRMIZI (beklenen; çıktı PR'da).

| Test (yeni ad) | Neden yeşildi | Düzeltmeyle kırmızı | Yeni iddia (fikstür aynı) | Eski kod + yeni test |
|---|---|---|---|---|
| B.1 `transport.rs:1412-1428` `reorder_is_bounded_and_advances_past_a_gap_only_over_cap` (→ `…_never_skips_a_gap`) | atlama :398-402 (`[1,2,3,4]`) | 4 pencere dışı, `out` boş | `out` boş, `held.len() ≤ 3` | `out = [1,2,3,4]` |
| B.2 `runtime_e2e.rs:255-305` (→ `delivery_overflow_refuses_without_ack_not_grown`) | :288 `acked == 40`, :297-300 `delivery_dropped > 0` | durakta ≤ 2 teyit, düşüş 0 | durakta teyit ≤ kanal (4), `delivery_dropped == 0`, drenajda kanal çalışır; eksiksizlik P0'da | 40 teyit, 36 düşüş |
| B.3 `runtime.rs:3976-4017` (→ `overflow_is_refused_and_drop_mirror_stays_zero`) | 15 düşüş canlı + kapanış | `wait_until(15)` zaman aşımı | `delivery_dropped()` canlı/kapanış 0; drenajda `delivered == 16` | canlı 15 |
| B.4 `kama-node/src/node/tests.rs:415-456` (→ `delivery_channel_full_refuses_and_node_metrics_stay_zero`) | demetler ACK'lenip düşer, `delivery_channel_drops > 0` | `until` zaman aşımı | Alice `delivery_queue + 1` demet `Ok` ya da `Backpressure` alana dek basar; Bob döngüye girince PttAudio sayısı = `Ok` sayısı, `delivery_channel_drops == 0` (içerik aynı → sıra iddiası yok) | PttAudio < `Ok` |

Belge ve mekanik:
- B.3 aynanın canlılığını artık ayırt etmez (sayaç yapısal 0; beyan). Atıf veren `transport.rs:1430-1433` belgesi güncellenir.
- `transport.rs:581-584`, `runtime.rs:645-646` ve `:2905-2911`: "yapısal 0; > 0 kabul kararının atlandığını gösterir".
- `session_keyed_slots.rs` (e): davranış aynı. `AcceptState` `retire_closed_slots` ile AYNI adımda silinir; belge "yuvanın dedup penceresi" → "oturumun kabul durumu".
- `runtime.rs:3929` `PeerSlot::new` (test): imza değişirse güncellenir.
- `kama-node/src/event.rs:209` geri çekilen DUR-K4G-01 (i)'ye atıf verir → K4G-16 belge kalemi.

---

## Ek C — DUR'lar

**Kapanan (sahip kararı; uygulanır, yeniden açılmaz — C11 §3):**

| DUR | Karar | Kaynak |
|---|---|---|
| DUR-05c-1 | (a′): çağrı = kabul (`receiver.rs` yalnız doc) + engine §2.2 `seen_count` yaması (A.5) | CEVAP 10 |
| DUR-05c-2 | (a) pay = 0 | CEVAP 10 |
| DUR-05c-3 | Kabul. Durakta eşzamanlı dosya ≥ 1 025 birim; taban kolunda `delivery_channel_drops > 0`; geçerlilik `full_refusals > 0`; 3/3. Profil taşıyamazsa kanal kapasitesi test ayarı değiştirilmez, DUR açılır | CEVAP 10 |
| DUR-05c-4 | (a) engine §14 dokunulmaz | CEVAP 10 |
| DUR-05c-5 | (a) A.3: plan:392 ve :532 | CEVAP 10 |
| DUR-05c-6 | (a) `NodeMetrics`/UniFFI değişmez; düzenek K4G-05e | CEVAP 10 |
| DUR-05c-7 | Kapandı: runtime K_MAP kullanmaz (silme teyit çıktısıyla) | rev2 |
| DUR-05c-8 | (a) R = min(`max_peers`, C − 1) = 64/1024; yalnız sıra başı garantisi, adalet sözü yok | CEVAP 10 |
| DUR-05c-9 | (a) A.4 | CEVAP 10 |
| DUR-05c-10 | (a) B.3/B.4 DUR-8 biçimiyle | CEVAP 10 |
| DUR-05c-11 | (a) 05b'nin `kama-v1.1-full` beyanına 05c satırı; T-hücre birleşik adayda | CEVAP 10 |
| DUR-23 (b) | Kabul; **05b'de** karşılandı: `SenderRecoveryDiag::{seq_to_unit_len, unit_seqs_len}` (05b-int `runtime.rs:819-820`) | CEVAP 10 + 05b |
| DUR-05c-13 | Teyit sonrası bayat kayıt sızıntısı **05b (`confirmed_units`) ile giderildi**; kanıt 3439fccd + kanca kırmızısı; 05b bekçisi `assert_runtime_maps_empty` (05b-int `k4g05b_runtime_ownership.rs`) | CEVAP 11 |

**Açık:**

| DUR | Konu | Seçenekler (öneri kalın) |
|---|---|---|
| DUR-05c-12 (yeniden; C13 §2) | Parçalı mesaj: ret katmanı ve terminal durum | **A — mevcut mekanizma + kapsam kilidi.** S1'de transport parçası yapısal olarak yok (`transport.rs:498-500`, `runtime.rs:266`); parçalı mesaj = dosya; ret Application'da (ANALIZ.md §2). Testler: `FragmenterOn` kilidi (kapı bugün testsiz); F6 enjeksiyonu (alıcıda `Failed(IdleTimeout)`, gönderende `SendFailed(ReceiverTimeout \| ReceiptTimeout)`, o kaydın transport yeniden gönderimi 0, sonraki TEXT ve aktarım ilerler); F3 `Rejected` uçtan uca. S1-dışı `MsgAsm` (bench/test) değişmez, beyanlı; eski D.5 "Parçalı: `MSG_ASM_CAP`" testi S1-dışı yola aittir, A'da yoktur. · B — Application'da açık ret olayı + hızlı terminal: `FILE_RECEIPT(rejected)` anlamı genişler, S8 kararı ve UniFFI yüzeyi değişir; **ayrı kart, sahip isterse**. · C — S1-dışı `MsgAsm` hijyeni 05c içinde: kapsam ürün dışı yol için büyür (önerilmez). · D — tel: D1 `FILE_RECEIPT` `0x05 invalid_record`, D2 `SESSION_CLOSE` (karar sahibin; önerilmez). **Sahibe iki olgu:** (a) dosyada yuva-dolu reddi (F4, `file_transfer.rs:1027-1035`) bugün spec gereği açık ve terminal `FILE_RECEIPT(rejected)`'dır (AMP spec :89); C13 §2'nin "kapasite reddi R2'dir" okumasıyla gerilimdedir: R2 transport kapasitesi içindir, F4'ü R2'ye taşımak spec kararını değiştirir. (b) `FILE_RECEIPT_TTL` 60 s, `FILE_END` kuyruğa alınınca başlar (`file_transfer.rs:1541`); düşük hızda geçerli dosyada yanlış `ReceiptTimeout` riski var [çıkarım, ölçülmedi]; INT-02'de 31–100 B/s'de tek dosya ölçümü önerilir. |
| DUR-05c-14 (yeni; C13 §4) | K_MAP budaması açılınca hangi kayıt budanır: `f3_di12ad` ResolvedByDataAck → TimedOut ve `external_pto` gerilemesi | **(A) orijinal Data kaydı budanmaz; budama orijinal dışındaki en eski kaydı seçer; sınır ≤ K_MAP aynı. İki test değişmeden yeşil [çıkarım; 05c kapısında doğrulanır].** · (A′) BUDAMA-UYARISI (a): orijinal sayım dışı, sınır ≤ 1 + K_MAP. · (B) yalnız kopya kökenli kayıtlar budanır; Data ve en son ARQ kaydı korunur. · (C) kasıtlı sözleşme değişikliği ("ufuk dışı geç ACK kuruluşu çözmez"), iki test DUR-8 ile değişir; C13 §4 kasıt kanıtı ister, önerilmez. (A), (A′) ve (B) AMENDMENT-12'nin "birimin EN ESKİ kaydı budanır" cümlesini değiştirir (aşağıda çelişki 1). |

**[CEVAP PAKETİ 14, 2026-10-25] Kapandı:** DUR-05c-12 → **(A)** (FragmenterOn kilidi + dosya F6/F3 uçtan uca + yuva-dolu reddi açık ve terminal, R2 sayılmaz; B ayrı kart, istenmiyor) · DUR-05c-14 → **(A, koşullu)** (orijinal Data kaydı yalnız K_MAP budamasının hedefi olmaz; TTL ve tahliye orijinale de uygulanır; toplam ≤ K_MAP orijinal dahil; K_MAP / TTL / tahliye için ayrı geç ACK testleri; A.6). Yukarıdaki "Açık" tablosu karar öncesi metindir.

**Çelişki / koordinasyon:**
1. ~~**AMENDMENT-12 budama cümlesi.** Cümle 05b'nin uyguladığı spec'tedir (05b-int `specs/transport-layer-tdd.md:920-929`). Orada hâlâ "geç-ACK ufku … 240 s" yazıyor; C11 DUR-28 bu ifadeyi kaldırdı (05b rev5 işi). DUR-05c-14 (A/A′/B) aynı cümlenin budama kuralını değiştirir. Karar gerekiyor: metin 05b rev5'e mi girer, 05c Ek A'ya (A.6) mı?~~ **Çözüldü (orkestratör, 2026-10-25):** 05b rev5 (PR #140 @35a02fcf, A.7 `0b4ff343`) "240 s"yi kaldırdı ve kayıt seçim kuralını 05b'de sabitlemedi ("Budama — hangi kaydın seçileceği ve uçuş muhasebesi dahil — K4G-05c'de tanımlanır"). Kural 05c Ek A'ya (A.6) DUR-05c-14 kararıyla girer; 11-EK §4'ün aynen gelen garanti cümlesi ve "≤ K_MAP" sınırı da (A)/(A′)/(B) seçimine göre 05c Ek A'da ayarlanır.
2. ~~**`k4g05b_accept_p0_mirror_horizon_out_declared_red`.** P0 `RuntimeMirror` eski runtime davranışını kopyalar. C13 §3 ile runtime uyarlaması 05b'ye geçtiği için TTL kolu 05b'de etkindir (05b-int `late_ack_after_ttl_keeps_payload`). C13 §5 gereği bu kol 05b kapısında kalmalıdır; 05c'ye yalnız budanmış-kayıt kolu aittir (`late_ack_after_prune_keeps_payload`). Ayna metni tanımı gereği yeşile dönemez, aktif teste dönüştürülmez.~~ **Çözüldü (orkestratör, 2026-10-25):** okuma kabul edildi ve 05b rev5 B.2'ye işlendi: ayna testinin yerini gerçek-runtime testleri alır — TTL kolu 05b kapısında (`late_ack_after_ttl_keeps_payload`, PR #140), budama kolu 05c kapısında (`late_ack_after_prune_keeps_payload`).
3. **Kol 2 sınırı.** C11-EK §5 "kol 2 geçerse sınır kalkar" der, C13 §7 "tek başarılı koşu sınırı kaldırmaz" der. Öncelik C13'tedir; not C13'ü izler.

---

## Ek D — türetimler ve atıflar

**D.0 Mekanizma satır atıfları** [kod `1b1425d2`, P0].
- **R2:** DATA kolu `runtime.rs:3578-3602`: `on_datagram` :3578 (PN kaydı `receiver.rs:233`) → `admit_chunk` :3589 (dedup :3055) → `push_delivery` dolu kanalda yalnız sayar (:3025-3031) → ACK :3602. FEC: ESI listeye :3294'te, parse (:3296) ve teslimden (:3308) önce girer; RECOVERED çıkışları :3649, :3734, :3799. `MsgAsm` reddi :3085, tahliye :3097-3100 (S1-dışı). P0: 36/40 ACK'li ama teslimsiz; arkadaki 12..15 hiç teslim edilmedi.
- **R1:** `transport.rs:398-402` boşluğu atlar, :394-396 geç eksik birimi sayaçsız atar (doc :384-385); gönderen kapısı yalnız sayıya bakar (`runtime.rs:2430`). P0: cap 3, `1,2,3,4,0` → 0 atıldı.
- **Kayıt/payload:** main'de ekleme :1998; silme ACK'te :2278-2285 ve RECOVERED'da :2216-2221, ikisi de motordan bağımsız; store-miss :2072-2077. 05b-int'te payload yalnız `drain_confirmed!` (:2078-2099) ile düşer; ekleme yalnız store'daki birim için yapılır (:2119-2124). Envanter satırları 1–3, 5, 7–8 ayrışır (ENVANTER.md). `Closed`: `transport.rs:1069-1073`.

**D.1 Receiver girdisi ve `seen_count`.** Kabul kararı `Receiver`'dan önce verilir; `on_datagram` çağrısı kararın kendisidir. Reddedilen datagram `NotReady` kolu gibi (`runtime.rs:3450`, :3467) motora hiç girmez. PN yalnız Accept/Duplicate'te kaydedilir ve `AcceptState` unutmaz; bu yüzden yinelenen PN ⇒ kabul edilmiş birim olur ve yinelenen PN'e Refuse çıkmaz. Reddedilen DATA `seen_count`'a girmez; gönderen durakta ileri-yön kaybı görür. Spec bunu A.5 ile yazar (DUR-05c-1 (a′)). `on_datagram(.., Admission)` seçeneği reddi `seen_total`'a katardı: gönderenin dönüş-yolu gözlemi (:2243) bunu dönüş bozulması sanabilirdi.

**D.2 Ayrılı yer ve döngüsel bekleme yokluğu.** C = `delivery_queue`, R = min(`max_peers`, C − 1), F = boş izin (`delivery_tx.capacity()`), U = rezervi kullanımda olan oturum sayısı. Oturum s'nin rezervi, rezervle kabul edilen teslimi kanaldan alınana dek kullanımdadır. Sunucu rezervli teslimin oturum içi sıra numarasını defterde tutar; tüketici `try_next_delivery`'de alımı ve oturum sayacını aynı kilit altında yapar; sunucu F ve U'yu aynı kilit altında okur. Kural: (i) u = floor_s ve s'nin rezervi boş → kabul, rezerv kullanımda; (ii) diğer her kabul F − 1 ≥ R − U ise.
- **Değişmez I: F ≥ R − U.** Başta F = C > R. (ii) I'yı korur. (i)'de U ≤ R − 1 iken F ≥ 1 olur; kabulden sonra F − 1 ≥ R − (U + 1). Rezervli teslim alınınca U azalır, F aynı adımda artar.
- **Kapsam:** aynı anda R oturuma kadar her oturumun sıra başına yer garantidir; R + 1'inci oturum yalnız tüketimi bekler. Kota: kanalda ≤ (C − R) + 1, Reorder'da ≤ cap. Fiziksel kanal C değişmez (`transport_v1_freeze.rs:99`).
- **Oturum geçişi:** `h` ve `h'` iki ayrı oturum olarak sayılır; geçiş sırasında U en çok iki rezerv artar. I aynı kalır ve iki oturumun sıra başı garantisi sürer (D.5 testi).
- **Döngüsel bekleme yok:** floor'un kabulü üç şeye bağlıdır: pencere (floor daima içinde), s'nin rezervinin boşalması (adaptörün kanalı boşaltması) ve I. Boşaltma yalnız adaptörün sürülmesine ve `inbox < MAX_INBOX`'a bağlıdır; inbox yalnız sıralı birim tutar ve uygulama tüketimiyle boşalır. Hiçbir adım bir eksik birime bağlı değildir.
- **`held ≤ cap`:** tutulan her u kabul anında u ≤ floor_t + cap'i sağlar. floor_t altındaki birimler FIFO kanalda u'dan önce yer alır, dolayısıyla `next ≥ floor_t`.

**D.3 Pay.** Payload yalnız `take_confirmed_units` çıktısıyla silinir. Bu çıktının kaynakları canlı kayıtla eşleşen ACK ve RECOVERED'dır; ikisi de alıcıda yalnız Accept/Duplicate'ten doğar. Dolayısıyla teyitli ⇒ kabul edilmiş, buradan O ≤ floor. Gönderilen her u ≤ O + cap − 1 < floor + cap olur. ARQ, kopya ve prob mevcut kimliği taşır; parite kimliği ESI'dir. Pay 0'dır (DUR-05c-2 (a)).

**D.4 Kabul durumunun kapsamı.** Birim-kimliği uzayı oturumundur: istemci her oturumda 0'dan başlar (`runtime.rs:1920`). Yuva anahtarlı durum üç durumda pencereyi 0'a çekip oturumu kilitlerdi: `idle_hold` (105 s) emekliliği, adres değişimi ve ek yuva (`runtime.rs:1241-1243`). Bu yüzden S1 kipinde durum ve rezerv defteri oturum anahtarlıdır ve `retire_closed_slots` (:3154; çağrılar :3428, :3535) ile aynı adımda silinir. Örtük kipte (`bind`/`bind_with_policy`, her yuva ayrı istemci) durum yuva başına `delivered_seen`'de kalır; pencere ve rezerv yoktur; değişen yalnız dedup'un commit'te ilerlemesi ve dolu kanalda ACK'siz rettir.

**D.5 Yeni testler ve dosya ayrıntısı.**
- **`late_ack_after_prune_keeps_payload`** (05b-int `k4g05b_runtime_ownership.rs` düzeneği, `late_ack_after_ttl_keeps_payload` :775 deseni): budamayla düşmüş seq'e geç ACK → teyit yok, payload yerinde, yeniden gönderim gövdeli, store-miss 0.
- **`unit_records_bounded_per_unit`:** her örnekte `unit_seqs_len` ≤ store.len() ve `seq_to_unit_len` = 0.
- **FEC kol 1:** `max_peers` 1 → R = 1, kanal K − 1 = 7, hedefin yalnız ilk emisyonu düşer. Durakta hedef teyitsiz, `full_refusals > 0`; drenajda 8/8. ESI mutantı burada ölür.
- **FEC kol 2:** kalıcı kaynak kaybı. Deneme ve rapordur, CI testi değildir (`#[ignore]` kullanılmaz); sonuç KATMAN 6 sınırını değiştirmez.
- **Oturum geçişinde rezerv:** yenileme ya da supersession sırasında `h` ve `h'` birlikte. İki oturumun sıra başı dolu kanalda kabul edilir; her örnekte U ≤ R ve F ≥ R − U.
- **Diğer:** kanal rezervi (A'nın seli kanalı doldururken B'nin sıra başı kabul edilir) · kontrol yolu (dolu kanalda ACK emisyonu ≥ 1, `mark_t0` döner) · gönderen mesafe · pencere ihlali.
- **`FragmenterOn` kilidi:** `seam_real_adapter/main.rs`, `product_builder_rejects_recovery_off` (:70-83) deseni.
- **Dosya davranışı:** DUR-05c-12 A.
- **Taşınan dört iddia** (§5) ve iki budama testi değişmeden.
- **Dosya ayrıntısı:**
  - `CtxDispatchStats` dört alan kazanır (`full_refusals`, `reorder_skip`, `reorder_window_violations`, `distance_holds`); literaller `runtime.rs:3906-3910` ve `ctx_first_dispatch.rs:220-226`.
  - `receiver.rs:192` ön koşul cümlesi.
  - #136 `capture_recovery` (main :829, 05b-int :846) `UnitTxLog`'u okur; `emission_decisions` doymaz.
  - `store` BTreeMap'e geçer (main :1906, 05b-int :1956).
  - Mutasyon listesi PR gövdesinde.

**D.6 Riskler.**
1. Duraklama gönderene kayıp gibi görünür: ARQ/kopya/parite bandı harcanır, `loss_ppm` şişer; hız düşmez (INV-RATE-1). INT-02 bunu raporlar.
2. Rezerv R izni (64/1024) paylaşımlı havuzdan alır.
3. FEC kol 2 tamamlanmayabilir (3.6; KATMAN 6).
4. DUR-05c-14 (A)'da orijinal kayıt uçuş muhasebesinde kalır (cwnd payı) [çıkarım].
5. Mesafe kapısının bench/T-hücre davranışını değiştirmemesi beklenir [çıkarım]; `distance_holds` > 0 ise DUR.
6. Örtük kipte pencere ve rezerv yok.

---

## Ek E — geri çekilenler (tarihçe; MUST NOT IMPLEMENT)

Aşağıdaki maddeler önceki revizyonlarda ya da kararlarda yer aldı ve geri çekildi. Uygulanmaz; metin iz için korunur.

- ~~DUR-05c-12 (CEVAP 10): "parçalı mesajda protokol ihlali: ACK + düşür + `frag_chunk_dropped`"~~ — CEVAP 11 ile geri çekildi.
- ~~DUR-05c-12 (CEVAP 11): "geçersiz girdi → ACK YOK + `FragmentInvalid{peer, msg_id, neden}` olayı"~~ — CEVAP 11-EK §2 ile geri çekildi (sonsuz yeniden gönderim ve baş-blokajı).
- ~~CEVAP 11-EK §2: "aynı mesaj ikinci kez reddedilirse gönderende kalıcı `Failed`"~~ — CEVAP 13 §2 ile geri çekildi (gönderen ret sayısını gözleyemez).
- ~~rev2 §3: "parçalı mesaj: MsgAsm'de yer yoksa Refuse, tahliye kalkar"; D.5 "Parçalı: runtime birim testi (`MSG_ASM_CAP`)"~~ — S1-dışı yoldur; DUR-05c-12 A'da yer almaz (yalnız C seçilirse).
- ~~rev2 §3: "runtime payload'u ve `unit_acked!`'i yalnız `take_confirmed_units` ile siler" (05c işi olarak)~~ — CEVAP 13 §3: bu uyarlama 05b'dedir (05b-int `drain_confirmed!`).
- ~~rev2 DUR-23 (b) tasarımı: `#[cfg(any(test, feature = "test-util"))] ClientHandle::unit_records_for_test()`~~ — 05b'nin `SenderRecoveryDiag::{seq_to_unit_len, unit_seqs_len}` alanlarıyla karşılandı.
- ~~rev2 DUR-05c-13: "05c tabanında yeşilse nota 'yanlışlandı (05c tabanında)'"~~ — CEVAP 11: "05b (`confirmed_units`) ile giderildi".
- ~~rev2 DUR-05c-1 (a): "`seen_count` sapması beyanla kalır, engine §2:216 değişmez"~~ — CEVAP 10: (a′), spec yaması A.5.
- ~~rev1 kart metni: "pay = kopya işi sınırı (3) × …"~~ — CEVAP 10 DUR-05c-2 (a): pay = 0.
- ~~CEVAP 11-EK §5: "kol 2 geçerse sınır kalkar"~~ — CEVAP 13 §7: tek başarılı koşu sınırı kaldırmaz.
- ~~rev2 §6: "INT-02 kolu C9 §5 metniyle (50 TEXT)"~~ — CEVAP 10 DUR-05c-3: eşzamanlı dosya ≥ 1 025 birim.
