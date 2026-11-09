# T1 — ince şeritler ve oynatma tamponu (YOL3-T1, gösterim notu)

**Sürüm** 1 · 2026-11-09 (G0) · **Dal** `yol3/t1-seritler` · **Taban** `origin/main` `5de1ce3f`
(`crates/` `a858f3fc` ile aynı; satır numaraları bu ağacındır) · **İş emri** ISEMRI-YOL3-T1 v1.
Etiket: *gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta.* Gösterim düğümünde
`runtime.fec = None`.

## 1. Amaç ve kapsam

18 Kas GÖSTERİM-α için: (a) S1 `send(.., class)` (EK-1 D-09; S1 v2), (b) `client-core`
`lanes.rs`: sınıf başına sunum kanalı, motora kabul sırası, dosya penceresi W, (c) `kama-app`
`playout.rs`: sabit gecikmeli oynatma tamponu. Tel, altın vektör, `KamaV1Profile`, UniFFI ve
`adaptive-transport` DEĞİŞMEZ. Aralık'a: sınıf başına `TransportCapabilities`/`AmpLimits`,
`PttBundle.seq`/`PttGap`, sınıf öneki ve (oturum, sınıf) Reorder (T1-2b), `accept_v2` (T1-2a),
canlı PTT (T1-5), T1-3 tam eşlemesi.

## 2. Sınıf ve şerit

Sınıf = EK-1 D-08 `TrafficClass {Be 0, Ctrl 1, Sig 2, Bk 3, Vo 4, VoRel 5}`; yalnız S1
parametresi, telde yok. `connect(early_unit)` imzası değişmez; erken birim BE sayılır ve
`activate`'te şeritsiz, oturumun ilk birimi olarak uca verilir (`transport.rs:1001-1006`).

| çağrı noktası | kayıt | sınıf | şerit |
|---|---|---|---|
| `messaging.rs:633-635`, `:1097-1099` | TEXT (ilk sunum, yeni oturumda yeniden sunum) | Be | TEXT |
| `messaging.rs:1126-1128` | RECEIPT | Ctrl | TEXT |
| `file_transfer.rs:1260` | FILE_RECEIPT | Ctrl | TEXT |
| `file_transfer.rs:1521` | FILE_BEGIN/CHUNK/END/ABORT (tek nokta) | Bk | FILE |
| `ptt.rs:961` (`send_record`) | PTT_OPEN/MEDIA_BUNDLE/PTT_CLOSE | VoRel | PTT |
| `signaling.rs:427`, `:584` | SIGNALING_BLOB | Sig | SIG |

Eşleme (KK-T1-04): `{Vo, VoRel}`→PTT · `Sig`→SIG · `{Be, Ctrl}`→TEXT · `Bk`→FILE; şerit içi
FIFO; şerit birim baytını ya da kayıt türünü OKUMAZ (v4 K3; EK-1 D-04). Sapma (KK-T1-03):
FILE_ABORT gösterimde `Bk` (BEGIN/CHUNK ile aynı tek çağrı noktası); Aralık T1-3'te `Ctrl`.

## 3. Kabul sırası, W ve teyit

Pompa her turda şeritleri PTT > SIG > TEXT > FILE sırasıyla boşaltır; uca verme yalnız uç
`QueueFull` dönünce durur. W = 5 "teyitsiz dosya birimi" (`LaneConfig::file_window_units`,
runtime parametresi): FILE şeridi uca verdiği ve teyidi gelmemiş birim kimliklerini tutar (≤ W).
Birim kimliği = `Sess.sent` (`transport.rs:336-338`; veri yolu kimliği sırayla verir,
`runtime.rs:1920`, `:2452-2453`). Teyit kaynağı uç başına `acked_rx`: kanal
`max(offer_queue, max_inflight_units)` = 4096 (`runtime.rs:1617`), olay `try_send` ile, dolunca
SESSİZCE düşer (`runtime.rs:2136`, `:2145`); üründe tüketicisi yok (`runtime.rs:1707-1709`,
`git grep "\.acked()"` yalnız bench/örnek/test). Tüketilmeyen kanal 4096 teyitten sonra W'yi
kilitlerdi → şeritler açıkken `drive()` her ucu yeni `ClientHandle::try_acked()` ile boşaltır
(KK-T1-07). Yedek (KK-T1-08, yalnız G2 akşamına dek çalışmazsa): pencere birim uca verildikten
1 srtt sonra serbest (`session_live_diags`, `transport.rs:598`; örnek yoksa 300 ms, sayılır),
etiket "W zaman tabanlı (teyit değil)".

## 4. Motor kuyruğu ≥ 1 BDP (KK-T1-09)

BDP_gösterim = 12 500 B/s × 0,3 s = 3750 B. W·S = 5 × 1013 B = 5065 B ≥ 3750 B
(S = birim tavanı, `runtime.rs:245`). W = 4 → 4052 B (alt sınır); W = 3 → 3039 B şartı bozar.
mini-Ö'de ACK'li birim kaybı görülürse geri dönüşün ilk adımı W = 4. TEXT'in motor kuyruğunda
önünde en çok W dosya birimi olur (5 × 1013 B / 12 500 B/s ≈ 0,41 s yayın süresi + kurtarma).

## 5. Geri basınç ve sınırlar (KK-T1-05, KK-T1-10)

`ClientTransportConfig.lanes: Option<LaneConfig>`; `product()` = `None` (bugünkü yol).
`LaneConfig { file_window_units: 5, caps: [PTT 64, SIG 64, TEXT 256, FILE 2·W] }`; doğrulama
tipli hatayla (W ≥ 1, her tavan ≥ 1, toplam ≤ `offer_queue` = 1024). Şerit tavanı dolu →
mevcut `TransportError::Backpressure` (yeni hata yok); kama-app tavrı değişmez (TEXT
`forget_unsent` + hata, `messaging.rs:633-639`; RECEIPT tek deneme; FILE pump'ta bekler; PTT
çağırana hata). Şeritte süresi dolan DeadlineBounded birim uca verilmeden düşer, `lane_expired`
sayılır (bugün bütün kama-app noktaları Reliable).

## 6. Yenileme ve S1 K7 (KK-T1-06 (iv))

Kendi yenilememizde RESPONSE eski oturumun ucu boşalana dek tutulur (`outstanding() == 0`,
`transport.rs:787-791`); şeritler açıkken ek şart "o oturumun şeritleri boş". `renew_drain_cap`
dolunca kesinti yine olur. Şeritler oturum başınadır: birim yeni oturuma TAŞINMAZ; supersede
edilmiş oturumun şeridi ucu yaşadıkça uca verilmeye devam eder, `Closed`'da kalan birimler düşer
ve sayılır (S1 K7: "kalan birimler eski oturumda kalır").

## 7. Oynatma tamponu (KK-T1-11)

`crates/kama-app/src/playout.rs`; `Ptt::set_playout(Option<PlayoutConfig>)`, `None` varsayılan =
bugünkü davranış. Akış (eş, `stream_id`) başına: `PttStarted` hemen; ilk demet a0'da vardıysa ilk
`PttAudio` a0 + T'de (T ∈ [1,5; 2,5] s, `Default` 2,0 s, aralık dışı tipli hata); sonrakiler
`bundle_ms` aralıkla; geç demet beklenir, ATLANMAZ, gelince hemen verilir ve takvim ona yeniden
çapalanır (`playout_stalls`, `playout_stall_ms`); `PttEnded` yalnız tampon boşalınca, en erken
son demetin verilişi + `bundle_ms`'te (KK-T1-16). Saat `Ptt`'nin enjekte saati (`ptt.rs:439`).
Akış başına tavan 256 demet; aşılırsa en eski demet beklemeden verilir (düşürme yok,
`playout_overflow`). Kotlin tarafı: Ek A.

## 8. Sınıflar arası sıra varsayımları (KK-T1-13; koddan)

| çift | kod atfı | korunur mu | önlem |
|---|---|---|---|
| FILE_BEGIN → CHUNK → END | `file_transfer.rs:74-75` ("aynı oturumda `FILE_BEGIN` her zaman önce gelir") | evet | aynı sınıf (Bk), aynı şerit |
| FILE_ABORT ↔ BEGIN/CHUNK | `file_transfer.rs:68-71`, `:1521` | evet | KK-T1-03: ABORT `Bk` |
| PTT_OPEN → MEDIA_BUNDLE → PTT_CLOSE | `ptt.rs:26-31`, `:124-126` (OPEN öncesi/CLOSE sonrası demet DROP) | evet | aynı sınıf (VoRel) |
| TEXT ↔ TEXT (`msg_id` dedup penceresi) | `messaging.rs:158` | evet | aynı sınıf (Be) |
| RECEIPT ↔ TEXT | `messaging.rs:1112-1128` (eşin mesajına alındı; yön ters) | varsayım yok; yine de aynı şerit | KK-T1-04: Ctrl → TEXT |
| FILE_RECEIPT ↔ FILE_* | `file_transfer.rs:82-90` (alındı alıcının, ABORT gönderenin; yönler ayrı) | varsayım yok | Ctrl → TEXT |
| SIGNALING ↔ diğerleri | `signaling.rs:63`, `:136` ("sıra garanti DEĞİLDİR") | varsayım yok | — |
| yeniden sunum (yeni oturum) | `messaging.rs:1097-1099`; `ptt.rs:589`, `:631`, `:904` (yeniden PTT_OPEN, D5) | evet | sınıf içi; şerit oturum başına |
| TEXT ↔ FILE, PTT ↔ FILE, PTT ↔ TEXT | her modül yalnız kendi kayıt türlerini işler (`AppMessage::*` kullanımı modül başına ayrık) | varsayım yok | — |

Çapraz sınıf varsayımı bulunmadı; bulunursa o çift çağrı noktasında aynı sınıfı alır.

## 9. Bilinen sınırlar

Oturum geneli Reorder (`transport.rs:383-411`) dosya kaybında PTT'yi kurtarma turu kadar
bekletir (≈ 0,5–0,9 s); T bunu örter. W = 5'te bu olayın sıklığı ölçülmedi (mini-Ö ölçer);
sınıf başına Reorder T1-2b, Aralık. W teyide bağlı: kayıp dosya sunumunu yavaşlatabilir — bu
motorun send_rate'i değildir (INV-RATE-1 motor kuralıdır, motora dokunulmaz); fotoğraf ≤ 30 s
ölçütünü mini-Ö ölçer. Tampon açıkken PTT olaylarıyla diğer olaylar arasındaki sıra değişir
(PTT T kadar geç); kama-node/Kotlin bu sırayı varsaymaz (Ek A satır 9).

## 10. Bayraklar ve testler

Varsayılanlar `lanes = None`, `playout = None` (ürün yolu değişmez; davranış-nötrlük T-L6,
T-P5). Gösterim düğümüne geçiş yalnız Ö'nün test-util kancası / kama-node `goster` özelliği;
T1 kama-node'a ve kama-mobile'a dokunmaz. Testler: T-L1…T-L10 (`client-core`), T-P1…T-P8
(`kama-app`, sahte saat); S1 v2 için seam mock/real + drift kapısı.

## Ek A — Rust ↔ Kotlin PTT oynatma sözleşmesi v1 (2026-11-09; Hasan, H2)

1. Olaylar değişmez: `EngineEvent.PttOpened{peerId, streamId, codec, bundleMs}` · `PttBundle{peerId, streamId, payload}` · `PttClosed{peerId, streamId}`; sıra alanı yok (kama-mobile `event.rs:97-122` @ `poc-uniffi-v0`; kama-node `event.rs:98-123`). Akış anahtarı (peerId, streamId); aynı eşten en çok 2 eşzamanlı akış gelebilir (PTT-R5, kama-app `ptt.rs:219`), her biri ayrı tamponlanır.
2. Rust tamponu açıkken `PttBundle` OYNATMA ANINDA gelir: akışın ilk demeti Rust'a vardıktan T sonra (T ∈ [1,5; 2,5] s, varsayılan 2,0 s; gösterim değeri 13 Kas ölçümüyle), sonrakiler `bundleMs` aralıkla. Tampon gösterim yapısında açık; bugünkü `poc-uniffi-v0` `.so`'sunda YOK (demetler varışta gelir). Kotlin kodu iki durumda da aynıdır.
3. Zaman çözünürlüğü düğüm turudur: `NODE_TICK` = 20 ms (kama-node `lib.rs:127`, `node.rs:298`). Olay planlanan andan en çok bir tur (≈ 20 ms + tur süresi) geç gelir, erken gelmez; aynı turda birden çok olay gelebilir.
4. Bir akışın `PttBundle`'ları gönderim sırasıyla gelir; aynı oturumda boşluk, atlama, yinelenme yoktur (üç PTT kaydı Reliable, D-10 B; `ptt.rs:20-31`). Tek istisna oturum değişimi (gösterimde beklenmez): o an yolda olan demetler kaybolabilir, eski oturumdan geç gelen demet yenilerden sonra gelebilir (`ptt.rs:156-170`).
5. Geç demette Rust bekler, atlamaz: iki `PttBundle` arası `bundleMs`'ten uzun olabilir. Kotlin o aralıkta sessizlik üretir (ses durur; bozuk ya da uydurma ses yok), demeti düşürmez, yetişmek için hızlandırmaz. Durmadan sonra Rust takvimi geç demete yeniden çapalar: o akışta gecikme durma kadar artar.
6. `PttOpened` hemen gelir (konuşma göstergesi); ilk `PttBundle` ondan en az T sonra gelir. Tampon dolmadan oynatma başlamaz: Kotlin `PttOpened`'da çözücüyü/çalıcıyı hazırlar, çalmaya ilk `PttBundle`'la başlar.
7. `PttClosed` yalnız Rust tamponu boşalınca, akışın son `PttBundle`'ından sonra gelir (en erken son demetin veriliş anı + `bundleMs`). Kotlin elindeki demeti çalıp bitirir, `PttClosed`'da kesmez. `PttClosed` demetsiz de gelebilir (boş basış, 120 s hareketsizlik `ptt.rs:215`, oturum sonu).
8. Kotlin kendi titreşim tamponunda en çok bir demet (`bundleMs`) tutar; ikinci bir ön-tampon ya da gecikme eklemez (gecikme bütçesi Rust'ta: dinleyici ≤ 3 s + T).
9. `onEvent` düğüm iş parçacığında çağrılır, hızlı döner (bloklarsa bütün düğüm turu durur; kama-mobile `lib.rs:105-110`). Olaylar `StateFlow`'a DEĞİL sınırlı bir olay kuyruğuna konur (ör. `Channel(256)` + `trySend`; dolarsa say ve logla, bloklama): `StateFlow` ara değerleri birleştirir ve demet düşürür. PTT olaylarıyla diğer olaylar (metin, dosya) arasında sıra varsayılmaz.
10. Sözleşme değişirse tarihli satırla bildirilir (bu sürüm v1, 2026-11-09). Aralık'ta `PttBundle.seq` ve `PttGap` (T1-1 facade eki, D-10) 4. ve 5. satırların yerine geçer.
