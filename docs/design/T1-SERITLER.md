# T1 — ince şeritler ve oynatma tamponu (YOL3-T1, gösterim notu)

**Sürüm** 2 · 2026-11-10 (Ek A v2: sahip kararı v4 EK-A §3, §10) · sürüm 1 · 2026-11-09 (G0) ·
**Dal** `yol3/t1-seritler` · **Taban** `origin/main` `5de1ce3f` (`crates/` `a858f3fc` ile aynı;
§1–§10 satır numaraları bu ağacındır; Ek A v2'deki `ptt.rs` satırları dalın 2026-11-10 ağacındır)
· **İş emri** ISEMRI-YOL3-T1 v1.
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
dolunca kesinti yine olur. Şeritler oturum başınadır: birim yeni oturuma TAŞINMAZ. Oturum etkin
olmaktan çıkınca (supersede ya da kapanış) şeridinde bekleyen birimler düşer ve `lane_dropped`'ta
sayılır (KK-T1-18; S1 K7: teslim edilmemiş birimi Application yeni oturumda yeniden sunar —
TEXT outbox, D5 yeniden PTT_OPEN, dosyada `superseded` ABORT). Uca verilmiş birimler ucun
boşaltmasıyla sürer (bugünkü gibi).

## 7. Oynatma tamponu (KK-T1-11)

`crates/kama-app/src/playout.rs`; `Ptt::set_playout(Option<PlayoutConfig>)`, `None` varsayılan =
bugünkü davranış. Yaşam döngüsü (eş, `stream_id`; `PttStarted` … `PttEnded`) başına:
`PttStarted` hemen; ilk demet a0'da vardıysa `i`'inci `PttAudio` **en erken** a0 + T + i·`bundle_ms`'te
(T ∈ [1,5; 2,5] s, `Default` 2,0 s, aralık dışı tipli hata) — hiçbir demet planından erken
verilmez (EK-A §10); geç demet beklenir, ATLANMAZ, gelince hemen verilir ve takvim ona yeniden
çapalanır (`playout_stalls`, `playout_stall_ms`); `PttEnded` yalnız tampon boşalınca, en erken
son demetin verilişi + `bundle_ms`'te (KK-T1-17). Saat `Ptt`'nin enjekte saati (`ptt.rs:439`).
**Yerel sonlandırma** (2026-11-10, KK-T1-19/20; Kotlin tarafındakiyle aynı kural): akış tavanı
256 demet aşılınca ya da oturum değişiminde (Ek A v2 madde 11) çalınmamış demetler atılır,
`PttEnded` hemen, sonraki demetler elenir, Ptt'nin `PttEnded`'i yutulur — sayaçlar
`playout_overflow`, `playout_session_closes`, `playout_discarded`, `playout_late_eliminated`;
tepe doluluk `playout_peak_bundles` (EK-A §6.5). Akış tablosu tavanı `2 × MAX_PEERS ×
PTT_MAX_ACTIVE_STREAMS` (türetilmiş). Kapatma (`None`): tampondaki akışlar planıyla biter, yeni
akış tamponlanmaz. *v1'deki "tavan aşılırsa en eski demet beklemeden verilir" kuralı kalktı (erken
verme; EK-A §10 ile çelişiyordu).* Kotlin tarafı: Ek A.

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
(PTT T kadar geç); kama-node/Kotlin bu sırayı varsaymaz (Ek A v2 madde 9).

## 10. Bayraklar ve testler

Varsayılanlar `lanes = None`, `playout = None` (ürün yolu değişmez; davranış-nötrlük T-L6,
T-P5). Gösterim düğümüne geçiş yalnız Ö'nün test-util kancası / kama-node `goster` özelliği;
T1 kama-node'a ve kama-mobile'a dokunmaz. Testler: T-L1…T-L5, T-L9, T-L10 birim
(`client-core/src/lanes.rs`), T-L6, T-L8, T-L10 adaptör (`client-core/tests/lanes_e2e.rs`),
T-L7 (yenileme) G2; T-P1…T-P12 (`kama-app/tests/ptt.rs`, sahte saat; T-P9 erken verilmez
taraması, T-P10…T-P12 oturum değişimi ve yaşam döngüsü, 2026-11-10); S1 v2 için seam mock/real
+ drift kapısı. Gözlem: `ClientTransport::lane_stats`/`lane_backlog`, `Ptt::playout_stats`.

## Ek A — Rust ↔ Kotlin PTT oynatma sözleşmesi v2 (2026-11-10)

**Geçerli sürüm.** v1'in (2026-11-09, A.6) yerine geçer. Dayanak: sahip kararı v4 EK-A §3 ve §10
(KK-T1-EKA-1…8; karşılama tablosu A.2). UniFFI yüzeyi DEĞİŞMEZ: `PttGap` ve `PttClosed{reason}`
yoktur, olaylarda sıra alanı yoktur. Muhatap: Kotlin tarafı (uygulama geliştiricisi). Etiket:
*gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta.*

### A.1 Sözleşme maddeleri

1. **Olaylar ve yaşam döngüsü.** `EngineEvent.PttOpened{peerId, streamId, codec, bundleMs}` ·
   `PttBundle{peerId, streamId, payload}` · `PttClosed{peerId, streamId}` (kama-mobile
   `event.rs:97-122` @ `poc-uniffi-v0`; kama-node `event.rs:98-123`). Anahtar (peerId, streamId).
   Rust her anahtar için `PttOpened (PttBundle)* PttClosed` dizisi verir: aynı anahtara yeni
   `PttOpened` yalnız öncekinin `PttClosed`'ından sonra gelir; `PttBundle` yalnız açık yaşam
   döngüsüne gelir (`kama-app/tests/ptt.rs` `assert_lifecycles`; T-P9…T-P12). Aynı eşten en çok 2
   eşzamanlı akış (PTT-R5, kama-app `ptt.rs:222`); her biri ayrı tutulur.
2. **Zamanlama ve ölçütler.** Rust tamponu açıkken `PttBundle` oynatma anında gelir: akışın ilk
   demeti Rust'a `a0`'da vardıysa `i`'inci demet **en erken** `a0 + T + i·bundleMs`'te gelir
   (T ∈ [1,5; 2,5] s, varsayılan 2,0 s; gösterim değeri mini-Ö ölçümüyle, KK-T1-12). Hiçbir demet
   planından erken gelmez (T-P9); geç gelebilir (madde 5). Tampon gösterim yapısında açıktır;
   bugünkü `poc-uniffi-v0` `.so`'sunda yoktur (demetler varışta gelir); Kotlin kodu iki durumda
   aynıdır. **Ölçütler (EK-A §3.4):** "ilk demet varışı ≤ 3 s" (basıştan alıcı Rust'a varış)
   bağlayıcı gösterim ölçütüdür ve tampondan bağımsızdır; "ilk duyulabilir ses" (varış + T +
   madde 3'teki pay) ayrıca ölçülür ve raporlanır. Dinleyicinin duyduğu gecikme **≤ 3 s + T**
   olarak etiketlenir; tampon kabul hedefini değiştirmez.
3. **Zaman çözünürlüğü — ölçülen değer (EK-A §3.7).** Düğüm turu `NODE_TICK` = 20 ms (kama-node
   `lib.rs:127`, `node.rs:298`): olay planlanan andan erken gelmez, aynı turda birden çok olay
   gelebilir. 20 ms tek başına gecikme garantisi DEĞİLDİR: callback süresi + tur süresi + Kotlin
   tüketimi mini-Ö'de ölçülür ve buraya tarihli "ölçülen" satırı olarak yazılır. Durum
   (2026-11-10): **ölçülmedi.**
4. **Sıra ve kayıp — gerçek davranış (EK-A §3.3).** Bir yaşam döngüsünün `PttBundle`'ları
   gönderim sırasıyla gelir, yinelenmez (üç PTT kaydı Reliable, D-10 B, `ptt.rs:20-31`; oturum
   içi sıra S1 §4). Demet YALNIZ şu açık ve sayılan yollarda kaybolur:
   (a) Rust yerel sonlandırması (tampon açık): akış tavanı 256 demet (KK-T1-19) ya da oturum
   değişimi (madde 11) — çalınmamış demetler atılır, sonrakiler elenir, `PttClosed` hemen gelir
   (`PlayoutStats::playout_discarded`, `playout_late_eliminated`);
   (b) Kotlin yerel sonlandırması (madde 9, 10; Kotlin sayaçları);
   (c) Rust alıcının mevcut DROP kuralları: akışı olmayan demet (PTT-R3), `codec`/`bundleMs`
   uyuşmazlığı (S9), eş başına 2 akış tavanı (PTT-R5) (`PttStats::ptt_orphan_bundles`,
   `bundles_mismatched`, `opens_over_active_limit`);
   (d) oturum değişiminde gönderende yolda kalıp yeni oturuma yeniden gönderilmeyen demetler
   (S1 K7, `ptt.rs:156-160`) — alıcı bunları göremez ve sayamaz.
   Bunların dışında boşluk yoktur: iki demet arası `bundleMs`'ten uzunsa neden geç teslimdir
   (madde 5), kayıp değil.
5. **Geç demet — açık gösterim tercihi (EK-A §3.5).** Geç demette Rust bekler, atlamaz; iki
   `PttBundle` arası `bundleMs`'ten uzun olabilir. Kotlin o aralıkta sessizlik üretir (bozuk ya da
   uydurma ses yok), demeti düşürmez, yetişmek için hızlandırmaz. Durmadan sonra Rust takvimi geç
   demete yeniden çapalar: o akışta gecikme durma kadar artar ve geri inmez (gecikme birikimi).
   Bu, "ses sıralı ve eksiksiz gelsin, gerekirse dursun" gösterim tercihidir; kesintisiz canlı PTT
   bu sürümle çözülmüş DEĞİLDİR (canlı kip T1-5, Aralık).
6. **Başlangıç.** `PttOpened` hemen gelir (konuşma göstergesi); ilk `PttBundle` ondan en az T
   sonra gelir. Tampon dolmadan oynatma başlamaz: Kotlin `PttOpened`'da çözücüyü/çalıcıyı
   hazırlar, çalmaya ilk `PttBundle`'la başlar. Ölçüt ayrımı madde 2'dedir.
7. **Kapanış.** `PttClosed` yaşam döngüsünün son `PttBundle`'ından sonra gelir: olağan yolda Rust
   tamponu boşalınca (en erken son demetin veriliş anı + `bundleMs`), yerel sonlandırmada hemen
   (madde 4a). Kotlin elindeki demetleri çalıp bitirir, `PttClosed`'da kesmez. `PttClosed`
   demetsiz de gelebilir (boş basış, 120 s hareketsizlik `ptt.rs:218`, oturum sonu, yerel
   sonlandırma). Kapanışın nedeni olayda yoktur; Rust sayaçlarındadır.
8. **Kotlin titreşim payı.** Kotlin kendi çalıcı tamponunda en çok bir demet (`bundleMs`) tutar;
   ikinci bir ön-tampon ya da gecikme eklemez (gecikme bütçesi Rust'tadır).
9. **Rust → Kotlin teslim sınırı ve taşma (EK-A §3.1, §10).** `onEvent` düğüm iş parçacığında
   çağrılır, BLOKLAMAZ, hızlı döner (bloklarsa bütün düğüm turu durur; kama-mobile
   `lib.rs:105-114`). Olay `StateFlow`'a konmaz (ara değerleri birleştirir) ve `trySend`'in
   reddedebileceği bir kanala da konmaz (reddedilen olayın sahibi kalmaz). Kuyruğun sahibi
   Kotlin'dir: `onEvent` olayı akış başına durum kaydına yazar (kısa kilit; A.3). Demet kuyruğu
   akış başına zaman tabanlı sınırlıdır: **256 × `bundleMs`** (`bundleMs` = 200 ms'de ≈ 51 s).
   **Taşma politikası:** kuyruk doluyken demet gelirse Kotlin akışı yerel sonlandırır —
   çalınmamış demetleri ve taşıran demeti atar, kendi kapanışını (`OVERFLOW`) üretir, sayar; o
   yaşam döngüsünün sonraki `PttBundle`'ları (peerId, streamId) ile elenir ve sayılır; Rust'un
   sonraki `PttClosed`'ı ikinci kapanış üretmez. Bu bir ölüm teşhisi değildir (tüketici geçici
   yavaşlamış olabilir), açık politikadır; Rust tamponunun taşma kuralıyla aynıdır (KK-T1-19).
   PTT olaylarıyla diğer olaylar (metin, dosya) arasında sıra varsayılmaz.
10. **Yaşam döngüsü olayları düşmez (EK-A §3.1, §10).** `PttOpened`/`PttClosed` demet kuyruğuna
    girmez, akış başına ayrı durum alanında tutulur: kuyruk dolu olsa da kapanış kaydedilir ve
    tüketici yeniden çalışınca kalan demetlerden sonra görünür. Tüketicinin gördüğü her açılışın
    ardından tam bir kapanış gelir. Tüketici çok gerideyse (yük taşıyan 256 tüketilmemiş yaşam
    döngüsü; Rust akış tablosuyla aynı türetme) en eski kaydın çalınmamış demetleri atılır
    (sayılır), açılış ve kapanışı yine tüketiciye ulaşır. Sessiz kayıp yok: atılan ve elenen her
    demet bir sayaçtadır.
11. **Oturum değişimi (EK-A §3.6) — `seq` beklenmeden, (peerId, streamId) yaşam döngüsüyle.**
    (a) **Rust (tampon açık; KK-T1-20):** yaşam döngüsü başladığında eşin güncel oturumu
    kaydedilir. Eşin güncel oturumu başka bir oturum olunca (supersession ya da kapanış +
    yeniden bağlanma) ya da aynı anahtara yeni yaşam döngüsü gelince eski yaşam döngüsü madde
    4a kuralıyla kapanır: tampondaki eski oturum demetleri çalınmaz, `PttClosed` hemen gelir,
    eski oturumun drain'inden geç gelen demetler ve akışın yeni oturumdaki devamı elenir (kayıtlar
    oturum tutamacı taşımadığı için ikisi `seq`'siz ayırt edilemez; `MessengerEvent::Other{peer,
    message}`); yeni oturumda açılan akış yeni yaşam döngüsüdür (T-P10, T-P11, T-P12). Kullanıcıya
    eski oturum demeti verilmez. Bedel: oturum değişimini aşan basışın kalanı ve tampondaki en çok
    T kadar ses çalınmaz (sayılır); gösterimde oturum değişimi beklenmez. Bilinen sınır: basış
    tam değişim anında başlar ve akışın `PTT_OPEN`'ı eski oturumun drain'inden değişim okunduktan
    SONRA gelirse o akış yeni oturumun akışı sayılır; Aralık'ta `PttBundle.seq` (T1-1 facade eki)
    kapatır. Tampon kapalıyken (bugünkü `.so`) bu madde Rust'ta uygulanmaz (PTT-R0,
    `ptt.rs:29-32`).
    (b) **Kotlin:** Kotlin oturumu değişince (düğüm yeniden kurulur, dinleyici değişir) `reset()`
    bütün oynatma durumlarını `RESET` ile kapatır, çalınmamış demetleri sayarak atar; eski
    dinleyiciden geç gelen olaylar dinleyici nesliyle elenir ve sayılır. Kapanmış ya da bilinmeyen
    anahtarın `PttBundle`'ı her durumda (peerId, streamId) ile elenir.
12. **Sürüm.** Bu sürüm v2 (2026-11-10). Sözleşme değişirse tarihli satırla bildirilir. Aralık'ta
    `PttBundle.seq` ve `PttGap` (T1-1 facade eki, D-10) madde 4, 5 ve 11'in yerine geçer.

### A.2 KK-T1-EKA karşılama tablosu

| KK | istenen | karşılayan madde | test |
|---|---|---|---|
| EKA-1 | callback bloklamaz; reddedilen olayın sahibi belli; "logla geç" yok; yaşam döngüsü olayı düşmez | 9, 10; A.3 `onEvent` | Kotlin `overflowTerminatesTheStreamLocallyAndCountsEverything`, `closeIsRecordedWhileTheQueueIsFullAndSeenAfterResume`, `onEventNeverCallsTheSinkAndIgnoresNonPttEvents` |
| EKA-2 | kuyruk taşıran test şimdi; taşmada davranış seçilen politikayla birebir | 9; 4a | Kotlin `overflowTerminatesTheStreamLocallyAndCountsEverything`; Rust T-P7 `t_p7_overflow_terminates_the_stream_locally_and_releases_nothing_early` (aynı kural, aynı eşik) |
| EKA-3 | madde 4 "kayıp yok" gerçek davranışla | 4 (a)–(d) | T-P7, T-P10, T-P11 (atılan/elenen sayaçları); Kotlin taşma ve `reset` testleri |
| EKA-4 | ilk demet varışı ≤ 3 s bağlayıcı; ilk duyulabilir ses ayrı; ≤ 3 s + T etiketli | 2, 6 | ölçüm: mini-Ö (EK-A §6.1) |
| EKA-5 | geç demette bekleme açık gösterim tercihi | 5 | T-P3, T-P9 ("durma" deseni) |
| EKA-6 | oturum değişiminde eski durum kapanır, geç olaylar elenir, (peerId, streamId) ile | 11 | Rust T-P10, T-P11, T-P12; Kotlin `resetClosesOldPlaybackAndDropsLateEventsOfTheOldListener`, `reopenedKeyIsAFreshLifecycle`, `lateBundleOfAClosedStreamIsEliminatedByKey` |
| EKA-7 | 20 ms tick garanti değil; "ölçülen" | 3 | ölçüm: mini-Ö |
| EKA-8 | UniFFI değişmez; Kotlin kuyruğu 256 × bundleMs; yerel sonlandırma + sayaç; geç demet (peerId, streamId) ile elenir; açılış/kapanış ayrı durumda; Rust'ta erken verilmez | 1, 9, 10, 2 | Rust T-P9 `t_p9_no_bundle_is_released_before_its_plan` (24 koşu: 3 T × 2 bundle_ms × 4 desen); Kotlin testlerinin tamamı; `stalledConsumerCompactsPayloadButKeepsEveryLifecycleEvent` |

### A.3 Kotlin başvuru tüketicisi (Kotlin tarafı için)

Yalnız Kotlin stdlib + JVM; coroutine gerekmez. `wake` tüketici iş parçacığını uyandırır (ör.
`LockSupport.unpark`, ya da `Channel<Unit>(Channel.CONFLATED).trySend(Unit)` — uyandırma veri
taşımadığı için birleşmesi kayıp değildir; veri akış durumundadır). Uygulamada `EngineListener`
şöyle bağlanır: `val on = playback.listener(); object : EngineListener { override fun onEvent(ev:
EngineEvent) = on(ev) }`.

```kotlin
// Rust ↔ Kotlin PTT oynatma sözleşmesi v2 (2026-11-10) — Kotlin tarafı başvuru tüketicisi.
// Yalnız Kotlin stdlib + JVM; coroutine gerekmez. Üretilmiş UniFFI tipleri (EngineEvent, PttCodec)
// `uniffi.kama_mobile` paketinden gelir; UniFFI yüzeyi değişmez.
package kama.ptt

import uniffi.kama_mobile.EngineEvent
import uniffi.kama_mobile.PttCodec

/** Akış anahtarı (madde 1). */
data class StreamKey(val peerId: String, val streamId: UInt)

/** Tüketicinin gördüğü kapanış nedeni — yalnız Kotlin içi (UniFFI'de `PttClosed{reason}` yok). */
enum class CloseReason {
    /** Rust `PttClosed` (PTT_CLOSE, hareketsizlik, oturum sonu ya da Rust yerel sonlandırması). */
    REMOTE,
    /** Kotlin kuyruk tavanı aşıldı: yerel akış sonlandırma (madde 9). */
    OVERFLOW,
    /** Kotlin oturumu değişti (düğüm yeniden kuruldu / dinleyici değişti): `reset()` (madde 11). */
    RESET,
}

/** Oynatıcı (uygulama sağlar). YALNIZ tüketici iş parçacığında, `drain()` içinden çağrılır. */
interface PttSink {
    fun open(key: StreamKey, codec: PttCodec, bundleMs: Int)
    fun play(key: StreamKey, payload: ByteArray)
    fun close(key: StreamKey, reason: CloseReason)
}

/** Kotlin sayaçları (madde 9, 11; "sessiz kayıp yok"). */
data class PlaybackStats(
    val overflowCloses: Long = 0,
    val discardedBundles: Long = 0,
    val lateBundlesEliminated: Long = 0,
    val oldListenerEvents: Long = 0,
    val unknownCloses: Long = 0,
    val protocolRepairs: Long = 0,
    val stallCompactions: Long = 0,
    val peakQueuedBundles: Int = 0,
)

/**
 * Başvuru tüketicisi. `onEvent` düğüm iş parçacığında çağrılır: kısa kilit, ayırma yalnız sınırlı
 * yapılara, `sink`'e asla dokunmaz, bloklamaz (madde 9). `drain` tüketici iş parçacığında çağrılır
 * (uyandırma `wake` ile; uyandırma veri taşımaz, birleşmesi kayıp değildir).
 */
class PttPlayback(private val sink: PttSink, private val wake: () -> Unit = {}) {
    companion object {
        /** EK-A §10: demet kuyruğu zaman tabanlı, akış başına 256 × bundleMs. */
        const val QUEUE_LIMIT_BUNDLES = 256

        /** Yük taşıyan, tüketilmemiş yaşam döngüsü tavanı (Rust akış tablosuyla aynı türetme:
         *  2 × 64 eş × 2 akış). Aşılınca en eski kaydın çalınmamış demetleri atılır, kaydı kalır. */
        const val MAX_PAYLOAD_LIFECYCLES = 256
    }

    private class Lifecycle(val key: StreamKey, val codec: PttCodec, val bundleMs: Int) {
        val bundles = ArrayDeque<ByteArray>()
        var opened = false
        var end: CloseReason? = null
    }

    private sealed class Action {
        class Open(val lc: Lifecycle) : Action()
        class Play(val key: StreamKey, val payload: ByteArray) : Action()
        class Close(val key: StreamKey, val reason: CloseReason) : Action()
    }

    private val lock = Any()
    /** Tüketilmemiş yaşam döngüleri, Rust `PttOpened` sırasıyla. Açılış/kapanış burada durur;
     *  demet kuyruğuna girmez (madde 10). */
    private val order = ArrayDeque<Lifecycle>()
    /** Rust'ta açık yaşam döngüsü (PttOpened görüldü, PttClosed görülmedi). */
    private val current = HashMap<StreamKey, Lifecycle>()
    private var generation = 0L
    private var stats = PlaybackStats()

    fun stats(): PlaybackStats = synchronized(lock) { stats }

    /** Bu Kotlin oturumunun dinleyici işlevi: `EngineListener.onEvent` bunu çağırır. `reset()`'ten
     *  sonra eski işlevin getirdiği olaylar elenir ve sayılır (madde 11). */
    fun listener(): (EngineEvent) -> Unit {
        val gen = synchronized(lock) { generation }
        return { ev -> onEvent(gen, ev) }
    }

    private fun onEvent(gen: Long, ev: EngineEvent) {
        val woke = synchronized(lock) {
            if (gen != generation) {
                val ptt = ev is EngineEvent.PttOpened || ev is EngineEvent.PttBundle ||
                    ev is EngineEvent.PttClosed
                if (ptt) stats = stats.copy(oldListenerEvents = stats.oldListenerEvents + 1)
                return@synchronized false
            }
            when (ev) {
                is EngineEvent.PttOpened ->
                    opened(StreamKey(ev.peerId, ev.streamId), ev.codec, ev.bundleMs.toInt())
                is EngineEvent.PttBundle -> bundle(StreamKey(ev.peerId, ev.streamId), ev.payload)
                is EngineEvent.PttClosed -> closed(StreamKey(ev.peerId, ev.streamId))
                // PTT dışı olaylar bu sınıfın değildir; sıra varsayımı yok (madde 9).
                else -> return@synchronized false
            }
            true
        }
        if (woke) wake()
    }

    private fun opened(key: StreamKey, codec: PttCodec, bundleMs: Int) {
        current.remove(key)?.let { old ->
            // Rust sözleşmesi: aynı anahtara yeni PttOpened yalnız önceki PttClosed'dan sonra gelir.
            stats = stats.copy(protocolRepairs = stats.protocolRepairs + 1)
            if (old.end == null) old.end = CloseReason.REMOTE
        }
        if (order.count { it.bundles.isNotEmpty() } >= MAX_PAYLOAD_LIFECYCLES) compactOldest()
        val lc = Lifecycle(key, codec, bundleMs)
        current[key] = lc
        order.addLast(lc)
    }

    private fun bundle(key: StreamKey, payload: ByteArray) {
        val lc = current[key]
        if (lc == null || lc.end != null) {
            // Sonlandırılmış ya da bilinmeyen yaşam döngüsü: (peerId, streamId) ile elenir.
            stats = stats.copy(lateBundlesEliminated = stats.lateBundlesEliminated + 1)
            return
        }
        // Zaman tavanı 256 × bundleMs; bundleMs akış boyunca sabit olduğundan 256 demete eşittir
        // (sayıyla denetlenir: bundleMs = 0 da sınırlı kalır). Rust tamponuyla aynı eşik.
        if (lc.bundles.size >= QUEUE_LIMIT_BUNDLES) {
            // Taşma: yerel akış sonlandırma (açık politika; ölüm teşhisi değil).
            stats = stats.copy(
                overflowCloses = stats.overflowCloses + 1,
                discardedBundles = stats.discardedBundles + lc.bundles.size + 1,
            )
            lc.bundles.clear()
            lc.end = CloseReason.OVERFLOW
            return
        }
        lc.bundles.addLast(payload)
        if (lc.bundles.size > stats.peakQueuedBundles) {
            stats = stats.copy(peakQueuedBundles = lc.bundles.size)
        }
    }

    private fun closed(key: StreamKey) {
        val lc = current.remove(key)
        if (lc == null) {
            stats = stats.copy(unknownCloses = stats.unknownCloses + 1)
            return
        }
        // Kuyruk dolu olsa da kapanış kaydedilir (madde 10); yerel sonlandırılmışsa nedeni korunur.
        if (lc.end == null) lc.end = CloseReason.REMOTE
    }

    /** Tüketici çok geride: yük taşıyan en eski kaydın çalınmamış demetleri atılır; açılış/kapanışı
     *  kalır (yaşam döngüsü olayı düşmez). Rust'ta hâlâ açıksa yerel sonlandırılır. */
    private fun compactOldest() {
        val victim = order.first { it.bundles.isNotEmpty() }
        stats = stats.copy(
            stallCompactions = stats.stallCompactions + 1,
            discardedBundles = stats.discardedBundles + victim.bundles.size,
        )
        victim.bundles.clear()
        if (victim.end == null) victim.end = CloseReason.OVERFLOW
    }

    /** Kotlin oturum değişimi (madde 11): bütün yaşam döngüleri RESET ile kapanır, çalınmamış
     *  demetler atılır (sayılır); eski dinleyiciden geç gelen olaylar elenir. */
    fun reset() {
        synchronized(lock) {
            generation += 1
            for (lc in order) {
                stats = stats.copy(discardedBundles = stats.discardedBundles + lc.bundles.size)
                lc.bundles.clear()
                if (lc.end == null) lc.end = CloseReason.RESET
            }
            current.clear()
        }
        wake()
    }

    /** Tüketici iş parçacığı: bekleyen open/play/close'u sırayla `sink`'e verir. Kilit `sink`
     *  çağrısı sırasında tutulmaz. Dönüş: yapılan işlem sayısı. */
    fun drain(): Int {
        var n = 0
        while (true) {
            val action = synchronized(lock) { nextAction() } ?: return n
            when (action) {
                is Action.Open -> sink.open(action.lc.key, action.lc.codec, action.lc.bundleMs)
                is Action.Play -> sink.play(action.key, action.payload)
                is Action.Close -> sink.close(action.key, action.reason)
            }
            n += 1
        }
    }

    private fun nextAction(): Action? {
        for (lc in order) {
            if (!lc.opened) {
                lc.opened = true
                return Action.Open(lc)
            }
            lc.bundles.removeFirstOrNull()?.let { return Action.Play(lc.key, it) }
            val end = lc.end
            if (end != null) {
                order.remove(lc)
                return Action.Close(lc.key, end)
            }
        }
        return null
    }
}
```

### A.4 Kotlin birim testi taslağı

```kotlin
// Rust ↔ Kotlin PTT oynatma sözleşmesi v2 — başvuru tüketicisinin birim testi (JUnit 4 +
// kotlin.test). Deterministik: tek iş parçacığı, sahte sink, RNG yok.
package kama.ptt

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import uniffi.kama_mobile.EngineEvent
import uniffi.kama_mobile.PttCodec

class PttPlaybackTest {
    private val peer = "peer-a"
    private val key = StreamKey(peer, 7u)

    /** Sink çağrılarını kaydeder; `drain` dışında çağrılırsa testi düşürür (madde 9). */
    private class RecordingSink : PttSink {
        val log = mutableListOf<String>()
        var inDrain = false
        override fun open(key: StreamKey, codec: PttCodec, bundleMs: Int) { check(inDrain); log += "open ${key.streamId}" }
        override fun play(key: StreamKey, payload: ByteArray) { check(inDrain); log += "play ${key.streamId}:${payload[0]}" }
        override fun close(key: StreamKey, reason: CloseReason) { check(inDrain); log += "close ${key.streamId} $reason" }
    }

    private fun fixture(): Triple<PttPlayback, RecordingSink, (EngineEvent) -> Unit> {
        val sink = RecordingSink()
        val pb = PttPlayback(sink)
        return Triple(pb, sink, pb.listener())
    }

    private fun PttPlayback.drainWith(sink: RecordingSink): Int {
        sink.inDrain = true
        try { return drain() } finally { sink.inDrain = false }
    }

    private fun opened(id: UInt = 7u) = EngineEvent.PttOpened(peer, id, PttCodec.LYRA, 200u.toUByte())
    private fun bundle(n: Int, id: UInt = 7u) = EngineEvent.PttBundle(peer, id, byteArrayOf(n.toByte()))
    private fun closed(id: UInt = 7u) = EngineEvent.PttClosed(peer, id)

    /** Kuyruk taşırma (KK-T1-EKA-2/8): 257. demette yerel sonlandırma; açılış ve kapanış düşmez,
     *  atılan ve elenen her demet sayılır; Rust PttClosed ikinci kapanış üretmez. */
    @Test
    fun overflowTerminatesTheStreamLocallyAndCountsEverything() {
        val (pb, sink, on) = fixture()
        on(opened())
        repeat(PttPlayback.QUEUE_LIMIT_BUNDLES) { on(bundle(it)) } // tüketici durmuş: drain yok
        assertEquals(0L, pb.stats().overflowCloses, "tavana dek taşma yok")
        on(bundle(99))
        repeat(3) { on(bundle(100 + it)) }
        on(closed())
        pb.drainWith(sink)
        assertEquals(listOf("open 7", "close 7 OVERFLOW"), sink.log)
        val s = pb.stats()
        assertEquals(1L, s.overflowCloses)
        assertEquals(257L, s.discardedBundles)
        assertEquals(3L, s.lateBundlesEliminated)
        assertEquals(0L, s.unknownCloses)
        assertEquals(256, s.peakQueuedBundles)
    }

    /** Kapanış demet kuyruğuna girmez: kuyruk tavanda doluyken gelen PttClosed kaydedilir ve
     *  tüketici yeniden çalışınca bütün demetlerden sonra görünür (madde 7, 10). */
    @Test
    fun closeIsRecordedWhileTheQueueIsFullAndSeenAfterResume() {
        val (pb, sink, on) = fixture()
        on(opened())
        repeat(PttPlayback.QUEUE_LIMIT_BUNDLES) { on(bundle(it % 100)) }
        on(closed())
        pb.drainWith(sink)
        assertEquals("open 7", sink.log.first())
        assertEquals("close 7 REMOTE", sink.log.last())
        assertEquals(PttPlayback.QUEUE_LIMIT_BUNDLES, sink.log.count { it.startsWith("play") })
        assertEquals(0L, pb.stats().discardedBundles)
    }

    /** Kapanmış akışın geç demeti (peerId, streamId) ile elenir ve sayılır (madde 4, 11). */
    @Test
    fun lateBundleOfAClosedStreamIsEliminatedByKey() {
        val (pb, sink, on) = fixture()
        on(opened()); on(bundle(1)); on(closed()); on(bundle(2))
        pb.drainWith(sink)
        assertEquals(listOf("open 7", "play 7:1", "close 7 REMOTE"), sink.log)
        assertEquals(1L, pb.stats().lateBundlesEliminated)
    }

    /** Aynı anahtarın yeni yaşam döngüsü (Rust önce PttClosed verir) ayrı çalınır; iç içe geçmez. */
    @Test
    fun reopenedKeyIsAFreshLifecycle() {
        val (pb, sink, on) = fixture()
        on(opened()); on(bundle(1)); on(closed())
        on(opened()); on(bundle(2)); on(closed())
        pb.drainWith(sink)
        assertEquals(
            listOf("open 7", "play 7:1", "close 7 REMOTE", "open 7", "play 7:2", "close 7 REMOTE"),
            sink.log,
        )
    }

    /** Kotlin oturum değişimi (madde 11): reset eski oynatma durumunu RESET ile kapatır, çalınmamış
     *  demetler sayılarak atılır; eski dinleyiciden geç gelen olaylar elenir; yeni dinleyici çalışır. */
    @Test
    fun resetClosesOldPlaybackAndDropsLateEventsOfTheOldListener() {
        val (pb, sink, old) = fixture()
        old(opened()); old(bundle(1))
        pb.reset()
        old(bundle(2)); old(closed())
        val fresh = pb.listener()
        fresh(opened(8u)); fresh(bundle(3, 8u)); fresh(closed(8u))
        pb.drainWith(sink)
        assertEquals(listOf("open 7", "close 7 RESET", "open 8", "play 8:3", "close 8 REMOTE"), sink.log)
        val s = pb.stats()
        assertEquals(1L, s.discardedBundles)
        assertEquals(2L, s.oldListenerEvents)
    }

    /** onEvent sink'e dokunmaz (RecordingSink drain dışında çağrılırsa düşer); PTT dışı olay yok sayılır. */
    @Test
    fun onEventNeverCallsTheSinkAndIgnoresNonPttEvents() {
        val (pb, sink, on) = fixture()
        on(EngineEvent.TextReceived(peer, "x"))
        on(opened()); on(bundle(1))
        assertTrue(sink.log.isEmpty())
        assertEquals(2, pb.drainWith(sink))
    }

    /** Tüketici çok geride (yük taşıyan 256 kayıt): en eski kaydın demetleri atılır, açılış ve
     *  kapanışı tüketiciye yine ulaşır — yaşam döngüsü olayı düşmez. */
    @Test
    fun stalledConsumerCompactsPayloadButKeepsEveryLifecycleEvent() {
        val (pb, sink, on) = fixture()
        val n = PttPlayback.MAX_PAYLOAD_LIFECYCLES + 1
        for (i in 0 until n) { val id = i.toUInt(); on(opened(id)); on(bundle(1, id)); on(closed(id)) }
        pb.drainWith(sink)
        assertEquals(n, sink.log.count { it.startsWith("open") })
        assertEquals(n, sink.log.count { it.startsWith("close") })
        assertEquals(n - 1, sink.log.count { it.startsWith("play") })
        assertEquals("close 0 REMOTE", sink.log[1], "en eski kaydın demeti atıldı, kapanışı geldi")
        assertEquals(1L, pb.stats().stallCompactions)
        assertEquals(1L, pb.stats().discardedBundles)
    }
}
```

JVM derlemesi için kullanılan UniFFI tip taslağı (yalnız derleme; uygulamada üretilmiş bağlama):

```kotlin
// YALNIZ JVM derlemesi için: UniFFI'nin `kama_mobile` için ürettiği tiplerin PTT kesiti (aynı ad
// ve alan tipleri; `poc-uniffi-v0` `crates/kama-mobile/src/event.rs`). Uygulamada üretilmiş
// bağlama kullanılır, bu dosya değil.
package uniffi.kama_mobile

enum class PttCodec { OPUS, LYRA }

sealed class EngineEvent {
    data class PttOpened(val peerId: String, val streamId: UInt, val codec: PttCodec, val bundleMs: UByte) : EngineEvent()
    class PttBundle(val peerId: String, val streamId: UInt, val payload: ByteArray) : EngineEvent()
    data class PttClosed(val peerId: String, val streamId: UInt) : EngineEvent()
    data class TextReceived(val peerId: String, val text: String) : EngineEvent()
}
```

### A.5 Doğrulama durumu

- Rust (kama-app, sahte saat, RNG yok): T-P1…T-P12 + kapatma testi `cargo test -p kama-app
  --all-features` içinde geçti (koşu kaydı PR gövdesinde). Ağaç dışı denetim: T-P7, T-P10, T-P11,
  T-P12 ve kapatma testi v1 tamponunda KIRMIZI; T-P9 v1'de de geçer (v1'in tek erken yolu taşmaydı).
- Kotlin (A.3 + A.4): JVM'de derlendi ve 7/7 koştu — kotlinc 2.3.10, JUnit 4.12, UniFFI
  tiplerinin aynı adlı JVM taslağıyla. **Android'de ve üretilmiş UniFFI bağlamasıyla [denenmedi].**
- Madde 2 ve 3'ün ölçümleri: mini-Ö (EK-A §6); bugün ölçülmedi.

### A.6 v1 (2026-11-09) — v2 ile değiştirildi, UYGULANMAZ (MUST NOT IMPLEMENT)

*v1 metni aşağıda aynen korunur (yalnız başlığı değişti). Madde 4 ile madde 9 çelişiyordu (EK-A §3: `trySend` reddi demeti ve `PttClosed`'ı düşürür); geçerli metin A.1'dir.*


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
