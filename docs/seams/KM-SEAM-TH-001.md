# KM-SEAM-TH-001 — S4: Transport ↔ Hub sözleşmesi (PoC, tek hub)

Sahip ve tek imzacı: Abkarada · Sürüm: **v1.3.1 taslak** (2026-10-15; v1.3 + Yaaz'ın uygulama soruları: cookie adres ailesi, aynı adresten iki kimlik, yansıtma kuralı, sayaç adları) · Durum: **G-S4 öncesi** — "G-S4 GEÇTİ" mesajıyla FROZEN olur · Yerine geçtiği belge: `docs/seams/KM-SEAM-TH-001-TASLAK.md` (#119)

Bu belge **iki kodun ortak hakikatidir**: `client-core`'un hub istemcisi (A3, Abkarada) ve `hub-server` (Y1, Yaaz). Baytlar yalnız burada tanımlanır; `KM-HUB-TDD-001` bu belgeye referans verir, kopyalamaz. Kaynak atıfları `S4-CIKARIM-2026-10-13.md`'deki (Ç-n) alıntılara ve spec satırlarına gider; **[YAMA]** = spec'e YAMA KAYDI ile girecek PoC eklemesi/değişikliği (§8'de toplu). Bu belge mevcut spec'in doğrudan uygulaması değil, spec'in üstüne yazılmış ve §8 ile spec'e işlenecek **PoC profilidir**.

---

## 0. Kararlar (KR) — G-S4 bunları onaylar

| # | Karar | Bu belgedeki hâli (önerim) | Spec dayanağı / alternatif |
|---|---|---|---|
| **KR-1** | Kontrol çerçevesi ve tip ayracı | HubChannel datagramı = `[0x00000000][frame_type][gövde][16×0x00]`; `frame_type` engine-spec §1.1 tablosundan; Plane 1 RPC'leri **yeni** `0x1D PLANE1_RPC` çerçevesinin gövdesinde `AppRpcFrame` olarak **[YAMA engine §1.1]** | Ç-1 `open_hub` çerçeveyi 4 baytlık tutamaçla birlikte döndürür; engine §1.1 "transport-control frame ilk byte = frame_type"; HubChannel üç biçimi ayraçsız taşıyordu (0x01 çakışması) → ayraç = frame_type. |
| **KR-2** | Handle modeli | Oturum başına **tek** `hub_handle`, iki uçta **aynı**, hub INIT kabulünde verir, **yeniden yazmaz**; ömrü oturum ömrü; yeniden kullanım 7 gün tombstone | noise-wire §2.55 (iki tarafa aynı; h_ext AAD'de), kod (`EnvelopeLink`, `LoopbackEnvelopeRelay`), ccr §35.7 KS-HANDLE-1. app-rpc §6.1 "her leg bağımsız handle" çok-hub içindir **[YAMA §6.1 notu]**. |
| **KR-3** | Yanıtlayan handle'ı nasıl öğrenir | `DELIVER` yükünün başında `hub_handle` **[YAMA app-rpc §6.11 — exact bytes OPEN]**; `HANDLE_ASSIGN`/`_ACK` çerçeveleri PoC'de yok. RESPONSE'u taşıyan `DELIVER`'daki handle, onu kabul eden oturum için de **atama** sayılır (§4 C4 — UDP sıralama boşluğu kapanır). Aynı INIT'in tekrarını alan yanıtlayan **yeni oturum açmaz**, mevcut el sıkışmanın RESPONSE'unu yeniden gönderir (C4) | Kod: `LinkEvent::IncomingInit/Response { hub_handle, … }`; `AssignHandle` idempotent (S3). |
| **KR-4** | RESPONSE hangi hatta | `STORE_MESSAGE` yükünün başında `hub_handle` (0 = yeni hat; ≠0 = mevcut hat) **[YAMA §6.1]**; geçerli (handle, tip) birleşimleri §2.4(c) tablosunda | Spec'te tanımsız; kod `send_response(peer, hub_handle, …)`. |
| **KR-5** | Bind doğrulaması (PoC) | **PoC-BIND**: `CLIENT_BIND` (+`client_nonce`) → durumsuz cookie → **yeni method `0x0B BIND_PROOF`** (cihaz Ed25519) → imzalı yanıt. 2 RTT. Cookie, ucun **`bind_epoch`**'una bağlıdır (her kabul edilen bind'de +1): kabulden önce verilmiş her cookie geçersizleşir → eski/gecikmiş kanıt asla kabul edilmez. Hata muhasebesi yalnız **geçerli cookie'den sonra** ve **kaynak adresine** **[YAMA app-rpc §5]** | Spec'in CLIENT_BIND'i Plane 0 Noise IK üstünde (M4). İmza anahtarı = sertifikadaki cihaz Ed25519 (ccr §13.1), kimlik anahtarı değil. İkinci inceleme 1–2. |
| **KR-6** | Dönüş adresi | Uç → **yalnız doğrulanmış bind** ile kurulur/taşınır; yeni bind hatları korur; veri/STORE/heartbeat adres değiştirmez; `bind_epoch` sayesinde eski bir kanıtın tekrarı adresi geriye taşıyamaz; hub kayıp ACK için yalnız **son kabul edilen** kanıtı hatırlar | PoC-KAPSAM K5 **[YAMA K5 notu]**; transport-tdd §2 "NAT-robust" → PoC'de path validation yok **[YAMA]**. |
| **KR-7** | Bilinmeyen/emekli handle | Bağlı kaynaktan veri → `ROUTE_MISS{hub_handle, reason}` (hız sınırlı); yabancıya sessiz **[YAMA §6.11 — `link_ref` tanımsız]** | K5 ↔ ccr §35.7/§35.16; TASLAK A4. |
| **KR-8** | `SESSION_REBIND` ve K = 1024 | PoC'de `SESSION_REBIND` yok; ROUTE_MISS kurtarması = yeni INIT; K = 1024 istemci işi, hub'a dokunmaz | app-rpc §4; ccr §35.16; DR-CONNECT-1 H5. |
| **KR-9** | Bellek: zaman tek başına silmez | Uç tablosu dizinle sınırlı. Hat tahsisi bir **emeklilik planıyla** (§2.6, sekiz adım): **alıcı** kotası doluysa yalnız **gönderen ile alıcı arasındaki** en-uzun-boşta hat plana girer (ortak hat yoksa `HUB_CAPACITY`; üçüncü tarafların hatlarına dokunulmaz); **gönderen** plan sonrası hâlâ kotadaysa kendi kalan en-uzun-boşta hattı; küresel tavan plan sonrası sayıyla; plan bir **küme**. Bütün denetimler (iki uç + küresel sınır, tombstone kapasitesi, aday handle) geçmeden hiçbir şey uygulanmaz — başarısız tahsis mevcut hatları, uç sayaçlarını ve tombstone içeriğini **değiştirmez** (yalnız hız bütçesi ve aday sayacı ilerler). Tombstone **kapasiteli**, erken silinmez; pasif sessizlikle hat silinmez | INV-NO-IDLE-TEARDOWN; KS-HANDLE-1; üçüncü tur 2. |
| **KR-10** | Medya | Hub için medya = `STORE_MESSAGE{envelope_type = 0x05 RELAY_OPEN}` ile açılan sıradan ikinci hat; `flow` yok; telefonda tek soket. Zarf = 16 B rastgele `open_nonce` (hub için ve alıcı için **bilgi**; bağlama KR-17) **[YAMA registry]** | EK-3 "hub yalnız iletir". Medya **anahtarı** kararı ayrı (A3/medya). |
| **KR-11** | Hız sınırları | CLIENT_BIND 1. tur sınırsız; BIND_PROOF: **önce cookie (ucuz), sonra adres kilidi + bütçe, sonra imza**; yanlış cookie **hiçbir durum yaratmaz** (sayaç yok); başarısızlık sayacı yalnız geçerli cookie'den sonra ve kaynak adresine, tablo `BIND_FAIL_ENTRIES` ile sınırlı; STORE 10/s/uç; hat tahsisi 1/s/uç; ROUTE_MISS 1/s/uç; veri hattında tavan yok (beyan) | KS-DOS-1; inceleme bulgusu 1; ikinci inceleme 2. |
| **KR-12** | Bağımlılık | `hub-server` → `kama-contracts` + `kama-noise-wire` (`ed25519_strict::verify_strict`, `entropy::OsEntropy`, `handshake::cert_ref_of`) + `ed25519-dalek` + `blake2` + `sha2` + `serde`/`serde_json` (hepsi Cargo.lock'ta) | Ç-19/Ç-20. |
| **KR-13** | Provisioning v1 | `--hub` → hub Ed25519 çifti, cihaz dosyasında `hub.ed25519_pub`, `hub.json`; `--probe` ile eş listelerine girmeyen `probe` | Ç-18. |
| **KR-14** | İstemci soketi | Telefon hub'la **tek UDP soketinden** konuşur | KR-6 uç başına adres. ADR-15 çelişirse A3'te karar. |
| **KR-15** (v1.1) | `DELIVER` gönderen kimliği | `DELIVER` yükünde `sender_dest_hash` (hub'ın bağlı kaynaktan **tanıklık ettiği** gönderen) **[YAMA §6.11]** | ccr :3099 "DELIVER taşıyıcı bağlamı gönderen kimliğini veriyorsa tipik durum tek deneme"; bilgi amaçlı, güvenlik kararı buna dayanmaz. |
| **KR-16** (v1.1) | Hata yanıtı biçimi | Protokol/yetki hataları spec'in `ErrorPayload`'ıyla (`flags.error = 1`); uygulama durumları (`CHALLENGE`, `PEER_UNBOUND`, `UNKNOWN_HANDLE`) sabit yanıt yerleşiminin kendi alanında | app-rpc §3/§11 aynen; bulgu 6. |
| **KR-17** (v1.2) | Medya hattının eşe bağlanması | Hub kanalı imzasız olduğu için bağlama **hub mesajıyla değil, uçtan uca** yapılır: hattı açan taraf `h`'yi aldıktan sonra eşe E2E, güvenilir (ARQ) bir **`MEDIA_LINK{OPEN, h, open_nonce}`** uygulama mesajı gönderir; eş **`MEDIA_LINK{ACK, h}`** döner; medya ancak ACK'ten sonra akar. Hub'dan gelen `DELIVER{RELAY_OPEN}` hiçbir bağlama yaratmaz. Sahte/kayıp/erken DELIVER'ın etkisi yok. Kural ve glare §4 C10; mesaj uygulama mesaj protokolüne eklenir (§8 Y12) — **tür kaydı `s1.rs`'te ise FROZEN istisnası bu KR ile verilir** | İkinci inceleme 5. Signaling blobu (medya ekibinin) okunmaz, değişmez; `call_id` telde açık taşınmaz. |
| **KR-18** (v1.3) | INIT tekrarının durma koşulu | `STORE_MESSAGE` yanıtındaki `OK`, hub'ın isteği kabul edip handle tahsis ettiğini gösterir; **karşı ucun INIT'i aldığını ya da el sıkışmanın tamamlandığını göstermez.** Başlatan, aynı el sıkışma denemesinde aynı `request_id` ve bayt-özdeş INIT ile T3 tekrarını **sürdürür**; tekrar yalnız doğrulanmış RESPONSE kabul edilince, bağlantı niyeti iptal edilince ya da deneme açıkça sonlandırılıp yeni denemeye geçilince durur (§4 C3). RELAY_OPEN'ın tekrarı ayrı tanımlıdır: hub yanıtını bekler (C10). Tamamlanmamış bir denemenin azami yaşı `ATTEMPT_MAX_AGE` | Üçüncü tur 1. Kurulmuş oturumlara uygulanmaz; pasif sessizlikle kapatma değildir. |
| **KR-19** (v1.3) | Tahsis anahtarı indeksi | Her aktif hat kendisini açan `allocation_key`'i taşır; `key → handle` indeksi **bütün aktif hatları** kapsar (bağımsız LRU tahliyesi yok); hat emekli olunca anahtar tombstone'a taşınır (aynı kapasite, aynı 7 gün). STORE: anahtar aktifte → aynı handle + yeniden DELIVER; tombstone'da → yanıt `{UNKNOWN_HANDLE, 0}` (tahsis yok, DELIVER yok); hiçbirinde → yeni tahsis. İstemci `UNKNOWN_HANDLE`'da eski denemeyi sonlandırır, niyet sürüyorsa **yeni kriptografik INIT + yeni `request_id`** (§4 C3) | Üçüncü tur 3. `IDEMPOTENCY_RING` ve "64 > 40" değişmezi kaldırıldı: UDP gönderim aralığı hub'a ulaşma aralığını garanti etmez. |

---

## 1. Model (on cümle)

1. İki telefon birbirine doğrudan ulaşamaz; **hub** sabit adresli tek bir UDP sunucusudur, paketleri birinden alıp diğerine atar.
2. Telefon önce hub'a **bind** olur: kimliğini (`cert_ref`) söyler, hub'ın meydan okumasını cihaz anahtarıyla imzalar; hub da yanıtını imzalar. Bundan sonra hub, o kimliğin **dönüş adresini** bilir.
3. Oturum kurma mesajları (INIT / RESPONSE) hub üzerinden **`STORE_MESSAGE` → `DELIVER`** ile iletilir; hub zarfı **açmaz**, saklamaz; alıcı bağlı değilse göndericiye durum döner, gönderici yeniden dener (T3).
4. INIT kabulünde hub oturuma 4 baytlık bir **`hub_handle`** verir; iki telefon da aynı değeri kullanır.
5. Veri datagramı `[hub_handle][şifreli yük]`'tür; hub ilk 4 bayta bakar, kaynağın o hattın bir ucu olduğunu denetler, datagramı **değiştirmeden** öbür uca atar.
6. Hub hiçbir **canlılık kararı** vermez; sessizlik hiçbir şeyi silmez. Telefon kopmayı olaylardan öğrenir (OS, soket hatası, ARQ, **cevapsız yoklama**).
7. **Heartbeat** telefonun NAT deliğini açık tutar; hub yalnız bağlı adrese cevap verir, durum tutmaz.
8. Bilinmeyen bir handle'a veri gelirse hub bağlı göndericiye **`ROUTE_MISS`** döner; telefon o handle'ı bırakır ve yeni INIT atar.
9. Hub'daki her tablo bellektedir; hub yeniden başlarsa boş başlar, telefonlar yeniden bind olur ve oturumları yeniden kurar (beyanlı PoC sınırı).
10. Sunucuda telefonlara ait **gizli** hiçbir şey yoktur: yalnız dizin (kimlik, sertifika) ve hub'ın kendi anahtarı.

---

## 2. Tel

Tüm sayılar big-endian, tüm alanlar sabit genişlikte (ADR-05; varint yok). Datagram ≤ `MAX_DATAGRAM` (§6).

### 2.1 İki datagram sınıfı (hub ilk 4 bayta bakar)

| İlk 4 bayt | Sınıf | Biçim |
|---|---|---|
| `00 00 00 00` | **HubChannel kontrol** (S3 `ProtectionContext::HubChannel`, PoC no-op: `seal_hub(frame) = frame ‖ 0×16`, `open_hub` = "tutamacı kayıtsız VE son 16 bayt sıfır") | `[0x00000000][frame_type u8][gövde][0×16]` — en az 21 B |
| başka | **Veri (Plane 2)** | `[hub_handle u32 ≠ 0][opak yük]` — hub yükü okumaz; en az 5 B |

Hub `hub_handle = 0` **hiç vermez**.

### 2.2 HubChannel çerçeve tipleri (engine-spec §1.1 `frame_type`)

| frame_type | Ad | Gövde | Yön |
|---|---|---|---|
| `0x1B` | HEARTBEAT | boş **ya da** `echo_token u32` (başka uzunluk → reject) | tel → hub |
| `0x1C` | HEARTBEAT_ACK | gelenle aynı uzunluk; token aynen | hub → tel |
| `0x1D` | PLANE1_RPC **[YAMA engine §1.1]** | `AppRpcFrame` (§2.3) | iki yön |

Başka frame_type hub'a **gelmez** (ACK'ler Session bağlamında, E2E); gelirse sessiz drop.

### 2.3 AppRpcFrame (app-rpc §3, birebir) ve mesaj tablosu

```
method_id   u8    # bit7 = alışverişi BAŞLATAN taraf: 0 = telefon başlatır (istek VE yanıtı aynı method_id taşır),
                  #        1 = hub başlatır (DELIVER, ROUTE_MISS; yanıtı yoktur)
flags       u8    # bit0 response · bit1 error · diğerleri 0
request_id  u32   # telefon başlatıyorsa telefon üretir: süreç başında rastgele u32 (OsEntropy), her yeni istekte +1
                  # (2^32'de sarmalar — pratikte erişilmez); süreç yeniden başlayınca yeni rastgele başlangıç.
                  # Yeniden gönderimde AYNI (idempotency anahtarının parçası); hub başlatıyorsa 0.
                  # Eski bir request_id'nin yeniden kullanımı hub'da yalnız bayt-özdeş zarf + aynı alıcı + aynı tiple
                  # eşleşir (§2.4(c) anahtarı) = zaten aynı istektir; halka 64 girdidir, daha eskisi unutulmuştur.
payload_len u32   # ≤ MAX_DATAGRAM − 31
payload     bytes[payload_len]
```

| Alışveriş | method_id | flags | request_id | payload |
|---|---|---|---|---|
| CLIENT_BIND istek | `0x01` | `00` | R | §2.4(a) istek |
| CLIENT_BIND yanıt (meydan okuma) | `0x01` | `01` (response) | R | §2.4(a) yanıt, `error_code = CHALLENGE` |
| CLIENT_BIND hata | `0x01` | `03` (response+error) | R | `ErrorPayload` |
| BIND_PROOF istek | `0x0B` | `00` | R' | §2.4(b) istek |
| BIND_PROOF yanıt (bağlandı) | `0x0B` | `01` | R' | §2.4(b) yanıt, `bind_ok = 1`, imzalı |
| BIND_PROOF hata | `0x0B` | `03` | R' | `ErrorPayload` (imzasız) |
| STORE_MESSAGE istek | `0x05` | `00` | R'' | §2.4(c) istek |
| STORE_MESSAGE yanıt | `0x05` | `01` | R'' | §2.4(c) yanıt (`status` OK / PEER_UNBOUND / UNKNOWN_HANDLE — UNKNOWN_HANDLE iki durumda: `hub_handle ≠ 0` ve hat yok; **ya da** `hub_handle = 0` ve yeni-hat anahtarı tombstone'da, `assigned_handle = 0`) |
| STORE_MESSAGE hata | `0x05` | `03` | R'' | `ErrorPayload` (BAD_REQUEST / RATE_LIMITED / HUB_CAPACITY) |
| DELIVER | `0x81` | `00` | `0` | §2.4(d) |
| ROUTE_MISS | `0x84` | `00` | `0` | §2.4(e) |

`ErrorPayload { error_code u16; detail_len u16 = 0 }` (app-rpc §11 aynen; `detail` boş). Hangi durumun **yanıt alanı**, hangisinin **hata çerçevesi** olduğu §2.5'te. Her satırın altın vektörü vardır (§7).

### 2.4 Mesajlar

**(a) CLIENT_BIND `0x01` — 1. tur (durumsuz)**
```
istek  (tel→hub): device_cert_mode u8 = 0x00 · device_cert_ref bytes32 · presence_flags u8 (PoC: 0)
                  · client_nonce bytes16                                  [YAMA: client_nonce]
yanıt  (hub→tel): bind_ok u8 = 0 · mailbox_cursor u64 = 0 · revocation_seq u64 = 0
                  · error_code u16 = 0x0003 CHALLENGE · hub_nonce bytes16   [YAMA: 0x0003, hub_nonce]
```
`hub_nonce` (cookie) = `BLAKE2s-256(cookie_secret ‖ fam u8 ‖ src_ip (4|16 B) ‖ src_port u16 ‖ device_cert_ref ‖ client_nonce ‖ minute u32 ‖ bind_epoch u32)[0..16]`; **`fam` = `0x04`** (IPv4, `src_ip` 4 B) **ya da `0x06`** (IPv6, `src_ip` 16 B); çift yığınlı sokette IPv4-eşlemeli IPv6 adresi (`::ffff:a.b.c.d`) özetlenmeden önce IPv4'e **normalize edilir** (aynı kaynak her zaman aynı baytlar); `minute` = hub'ın açılıştan beri geçen dakikası; `cookie_secret` açılışta rastgele, saatte bir yenilenir, **bir önceki saklanır**; **`bind_epoch`** = o kimliğin uç kaydındaki sayaç (kayıt yoksa 0; her kabul edilen bind'de +1). Hub bu turda hiçbir şey **yazmaz** (uç kaydını yalnız okur). `device_cert_ref` dizinde yoksa **cevap yok**. `client_nonce` telefonun CSPRNG'sinden, her bind denemesinde **yeni**.

**(b) BIND_PROOF `0x0B` — 2. tur [YAMA app-rpc §4/§5 — yeni method, PoC-BIND]**
```
istek  (tel→hub): device_cert_ref bytes32 · client_nonce bytes16 · hub_nonce bytes16 · client_signature bytes64
yanıt  (hub→tel): bind_ok u8 = 1 · mailbox_cursor u64 = 0 · revocation_seq u64 = 0 · error_code u16 = 0 · hub_signature bytes64
hata   (hub→tel): ErrorPayload { BAD_COOKIE | POSSESSION_PROOF_FAIL | RATE_LIMITED | HUB_CAPACITY }  (imzasız)
```
```
client_signature = Ed25519_Sign(device_ed25519_sk,
    domain_sep("KAMA-HUB-BIND-V1") ‖ hub_ed25519_pub ‖ device_cert_ref ‖ client_nonce ‖ hub_nonce)   # TBS 112 B
hub_signature    = Ed25519_Sign(hub_ed25519_sk,
    domain_sep("KAMA-HUB-ACK-V1")  ‖ device_cert_ref ‖ client_nonce ‖ hub_nonce ‖ bind_ok ‖ error_code) # TBS 83 B
```
`domain_sep` = ccr §13.2 geleneği (ASCII ≤ 16 B + `0x00` dolgu). Doğrulama katı kip.

**Hub'ın işlem sırası (ikinci inceleme 1–2; TDD `on_bind2` birebir):**
1. `cert_ref` dizinde mi — değilse **sessiz**.
2. **Kayıp ACK tekrarı:** uç kaydı varsa ve gelen `(client_nonce, hub_nonce)` ucun **son kabul edilen** kanıtıyla aynıysa **ve** kaynak adres hem o kanıtın adresi hem ucun şu anki adresiyse → son başarı yanıtı bayt-özdeş yeniden gönderilir; **durum değişmez**. (Hub yalnız son kabul edilen kanıtı saklar: `last_accepted = {client_nonce, hub_nonce, addr, ack_bytes}`.)
3. **Cookie** doğrulaması, ucun **şu anki `bind_epoch`**'uyla (ucuz: ≤ 4 BLAKE2s; dakika ∈ {şimdi, bir önceki} × anahtar ∈ {geçerli, önceki}) — yanlışsa `BAD_COOKIE`; **hiçbir durum yazılmaz, hiçbir sayaç artmaz** (iddia edilen kaynak doğrulanmamıştır; adresi taklit eden biri o adrese kilit koyduramaz).
4. Kaynak adres **kilidi** (`bind_fail`): tutuluyorsa sessiz. Küresel `VERIFY_BUDGET` — yoksa `RATE_LIMITED`.
5. **İmza** — yanlışsa `POSSESSION_PROOF_FAIL` ve `bind_fail[src]` +1 (cookie geçtiği için bu adres gerçekten erişilebilir: ya sahibi ya yoldaki saldırgan).
6. **Kabul:** uç kaydı yoksa oluştur; adres farklıysa **taşı** (hatlar korunur, `by_addr` eski kayıt silinir); **`bind_epoch += 1`**; `last_accepted` güncellenir; imzalı yanıt.

**Eski/yeni bind sıralaması (ikinci inceleme 1):** sıralama hub'ın **kabul sırasıdır**. Her kabul `bind_epoch`'u ilerlettiği için, kabulden önce verilmiş **bütün** cookie'ler (2 dakikalık pencere içinde olsalar da) 3. adımda düşer: eski bir kanıt — gecikmiş, tekrarlanmış, halka taşırılmış fark etmez — yeni bind'den sonra **hiçbir zaman** kabul edilmez; hatırlanacak bir nonce listesi yoktur, taşacak bir halka da. "İlk kez görülen gecikmiş kanıt": kabulünden önce başka kabul olmadıysa o kanıt zaten **en yeni** bind'dir (telefon onu istemiştir); olduysa düşer. Telefonun aynı anda iki bind denemesi varsa (iki 1. tur, bir kabul): ikincisi `BAD_COOKIE` alır ve 1. turu yineler (C2) — son durum her zaman en son kabul edilen bind'dir. Bir kanıt en çok bir kez durum değiştirir.

**Kilit tablosu (ikinci inceleme 2):** `bind_fail` yalnız 5. adımda yazılır; en çok `BIND_FAIL_ENTRIES` girdi (LRU; dolunca en eski atılır), girdi `BIND_FAIL_HOLD` sonra silinir. Yanlış cookie seli (sahte adreslerden bile) tabloyu **büyütmez**.

**(c) STORE_MESSAGE `0x05` — zarf rölesi (depolamasız)**
```
istek  (tel→hub): hub_handle u32 (0 = yeni hat) · envelope_type u8 · recipient_dest_hash bytes32
                  · envelope_len u32 · envelope bytes[envelope_len]          [YAMA: hub_handle öne eklendi]
yanıt  (hub→tel): status u16 · assigned_handle u32                           (app-rpc §6.1 birebir)
hata   (hub→tel): ErrorPayload { BAD_REQUEST | RATE_LIMITED | HUB_CAPACITY }
```
Geçerli birleşimler (başkası → `BAD_REQUEST`):

| `hub_handle` | `envelope_type` | Anlam | Hub |
|---|---|---|---|
| 0 | `0x01 SESSION_INIT` | yeni oturum | tahsis + DELIVER |
| 0 | `0x05 RELAY_OPEN` | yeni medya hattı | tahsis + DELIVER |
| h | `0x02 SESSION_INIT_RESPONSE` | h hattındaki INIT'e cevap | DELIVER karşı uca |
| 0 · `0x02` — h · `0x01` — h · `0x05` — her şey · `0x03`/`0x04`/diğer | | geçersiz (medya hattının ACK'i hub'dan değil, E2E `MEDIA_LINK` ile — KR-17) | `BAD_REQUEST` |

Kurallar: `recipient_dest_hash` dizinde olmalı ve gönderenden farklı; `hub_handle ≠ 0` ise `recipient_dest_hash` **o hattın karşı ucu** olmalı (bulgu 4); `RELAY_OPEN` zarfı ≤ 64 B. `request_id` kuralı tek yerde: §2.3 (süreç başında rastgele, +1, u32 sarmalar).

**Tahsis anahtarı ve indeksi (KR-19; üçüncü tur 3).** Yeni hat isteğinin anahtarı `allocation_key = (gönderen dest_hash, request_id, recipient_dest_hash, envelope_type, BLAKE2s(envelope)[0..8])`. Hub bunu **isteğe ait bir geçmiş olarak değil, hattın bir alanı olarak** tutar:

- Her aktif hat (`links[h]`) kendisini açan `allocation_key`'i taşır; `key → handle` indeksi aktif hatların **tamamını** kapsar ve hat yaşadığı sürece **hiçbir bağımsız tahliyeyle** (LRU, zaman) kaybolmaz.
- Hat emekli olurken (§2.6 planı, küresel tahliye) tombstone'a `handle` ile birlikte `allocation_key` de taşınır; emekli-anahtar sorgusu tombstone'la **aynı kapasiteye** (`MAX_TOMBSTONES`) ve **aynı 7 günlük** saklama süresine bağlıdır.
- İndekslerin toplam boyutu `aktif hat sayısı + tombstone sayısı` ile sınırlıdır; ayrı bir istek geçmişi **yoktur**.

STORE `{0, tip, alıcı, zarf}` davranışı anahtara göre:

| Anahtar nerede | Yanıt | Hub ne yapar |
|---|---|---|
| **aktif hatta** | `{OK, h}` | aynı handle; alıcıya **yeniden DELIVER** (T3'ün varlık sebebi; kopya eleme alıcının kriptosunda) |
| **tombstone'da** | `{UNKNOWN_HANDLE, 0}` | tahsis **yok**, DELIVER **yok**: o deneme hub'da bitmiştir, istemci yeni deneme başlatır (C3) |
| **hiçbirinde** | §2.6 planı | yeni tahsis (`{OK, h}` + DELIVER) ya da PEER_UNBOUND / RATE_LIMITED / HUB_CAPACITY |

Aynı `request_id`, farklı içerik → farklı anahtar → "hiçbirinde" (yeniden başlamış telefon). Hub, bir anahtarı **kaç kez** gördüğünü saymaz ve LRU hatırlama süresine dayanan hiçbir iddia yoktur: UDP gönderim aralığı hub'a ulaşma aralığını garanti etmez.

**(d) DELIVER `0x81` (hub→tel)**
```
hub_handle u32 · sender_dest_hash bytes32 · envelope_type u8 · envelope_len u32 · envelope bytes[envelope_len]
                                                   [YAMA: hub_handle ve sender_dest_hash öne eklendi]
```
`sender_dest_hash` = hub'ın **bağlı kaynaktan** tanıklık ettiği gönderen (hub kanalı imzasız olduğu için yoldaki saldırgan bunu sahteleyebilir — beyanlı sınır 1; **hiçbir güvenlik kararı bu alana dayanmaz**: INIT/RESPONSE için kripto göndereni doğrular, medya hattı için bağlama E2E `MEDIA_LINK` ile yapılır, §4 C10). INIT'in alıcısı handle'ı buradan öğrenir; RESPONSE'un alıcısı için handle **atamadır** (§4 C4); RELAY_OPEN'ın alıcısı için yalnız **bilgidir** (sayaç; durum yaratmaz).

**(e) ROUTE_MISS `0x84` (hub→tel)**
```
hub_handle u32 · reason u8 (0x01 UNKNOWN_HANDLE)          [YAMA: link_ref bytes16 yerine]
```

**(f) RELAY_OPEN zarfı (envelope_type 0x05; medya hattı) [YAMA registry]**
```
envelope = open_nonce bytes16   (telefonun CSPRNG'sinden; 16 B; ≤ 64 B)
```
Hub için opak; alıcı için bilgi. İçinde **`call_id` yok** (telde açık taşınmaz); `open_nonce` yalnız idempotency anahtarının zarf özetini her açılışta farklı kılar ve `MEDIA_LINK{OPEN}` ile çapraz eşleşir (§4 C10).

### 2.5 Durum ve hata kodları (app-rpc §11 aileleri; PoC değerleri [YAMA])

| Kod | Ad | Nerede |
|---|---|---|
| `0x0000` | OK | yanıt alanı |
| `0x0003` | CHALLENGE [YAMA] | CLIENT_BIND yanıtı `error_code` |
| `0x0301` | PEER_UNBOUND [YAMA değer] | STORE yanıtı `status` |
| `0x0302` | UNKNOWN_HANDLE [YAMA] | STORE yanıtı `status` (mevcut hat yok **ya da** yeni-hat tahsis anahtarı emekli — KR-19, `assigned_handle = 0`); ROUTE_MISS `reason` 0x01 |
| `0x0105` | POSSESSION_PROOF_FAIL | ErrorPayload |
| `0x0106` | BAD_COOKIE [YAMA] | ErrorPayload |
| `0x0501` | HUB_CAPACITY [YAMA] | ErrorPayload |
| `0x0502` | RATE_LIMITED [YAMA] | ErrorPayload |
| `0x0601` | BAD_REQUEST [YAMA] | ErrorPayload |

Dizinde olmayan kimlik **kod almaz** (sessizlik).

### 2.6 Handle tahsisi (ccr §35.7 biçimi)

`hub_handle = BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4]`; **her deneme `alloc_counter`'ı ilerletir** (başarılı ya da değil — sonraki istek aynı adayları yeniden denemez; ikinci inceleme 3); `0`, aktif tabloda ya da tombstone'da olan değer atlanır; **8 denemede de çakışırsa tahsis başarısız → `HUB_CAPACITY`** (üstüne yazma yok; bulgu 5). `handle_secret` açılışta rastgele.

**Yeni hat açma — emeklilik planlayıcısı (KR-9; üçüncü tur 2). Hiçbir şey değişmeden önce plan, bütün denetimler geçince uygulama:**
1. **Mevcut/idempotent tahsis:** `allocation_key` aktif hattaysa → aynı handle, `{OK, h}`, yeniden DELIVER, bitti (alıcı hiç bind olmamışsa `{PEER_UNBOUND, 0}`; hat kalır); tombstone'daysa → `{UNKNOWN_HANDLE, 0}`, bitti. Hiçbirinde değilse ve alıcı bağlı değilse → `{PEER_UNBOUND, 0}`, bitti.
2. Yeni tahsis gerekiyor: `LINK_ALLOC_RATE` → yoksa `RATE_LIMITED`.
3. Boş bir **emeklilik planı** (küme) oluştur.
4. **Alıcının** hat sayısı `MAX_LINKS_PER_ENDPOINT` ise: yalnız **gönderen ile alıcı arasındaki ortak hatlardan** en uzun süredir kullanılmayanı plana ekle. Ortak hat **yoksa** → `HUB_CAPACITY`; mevcut hatlara dokunulmaz. (Üçüncü taraflarla olan hatlar bu dalda **seçilemez**: Carol'ın Bob'a isteği Alice–Bob hattını düşüremez; Alice'in Bob'a isteği aynı çiftin eski hattını değiştirebilir.)
5. Planın uygulanmış hâlini hesapla (`sayı(uç) − plandaki o uca ait hatlar`). **Gönderen** hâlâ kotadaysa gönderenin **kalan** hatlarından en uzun süredir kullanılmayanı plana ekle.
6. **Küresel** sınır için de plan sonrası sayıyı kullan: `links − |plan| ≥ MAX_LINKS` ise küresel en-uzun-boşta hattı (planda olmayan) plana ekle. Plan bir **küme**: aynı hat iki kez emekli edilmez.
7. Denetimler: yeni hat eklendiğinde **iki uç ve küresel tablo** sınırlar içinde mi; **bütün** emeklilikler tombstone kapasitesine sığıyor mu (`tombstones + |plan| ≤ MAX_TOMBSTONES`); kullanılabilir **aday handle** üretilebiliyor mu (üretici yukarıda; yan etkisi yalnız `alloc_counter`). Herhangi biri başarısızsa → `HUB_CAPACITY`.
8. **Ancak bütün denetimler başarılıysa:** plandaki hatlar emekli edilir (tombstone'a `handle + allocation_key`), yeni hat `allocation_key` ile yazılır, iki ucun sayacı +1, yanıt `{OK, h}`, alıcıya `DELIVER`.

**Garanti:** başarısız tahsis mevcut hatları, uç sayaçlarını ve tombstone içeriğini **değiştirmez**. Bu garantinin dışında kalanlar açıkça: `LINK_ALLOC_RATE` bütçesinin tüketilmesi ve aday üreticisinin ilerleyen `alloc_counter`'ı. Hat emekli olunca **her iki ucun** sayacı −1. Pasif sessizlikle hat silme **yoktur**; emeklilik yalnız bu planla ya da kapasite tahliyesiyle olur.

---

## 3. Hub davranışı (normatif; TDD §3 bunları işleyici başına sözde koda çevirir)

| # | Kural |
|---|---|
| H1 | **Demux ilk 4 bayt:** `0` → kontrol (§2.2; bilinmeyen tip → sessiz drop + sayaç); değilse veri. |
| H2 | **Dizin kapısı:** `CLIENT_BIND`/`BIND_PROOF`'ta `device_cert_ref` dizinde yoksa cevap **yok**. Dizin `hub.json`'dan: her cihaz için `dest_hash` + `cert` (152 B); `cert_ref = SHA-256(cert)`, `device_ed25519_pub = cert[0..32]`. |
| H3 | **Bind:** 1. tur hiçbir şey yazmaz (ucun `bind_epoch`'unu okur). 2. tur §2.4(b) sırasıyla (1 dizin → 2 kayıp-ACK tekrarı → 3 cookie[epoch] → 4 kilit + bütçe → 5 imza → 6 kabul); başarıda `endpoints[dest_hash] = {addr, cert_ref, bind_epoch+1, last_accepted, last_activity}` yazılır/güncellenir; aynı kimlik başka adresten → adres **taşınır**, hatlar korunur, `by_addr` eski kayıt silinir. **Bir adres en çok bir uca, bir uç en çok bir adrese bağlıdır:** kimlik B, o anda A'ya bağlı olan X adresinden geçerli kanıt verirse X'i **alır** (`by_addr[X] = B`), A **adressiz** kalır (`addr = None`; hatları ve tahsis anahtarları korunur; A'ya STORE → `PEER_UNBOUND`, A'ya veri → `drop_peer_unbound`) — A yeniden bind olunca düzelir (NAT port yeniden kullanımı; kaybolan cihazın adresi başkasına düşebilir). Son kabul kazanır; reddetme yok (reddetme, eski ucun adresi sonsuza dek kilitlemesi demekti). Kabulden önce verilmiş her cookie kabulle geçersizleşir; nonce listesi tutulmaz. |
| H4 | **Adres yalnız bind ile değişir.** İki sayaç, iki ayrı durum: **`src_unbound`** = kaynak adres hiçbir uca bağlı değil (`by_addr`'da yok; eski/taşınmış adresler dahil) → STORE/heartbeat/veri sessiz düşer; **`src_mismatch`** = kaynak adres **bir uca bağlı** ama o uç, datagramdaki hattın ucu değil → veri sessiz düşer (STORE{h} için aynı durum yanıtlıdır: `BAD_REQUEST`, H6). Yüke hiçbir durumda bakılmaz. |
| H5 | **STORE (yeni hat, §2.4(c) tablo):** alıcı dizinde değil ya da gönderenin kendisi → `BAD_REQUEST`; sonra §2.6 planlayıcısının **sekiz adımı o sırayla**: anahtar aktif hatta → aynı handle + yeniden DELIVER; anahtar tombstone'da → `{UNKNOWN_HANDLE, 0}`; alıcı bağlı değil → `{PEER_UNBOUND, 0}`; `LINK_ALLOC_RATE` → `RATE_LIMITED`; plan (alıcı kotası: yalnız ortak hat, yoksa `HUB_CAPACITY`; gönderen kotası; küresel); denetimler (iki uç + küresel, tombstone kapasitesi, aday) → biri başarısızsa `HUB_CAPACITY`, hiçbir şey değişmez; hepsi geçince emekli + yaz (hat `allocation_key` taşır) + yanıt `{OK, h}` + alıcıya `DELIVER{h, gönderen, tip, zarf}` — **her tekrarda yeniden teslim**. |
| H6 | **STORE (mevcut hat):** hat yoksa yanıt `UNKNOWN_HANDLE`; gönderen hattın ucu değilse ya da alıcı karşı uç değilse `BAD_REQUEST`; karşı uç bağlı değilse `PEER_UNBOUND`; değilse `DELIVER{h, gönderen, tip, zarf}` karşı uca, yanıt `{OK, h}`. |
| H7 | **Veri `[h][…]`:** hat yoksa → kaynak bağlıysa `ROUTE_MISS{h, UNKNOWN_HANDLE}` (`ROUTE_MISS_RATE`), bağsızsa sessiz; hat var, kaynak uçlardan biri → datagram **bayt bayt** karşı ucun adresine; `last_activity` güncellenir. Kuyruk yok. |
| H8 | **Heartbeat:** kaynak bağlı adres → aynı uzunlukta ACK, token aynen; `last_activity` tazelenir; slot yaratmaz, adres değiştirmez, canlılık kararı yok; bağsız kaynak → sessiz. |
| H9 | **Bellek:** `endpoints` dizin boyutuyla sınırlı, silinmez. `links` uç başına `MAX_LINKS_PER_ENDPOINT` (**her hat iki ucun da sayacında**; tavan iki uçta da denetlenir, §2.6), küresel `MAX_LINKS`; her hat `allocation_key` taşır ve `key → handle` indeksi aktif hatların tamamını kapsar (bağımsız tahliye yok). `tombstones` `MAX_TOMBSTONES` kapasiteli, girdi = `handle + allocation_key + retired_at`; girdi `HANDLE_TOMBSTONE` dolmadan **silinmez**; sığmayan emeklilik planı → `HUB_CAPACITY`, mevcut hatlar dokunulmaz (bulgu 5, ikinci inceleme 3). İndekslerin toplam boyutu `links + tombstones` ile sınırlı; istek geçmişi yok. `bind_fail` `BIND_FAIL_ENTRIES` LRU, girdi `BIND_FAIL_HOLD` sonra silinir. **Başka hiçbir zamanlayıcı tablo değiştirmez.** |
| H10 | **Yansıtma:** hub kendiliğinden paket üretmez; her çıkış bir girişe cevaptır ya da bağlı bir uca iletimdir. **Doğrulanmamış kaynağa** (henüz bind olmamış adres) giden her cevap **istekten küçük ya da eşittir**: CLIENT_BIND meydan okuması (66 B ≤ 81 B), BIND_PROOF hata yanıtları (35 B ≤ 159 B); bilinmeyen/bağsız kaynağın veri, STORE, heartbeat datagramları cevapsızdır. **Bağlı kaynağa** giden cevaplar (`ROUTE_MISS` 36 B — 5 baytlık veriye bile —, STORE hata/durum yanıtları, heartbeat ACK) bu kurala tabi değildir; yükseltme riski uç başına hız sınırlarıyla kapatılır (`ROUTE_MISS_RATE` 1/s, `STORE_RATE`; adres sahteciliği yalnız bağlı bir adresi taklit ederek en çok o hızda cevap üretebilir). |
| H11 | **Hız:** BIND_PROOF küresel `VERIFY_BUDGET` (cookie'den **sonra**); yanlış cookie **durum yaratmaz**; geçerli cookie sonrası imza başarısızlığı kaynak adrese yazılır, `BIND_FAIL_MAX`'ta o adres `BIND_FAIL_HOLD` boyunca sessiz (kimlik kilitlenmez; adresi taklit eden de kilitleyemez — ikinci inceleme 2); STORE `STORE_RATE`/uç; hat tahsisi `LINK_ALLOC_RATE`/uç; ROUTE_MISS `ROUTE_MISS_RATE`/uç. Veri hattına tavan yok (beyan). |
| H12 | **Dayanıklılık:** hiçbir girdi hub'ı düşürmez; bozuk/kısa/uzun datagram = sayaç + sessiz drop; yük, kimlik, anahtar loglanmaz. |

---

## 4. İstemci yükümlülükleri (A3 — `client-core` hub istemcisi)

| # | Yükümlülük |
|---|---|
| C1 | **Tek UDP soketi** (KR-14): bind, kontrol, veri, heartbeat, medya aynı soketten; demux handle ile (protector `by_wire_handle`); medya handle'ları protector'dan **önce** ayrılır (handle → bağlam; bağlam-önce). |
| C2 | Bind olmadan hiçbir şey gönderilmez. PoC-BIND (§2.4 a–b); `hub_ed25519_pub` provisioning dosyasından; hub imzası doğrulanmadan bind "tamam" sayılmaz; her denemede yeni `client_nonce`; kayıp yanıtta aynı kanıt **aynı adresten** bayt-özdeş yeniden gönderilir (en çok 3 kez, 1 s arayla); `BAD_COOKIE` → **1. tura dön** (yeni cookie; eski kanıt bir daha gönderilmez); aynı anda tek bind denemesi (ikincisi başlamadan birincisi bitirilir ya da terk edilir). |
| C3 | **INIT denemesi (KR-18, KR-19).** INIT = `STORE_MESSAGE{0, 0x01, peer, env}`; **T3 tekrarı aynı `request_id`, bayt-özdeş** (1 s × 2 … ≤ 30 s). `STORE_MESSAGE OK`, hub'ın isteği kabul ettiğini ve handle tahsis ettiğini gösterir; **karşı ucun INIT'i aldığını veya el sıkışmanın tamamlandığını göstermez.** Başlatan, aynı el sıkışma denemesinde aynı `request_id` ve bayt-özdeş INIT ile T3 tekrarını **sürdürür**. Tekrar yalnız şunlarla durur: **doğrulanmış RESPONSE kabul edildi** · bağlantı niyeti iptal edildi · deneme açıkça sonlandırılıp yeni denemeye geçildi. `StoreResp{OK, h}` gelince `AssignHandle(session, h)` yapılabilir; **INIT tekrar zamanlayıcısı durmaz**. Yanıtlar yalnız **ilgili bekleyen istekle** (`request_id`) eşleşiyorsa işlenir; sonlandırılmış denemelerin geç yanıtları (`StoreResp`, `DELIVER{RESPONSE}`) yeni denemeye **atanmaz**, sayılır. `PEER_UNBOUND`, `RATE_LIMITED`, `HUB_CAPACITY` → mevcut geri çekilme (≥ 5 s), başarı sayılmaz. **`{UNKNOWN_HANDLE, 0}`** → bu deneme hub'da bitmiştir: deneme **sonlandırılır**; niyet sürüyorsa mevcut yeniden bağlanma yoluyla **yeni kriptografik INIT ve yeni `request_id`** üretilir (eski INIT'in yalnız `request_id`'si değiştirilmez). **Çelişkili handle** (aynı denemede `StoreResp` ile `DELIVER{RESPONSE}` farklı `h` söylüyor): "yok say ve başarılı say" **değil** — deneme hata geçişiyle sonlandırılır (`attempt_handle_conflict`), mevcut S3 oturumuna farklı handle **zorla yazılmaz**, niyet sürüyorsa yeni deneme. **Deneme yaşı:** tamamlanmamış bir INIT ya da RELAY_OPEN denemesi `ATTEMPT_MAX_AGE`'i (§6) aşınca yalnız **o deneme** sonlandırılır; niyet sürüyorsa yeni deneme başlar. Bu kural kurulmuş oturumlara uygulanmaz ve pasif sessizlikle oturum kapatma değildir. |
| C4 | RESPONSE = `STORE_MESSAGE{h, 0x02, peer, env}`. Alımda `DELIVER{h, _, 0x02, env}`: önce `hub_of` ile yenileme eşlemesi; eşleşme yoksa `AcceptResponse{wire}` (KS-DEMUX-1 kriptoda) → kabul eden oturum için `AssignHandle(session, h)` (idempotent) — **STORE yanıtı kaybolmuş ya da gecikmişken gelen RESPONSE böyle eşlenir (bulgu 3)**; kabul eden oturum bir yenilemenin bekleyeni ise RESPONSE "held" sayılır (KARAR-2 cutover). **Yanıtlayan tarafta tekrar (KR-18):** aynı INIT'in (aynı bayt, aynı `h`) ikinci `DELIVER`'ı **yeni oturum açmaz**; yanıtlayanın hub istemcisi o el sıkışma için ürettiği RESPONSE baytlarını saklar ve `STORE_MESSAGE{h, 0x02, …}` ile **yeniden gönderir** (RESPONSE DELIVER'ı kaybolduysa toparlanma budur). Aynı INIT baytları **farklı** bir `h` ile gelirse yok sayılır ve sayılır (`init_handle_conflict`); yanıtlayanın yarım oturumu yeniden bağlanmaz. Üç kayıp ayrı ayrı test edilir: STORE yanıtı · INIT DELIVER'ı · RESPONSE DELIVER'ı — başarı ölçütü **iki uçta aynı handle, doğrulanmış oturum, iki yönde gerçek veri**. |
| C5 | `ROUTE_MISS{h}` → o handle **bırakılır**, oturum için yeni INIT; hub başına eş başına sınırlı deneme + geri çekilme (app-rpc §6.11.1 ruhu; sahte ROUTE_MISS DoS'u beyanlı). |
| C6 | Heartbeat yalnız **sessizlikte**: 8 s / 25 s / doze OS (KS-HB-1). **Kopma ilanı:** pasif sessizlik tek başına hiçbir şeyi düşürmez; "koptu" yalnız **cevapsız aktif yoklama** (3 × 1 s) sonrasıdır — yoklamayı OS sinyali, soket hatası, ARQ RTO yedeklemesi ya da heartbeat cevapsızlığı tetikler. Kaçırma sayacı yoktur. |
| C7 | "Koptu" → **yeniden bind** (aynı kimlik, yeni nonce); hub hatları koruduğu için **kurulmuş oturumlar devam eder** (60 s tam kesinti bu yoldur: aynı oturum, aynı handle; rebind handle'ı değiştirmez). **Tamamlanmamış açılış denemeleri** (INIT ya da RELAY_OPEN) yeniden bind sonrasında **yeni INIT / yeni `open_nonce` ve yeni `request_id` ile yeniden başlatılır** — hub yeniden başlamışsa eski tahsis varsayımı taşınmaz. Yeni INIT kurulmuş bir oturum için yalnız (a) OS `LinkChanged` (K4: adres değişti) ya da (b) `ROUTE_MISS` (hub yeniden başladı / hat emekli) ile. |
| C8 | HubChannel çerçeveleri `on_hub_frame()`'de; Session durumuna erişimsiz imza korunur (D1); `0x1D` → AppRpcFrame; `0x1C` → heartbeat eşleyici; başka tip → sayaç. Hata çerçeveleri (`flags.error`) yalnız yeniden deneme ipucudur; imzasız BIND hatası bind'i **düşürmez** (yoldaki saldırgan üretebilir) — yeniden deneme T3 kadansıyla. |
| C9 | `metrics()`: `last_heartbeat_age_s` dolu (hub'dan gelen son datagramın yaşı, her tür); `hub_frames_dropped` yalnız bilinmeyen tip için. |
| C10 | **Medya hattı — bağlama uçtan uca (KR-17).** `kama-node` API'si: `media_open(peer)` / `media_close(peer)`; olaylar `MediaLinkOpened{peer}` / `MediaLinkClosed{peer, reason}`; `media_send(peer, bytes)` hat açık değilse `MediaLinkNotOpen`. **Akış:** açan taraf `STORE_MESSAGE{0, 0x05, peer, open_nonce}` → `{OK, h}` → eşe **E2E, güvenilir** uygulama mesajı `MEDIA_LINK{op = OPEN, hub_handle = h, open_nonce}` (ARQ taşır, kaybolmaz); eş `(peer → h)` bağlar, `MEDIA_LINK{op = ACK, h}` döner ve `MediaLinkOpened` verir; açan ACK'te `MediaLinkOpened` verir; **medya yalnız ACK'ten sonra** akar. **RELAY_OPEN'ın tekrarı INIT'inkinden ayrıdır (KR-18):** `STORE_MESSAGE{0, 0x05}` T3 kadansıyla, aynı `request_id` ve aynı `open_nonce` ile **yalnız hub yanıtını** bekler; `{OK, h}` gelince hub tekrarı durur ve iş E2E `MEDIA_LINK`'e geçer (o da ARQ ile güvenilirdir). `{UNKNOWN_HANDLE, 0}` → bu açılış denemesi bitti: **yeni `request_id` ve yeni `open_nonce`** ile yeni açılış; mevcut E2E oturumu sırf bu nedenle **sıfırlanmaz**. Deneme yaşı `ATTEMPT_MAX_AGE` burada da geçerli. Hub'dan gelen `DELIVER{h, _, 0x05, open_nonce}` yalnız sayılır: **erken gelse, kaybolsa, sahte olsa etkisi yoktur**. **Glare:** iki taraf da OPEN göndermişse `dest_hash`'i küçük olanın `h`'si kalır; diğeri onu ACK'ler, kendi hattı için `MEDIA_LINK{op = CLOSE, h_kendi}` gönderir (hub'daki hat idle kalır, sırası gelince emekli). **Kopma:** `h` için `ROUTE_MISS` alan taraf `MediaLinkClosed{RouteMiss}` verir ve eşe `MEDIA_LINK{CLOSE, h}` gönderir; açan taraf (ya da `media_open`'ı yeniden çağıran) yeniden açar. **Tekrar:** OPEN'ın ARQ tekrarı aynı `h`; eş aynı `h` için ikinci OPEN'ı ACK'ler (idempotent); farklı `h` ile ikinci OPEN = yeniden açılış, eskisi kapanır. **Medya anahtarı** ayrı, henüz verilmemiş karar (A3 öncesi; seçenekler karar mesajında); hat açılışını değiştirmez. |

---

## 5. Güven modeli ve beyanlı sınırlar

| Konu | Durum |
|---|---|
| Telefon ↔ telefon | Değişmedi: Session Model V3, dizin = güven çapası. Hub E2E anahtarını bilmez. |
| Telefon → hub | PoC-BIND: cihaz Ed25519 imzası + durumsuz cookie (adres + `bind_epoch` bağı) + taze nonce. Her kabul edilen bind önceki bütün cookie'leri geçersizleştirir → eski kanıt adresi geriye taşıyamaz (nonce listesi yok). Dizinde olmayan kimlik hub'ın varlığını öğrenmez. |
| Hub → telefon | Hub Ed25519 imzası, yalnız başarılı bind yanıtında. |
| **Beyanlı sınır 1** | Bind sonrası kontrol mesajları **imzasız**: kaynak sahteciliğiyle gereksiz INIT/ROUTE_MISS üretilebilir, `sender_dest_hash` ve `DELIVER{RELAY_OPEN}` sahtelenebilir → yalnız **erişilebilirlik** (yoldaki saldırgan zaten düşürebilir); içerik, adres (H4) ve medya hattı bağlaması (E2E `MEDIA_LINK`, C10) etkilenmez. Yoldaki saldırgan medya datagramlarını yanlış hatta yönlendiremez: alıcı yalnız E2E ile bağladığı `h`'yi kabul eder, hub yalnız hattın iki ucuna iletir. Kapanış: M4 hop anahtarı. |
| **Beyanlı sınır 2** | Path validation yok: NAT/port değişimi → istemci yoklamayla fark eder, yeniden bind (2 RTT). |
| **Beyanlı sınır 3** | Hub yeniden başlarsa tüm oturumlar yeniden kurulur. INT-03 (5) ölçer. |
| **Beyanlı sınır 4** | Hub iletişim grafiğini öğrenir (ADV-5). |
| **Beyanlı sınır 5** | Veri hattında bant tavanı yok; tombstone tablosu doldurulursa (`MAX_TOMBSTONES`; 1/s tahsis hızında ≥ 18 saat sürekli açılış) yeni hat açılamaz — dizindeki bir cihazın işi olur, `hub stats` gösterir. |
| **Beyanlı sınır 6** | Yoldaki (on-path) saldırgan bind yanıtını düşürebilir, sahte hata üretebilir; istemci T3 kadansıyla yeniden dener. |
| **Beyanlı sınır 7** | Erişilebilir adreslerden (gerçek cookie alıp geçersiz imza yollayan) bir saldırgan kümesi küresel `VERIFY_BUDGET`'ı tüketebilir → dürüst bind'ler `RATE_LIMITED` alır, yeniden dener (erişilebilirlik). Adres başına kilit bunu adres başına 10 denemeyle sınırlar; kapanış M4 (Noise IK, maliyet saldırgana). |

---

## 6. Sabitler (G-S4 ile kesinleşir; TDD adıyla kullanır)

| Ad | Değer | Not |
|---|---|---|
| `MAX_DATAGRAM` | 1280 B | |
| `MAX_LINKS` | 8192 | küresel; dolunca en-uzun-boşta tahliye |
| `MAX_LINKS_PER_ENDPOINT` | 64 | her hat iki uçta sayılır; plan: alıcı dolu → yalnız gönderen–alıcı ortak hattı (yoksa `HUB_CAPACITY`), gönderen dolu → kendi kalan en-uzun-boşta hattı (§2.6) |
| `MAX_TOMBSTONES` | 65 536 | sığmayan emeklilik planı → `HUB_CAPACITY`; erken silme yok |
| `HANDLE_TOMBSTONE` | 7 gün | KS-HANDLE-1 |
| (tahsis anahtarı indeksi) | `links + tombstones` ile sınırlı | ayrı sabit yok; `IDEMPOTENCY_RING` kaldırıldı (KR-19) |
| `ATTEMPT_MAX_AGE` | 1 saat | tamamlanmamış INIT / RELAY_OPEN denemesinin azami yaşı; yalnız o deneme biter (C3, C10); kurulmuş oturuma uygulanmaz |
| `bind_epoch` | u32, uç başına, 0'dan | her kabul edilen bind'de +1; cookie girdisi |
| `COOKIE_ROTATE` / `COOKIE_WINDOW` | 1 saat / 2 dakika | önceki anahtar saklanır |
| `VERIFY_BUDGET` | 2000 doğrulama/s (küresel) | cookie'den sonra |
| `BIND_FAIL_MAX` / `BIND_FAIL_HOLD` / `BIND_FAIL_ENTRIES` | 10 / 60 s / 4096 | **kaynak adres** başına; yalnız geçerli cookie sonrası; LRU |
| `STORE_RATE` | 10/s, patlama 20 | uç başına |
| `LINK_ALLOC_RATE` | 1/s, patlama 10 | uç başına (aktif anahtara isabet eden tekrarlar tahsis değildir); başarısız tahsiste bütçe **harcanır** (garanti dışı) |
| `ROUTE_MISS_RATE` | 1/s | uç başına |
| `STATS_PERIOD` | 10 s | |
| `RELAY_OPEN_MAX_ENVELOPE` | 64 B | §2.4(f); PoC zarfı 16 B (`open_nonce`) |
| Heartbeat kadansı | 8 s / 25 s / OS | istemci; KS-HB-1 |

---

## 7. Uyum (A2)

- `crates/kama-contracts/src/s4.rs`: §2 tipleri + fixed-width codec + TBS/cookie bayt birleştiricileri (kripto yok) — sıfır bağımlılık.
- Python referansı `ref/hub/gen_vectors.py` → `vectors/hub/`: §2.3 tablosundaki **her satır** (istek, yanıt, hata çerçevesi ayrı vektör) · `cookie` (epoch 0 ve epoch 7 örneği; `fam = 0x04` IPv4, `fam = 0x06` IPv6, IPv4-eşlemeli IPv6 → IPv4 normalizasyonu örneği) · `bind-proof-tbs/sig` · `bind-ack-tbs/sig` · `store` (üç geçerli birleşim + geçersiz birleşimler negatif) · `deliver` (INIT, RESPONSE, RELAY_OPEN) · `relay-open-envelope` (16 B) · `route-miss` · `heartbeat` (0 B, 4 B) · `hub-frame` · `handle-alloc` (sayaç ilerlemesi; çakışma atlama; 8 başarısız deneme → hata, sayaç 8 ilerlemiş) · `idempotency-key` · negatifler. Rust hiçbir beklenen baytı üretmez.
- Harness `crates/kama-integration/tests/s4_*` (sahte hub ↔ gerçek istemci, sahte istemci ↔ gerçek hub) — vaka listesi H1–H12 + C2–C7 + **kimlik kilitleme denemesi (H11; kimlikle ve sahte kaynak adresle)** + **epoch tekrar vakaları (H3)** + **kilit tablosu sınırı (H9)** + aşağıdaki **kapanış vakaları** (üçüncü tur 5). El sıkışma vakalarının başarı ölçütü yalnız sayaç ya da `OK` yanıtı **değildir**: **iki uçta aynı handle + doğrulanmış oturum + iki yönde gerçek veri iletimi** aranır; sonlu paket kaybından sonra trafik yeniden ulaşınca tamamlanma doğrulanır. `MEDIA_LINK` vakaları A3'te (C10; E2E mesaj hub harness'ının dışında).

| Kapanış vakası | Başlangıç | Düşürülen / geciktirilen | Beklenen son durum |
|---|---|---|---|
| K1 | Alice→Bob INIT; hub çalışıyor | `StoreResp` ulaşır, **ilk `DELIVER{INIT}` kaybolur** | T3 tekrarı → hub aynı `h` ile yeniden DELIVER → RESPONSE → iki uçta `h`, oturum doğrulanmış, iki yönde veri |
| K2 | aynı | `StoreResp` ulaşır, **ilk `DELIVER{RESPONSE}` kaybolur** | Alice T3 ile INIT'i yineler (StoreResp durdurmamıştır) → Bob aynı INIT için **RESPONSE'u yeniden gönderir** (yeni oturum açmaz) → kurulur |
| K3 | aynı | **`StoreResp` kaybolur**, `DELIVER{RESPONSE}` önce ulaşır | RESPONSE'taki `h` doğru oturuma **atama** olur; sonra gelen `StoreResp{OK, h}` aynı `h` (idempotent); oturum kurulmuş |
| K4 | Alice–Bob: 1 kurulu oturum + 63 kapanmış medya hattı (ikisi de 64'te) | — | Alice'in yeni `RELAY_OPEN`'ı: plan = Alice–Bob ortak en-uzun-boşta kapalı medya hattı → yeni hat açılır; **iki uç da 64'ü aşmaz**; kurulu oturum dokunulmamış |
| K5 | Bob 64 hatta, Carol'ın Bob'la ortak hattı yok | — | Carol→Bob `HUB_CAPACITY`; **diğer çiftlerin hatları korunur** (Alice–Bob dahil), sayaçlar değişmez |
| K6 | Emeklilik planı var ama tombstone dolu **ya da** 8 aday çakışıyor | — | `HUB_CAPACITY`; mevcut hatlar, sayaçlar, tombstone içeriği **aynen**; yalnız hız bütçesi ve `alloc_counter` ilerlemiş |
| K7 | Alice'in aktif tahsisi (anahtar K) | arada 200 başka hat açılıp kapanır (eski LRU sınırını aşar) | K yinelenir → **hâlâ aynı `h`**; testte hattın gerçekten aktif kaldığı (veri geçer) doğrulanır |
| K8 | Bob eski INIT'i görmüş (yarım oturum, `h`); hub hattı emekli eder | — | Alice eski isteği yineler → `{UNKNOWN_HANDLE, 0}` → deneme biter → **yeni kriptografik INIT, yeni `request_id`** → Bob yeni INIT'e yeni yarım oturum → iki uçta aynı **yeni** `h`, veri iki yönde |
| K9 | Alice yeni denemeye geçmiş (deneme 2) | eski denemenin **gecikmiş `StoreResp`/`RESPONSE`'u** sonra gelir | yeni denemeye atanmaz (sayaç); deneme 2 kendi `h`'siyle kurulur |
| K10 | Tamamlanmamış INIT denemesi `ATTEMPT_MAX_AGE`'i aşar (sahte saat); aynı telefonda kurulu başka oturum var | — | yalnız o deneme biter, niyet sürüyorsa yeni deneme başlar; **kurulu oturum kapanmaz**, handle'ı değişmez |
| K11 | Alice–Bob kurulu oturum (`h`) | 60 s tam kesinti → yeniden bind | hub `h`'yi korumuştur; rebind **handle'ı değiştirmez**; veri `h` üzerinden iki yönde devam; tamamlanmamış deneme varsa yeni INIT ile yeniden başlar |
- Kapı: S4 **FROZEN** ("G-S4 GEÇTİ") = `s4.rs` derleniyor + vektörler Python'dan + `--target mock` harness yeşil + provisioning v1 + **§8'deki bütün satırlar** (Y1–Y12; Y11 A2.1'de tamamlanır) main'de. `--target real` Y2 ile; FROZEN'ı beklemez.

---

## 8. Spec yamaları (A2.6; YAMA KAYDI; üstü çiz, silme; word_coverage)

| # | Dosya / yer | Yama |
|---|---|---|
| Y1 | engine-spec §1.1 | `0x1D PLANE1_RPC`; 0x1E/0x1F rezerve (HANDLE_ASSIGN/ACK, M4) |
| Y2 | app-rpc §3 | bit7 = "alışverişi başlatan"; hub→tel mesajlarda `flags = 0`, `request_id = 0`; HubChannel'da taşınma = engine §1.1 `0x1D`; **hata yanıtı = `ErrorPayload` (aynen), uygulama durumları sabit yanıt alanında (§2.5 ayrımı)** |
| Y3 | app-rpc §4, §5 | PoC-BIND: `client_nonce`, `hub_nonce` + `0x0003`; yeni method `0x0B BIND_PROOF`; "Noise IK possession" PoC'de "cihaz Ed25519 imzası"; cookie girdisinde `bind_epoch`; işlem sırası (§2.4(b) 1–6); başarısızlık muhasebesi yalnız geçerli cookie sonrası, kaynak adrese, sınırlı tablo; kayıp ACK = son kabul edilen kanıtın tekrarı |
| Y4 | app-rpc §6.1 | `STORE_MESSAGE` başına `hub_handle`; geçerli (handle, tip) tablosu; `0x05 RELAY_OPEN` + zarf (`open_nonce`); tahsis anahtarı hattın alanı, tombstone'a taşınır, `{UNKNOWN_HANDLE, 0}` (KR-19); emeklilik planlayıcısı (§2.6 sekiz adım); `OK` = tahsis, el sıkışma değil (KR-18); "her leg bağımsız handle" → tek hub'da aynı değer |
| Y5 | app-rpc §6.11 | `DELIVER` başına `hub_handle` + `sender_dest_hash`; `ROUTE_MISS{hub_handle, reason}`; `SESSION_REBIND` PoC'de yok |
| Y6 | app-rpc §11 | PoC kodları (§2.5) |
| Y7 | ccr §35.16 | bilinmeyen handle: bağlı kaynağa ROUTE_MISS, yabancıya sessiz |
| Y8 | transport-tdd §2 | `hub_handle` "NAT-robust" → PoC'de path validation yok |
| Y9 | PoC-KAPSAM-001 **EK-4** | K5 notu: adres yalnız bind ile; INIT = bağlı adresten STORE |
| Y10 | PROVISIONING-FORMAT v1 | `hub.ed25519_pub`, `hub.json`, `--probe` |
| Y11 | KM-SEAM-TH-001-TASLAK | SUPERSEDED bandı |
| Y12 (v1.2; **A2.6'da tamamlanır**) | uygulama mesaj protokolü (`application-message-protocol.md`) — belge ve codepoint kararı A2.6'da; tür kaydı `s1.rs`'teyse DUR-S2-A2.6-MEDIA (KR-17 istisnası yalnız o kayıt satırı için, Abkarada'nın yazılı DUR cevabıyla; genel FROZEN muafiyeti değil) | yeni uygulama mesajı **`MEDIA_LINK`** (Reliable): `op u8 (0x01 OPEN · 0x02 ACK · 0x03 CLOSE) · hub_handle u32 · open_nonce bytes16` (+ karar (c) seçilirse `media_key bytes32`, yalnız OPEN'da); signaling 0x09'dan ayrı, Kama'nın kendi mesajı; medya blobu okunmaz |

---

## 9. Kapsam dışı (PoC)

Plane 0 Noise IK/KK (hop anahtarı, M4) · HANDLE_ASSIGN/ACK çerçeveleri · extended zarf / h_ext · mailbox (STORE/FETCH/ACK_RANGE) · SESSION_REBIND / SESSION_INIT_REF · hop-ARQ · path validation · çoklu hub, failover, DHT · revocation/zincir doğrulama · TLS/HTTP/metrik sunucusu · IPv6 testi.

---

## Ek — v1 → v1.1 değişiklikleri (bağımsız inceleme, 2026-10-14)
1 bind kilitleme (kimlik → kaynak adres; cookie önce) · 2 kabul edilmiş nonce halkası · 3 RESPONSE DELIVER'ı = handle ataması, üç kayıp vakası · 4 idempotency anahtarı + geçerli birleşim tablosu + karşı uç denetimi · 5 tombstone/hat tavanları, tahsis hata dalı · 6 bit7 anlamı, `ErrorPayload`, mesaj tablosu · 7 `sender_dest_hash`, RELAY_OPEN zarfı ve C10 · (8 Deploy TDD'de) · 9 (backlog A1/A2 Done) · C6 "pasif sessizlik" ifadesi · C7 "koptu = yeniden bind, oturum devam eder".

## Ek — v1.1 → v1.2 değişiklikleri (bağımsız inceleme ikinci tur, 2026-10-14)
1 nonce halkası **kaldırıldı** → cookie `bind_epoch`'a bağlı (her kabul önceki cookie'leri geçersizleştirir), hub yalnız son kabul edilen kanıtı saklar (kayıp ACK); eski/yeni sıralama = kabul sırası · 2 yanlış cookie durum yaratmaz; kilit yalnız geçerli cookie sonrası, tablo `BIND_FAIL_ENTRIES` LRU · 3 yeni hat açma sırası (plan → aday → emeklilik → yazma); alıcı kotası → `HUB_CAPACITY` emekliliksiz; sayaç her denemede ilerler; iki uç sayacı · 4 tek `request_id` kuralı (§2.3); idempotency LRU + değişmez; "ilk atama kazanır", geç `StoreResp` yok sayılır · 5 KR-17: medya hattı bağlaması E2E `MEDIA_LINK{OPEN/ACK/CLOSE}`; RELAY_OPEN zarfı = `open_nonce`, `call_id` telde yok; glare/kopma/tekrar C10; beyanlı sınır 1 daraltıldı; Y12 · (6 Deploy TDD'de) · 7 beyanlı sınır 7 (bütçe tüketimi).

## Ek — v1.2 → v1.3 değişiklikleri (üçüncü tur, 2026-10-14)
1 **INIT tekrarı** (KR-18, C3/C4/C10): `StoreResp OK` = tahsis, el sıkışma değil; T3 tekrarı doğrulanmış RESPONSE'a kadar sürer; yanıtlayan tekrar INIT'e RESPONSE'u yeniden gönderir; RELAY_OPEN tekrarı yalnız hub yanıtını bekler; `ATTEMPT_MAX_AGE` 1 saat; yeniden bind sonrası tamamlanmamış denemeler yeni INIT/`open_nonce` ile · 2 **kota planlayıcısı** (KR-9, §2.6 sekiz adım): alıcı dolu → yalnız gönderen–alıcı ortak hattı, yoksa `HUB_CAPACITY`; plan küme; bütün denetimler geçmeden uygulama yok; garanti dışı yalnız hız bütçesi ve `alloc_counter` · 3 **tahsis anahtarı indeksi** (KR-19): anahtar hattın alanı, tombstone'a taşınır; `IDEMPOTENCY_RING` ve "64 > 40" kaldırıldı; `{UNKNOWN_HANDLE, 0}` yeni-hat anahtarı emekliyse; istemcide eski deneme biter → yeni kriptografik INIT + yeni `request_id`; "ilk atama kazanır" **kaldırıldı** → çelişki = hata geçişi, yeni deneme · 4 **Y12** A2.6 paketinde; FREEZE koşulu "§8'deki bütün satırlar"; KR-17 istisnası yalnız kayıt satırı · 5 kapanış vakaları K1–K11 (§7) ve başarı ölçütü (aynı handle + doğrulanmış oturum + iki yönde veri).

**Aşılan v1.2 kararları (artık normatif değil):** "tekrar `StoreResp` gelince durur" · "ilk atama kazanır / `handle_conflict_ignored`" · `IDEMPOTENCY_RING = 64` ve LRU-isabet değişmezi · "alıcı kotası dolu → hemen `HUB_CAPACITY`" · "§8 Y1–Y11" sayımı.

## Ek — v1.3 → v1.3.1 (Yaaz'ın uygulama soruları, 2026-10-15)
1 cookie `fam`: `0x04`/`0x06`, IPv4-eşlemeli IPv6 normalize (§2.4(a)) · 2 aynı adresten ikinci kimlik: son kabul adresi alır, önceki uç adressiz kalır, hatları korunur (H3) · 3 yansıtma kuralı: "cevap ≤ istek" yalnız doğrulanmamış kaynağa; bağlı kaynağa cevaplar uç başına hız sınırlı (H10) · 4 `src_unbound` / `src_mismatch` tanımları (H4). Davranış değişikliği yalnız 2'de (önceden tanımsızdı).
