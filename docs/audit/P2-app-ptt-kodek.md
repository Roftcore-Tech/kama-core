# P2-app-ptt-kodek — davranış envanteri ve otorite karşılaştırması

Taban: `125b9afa87e731c1e666ecf63f33b7cc555dc9b1` (ürün hattı; aday `1a4f9efc` bunun içinde). Satır numaraları bu tabandandır. Salt okuma: kaynak dosyalara yazılmadı, cargo çalıştırılmadı, git'te değişiklik yapılmadı.

## Kapsam

| dosya | satır aralığı | okunan satır |
|---|---|---|
| `crates/kama-app/src/ptt.rs` | 1–2351 | 2 351 |
| `crates/kama-app/src/playout.rs` | 1–454 | 454 |
| `crates/kama-app/src/signaling.rs` | 1–1080 | 1 080 |
| `crates/kama-app/src/codec/mod.rs` | 1–1305 | 1 305 |
| `crates/kama-app/src/codec/body.rs` | 1–359 | 359 |
| `crates/kama-app/src/codec/limits.rs` | 1–271 | 271 |
| `crates/kama-app/src/codec/header.rs` | 1–151 | 151 |
| `crates/kama-app/src/codec/compress.rs` | 1–115 | 115 |
| **toplam** | | **6 086** |

Bağlam için kapsam dışından okunanlar (bulgu bu dosyalara atanmadı): `kama-contracts/src/s1.rs` 75–100 (`TrafficClass`); `client-core/src/transport.rs` 536–545 (`PTT_HOLE_SKIP`), 855–875 (birim tavanı); `client-core/src/runtime.rs` 320, 455–462 (`MAX_UNIT_BYTES`, `PTT_DEADLINE_NS`); `kama-node/src/node.rs` 720–745; `kama-node/src/event.rs` 96–110; `docs/design/T1-SERITLER.md` §7 ve Ek A satırları (grep ile); `client-core/Cargo.toml` 21; çağıran taraması (`set_playout`, `set_compact_bundles`, `set_spurt_open_*`, `open_press`, `bundle_ms_from_code`).

## Davranış envanteri

### codec/limits.rs (271)

- **Alan genişliği sabitleri 15–18** (`U8/U16/U32/U64_BYTES` = `size_of`): bütün önek boyları bunlardan toplanır, elle sayı yok.
- **§2 sabitleri 20–39:** `MSG_VERSION_V1 = 0x01`; `APP_RECORD_HEADER_BYTES = 1+1+1+2 = 5`; `FLAGS_NONE = 0`, `FLAG_COMPRESSED = 0x01`, `FLAGS_RESERVED_MASK = !0x01`; `MSG_TYPE_RESERVED_ZERO = 0x00`, `MSG_TYPE_RESERVED_HIGH_START = 0xF0`.
- **§5 sabitleri 41–53:** `COMPRESSION_ID_ZSTD = 0x01`, `COMPRESSION_PREFIX_BYTES = 1+4 = 5`, `MAX_EXPANSION_RATIO = 64` (u64, çarpımla karşılaştırma), `COMPRESS_MIN_BYTES = 64` — A2 değerleri.
- **§3 önek sabitleri 55–93:** `WHOLE_FILE_HASH_BYTES = 32`, `FILE_NAME_MAX_BYTES = 255`, TEXT 6, FILE_BEGIN 53, FILE_CHUNK 18, FILE_END 8, FILE_ABORT 9, PTT_OPEN 6, PTT_CLOSE 4, RECEIPT 5, SIGNALING_BLOB 2, MEDIA_BUNDLE 8, FILE_RECEIPT 9 (167–169) — hepsi §3 alan listelerinin toplamı.
- **PTT-BANT / PTT-BİRLEŞİK işaretleri 95–123:** `COMPACT_MEDIA_MARKER = 0x8A` (2 B başlık: işaret + `stream_tag`), `SPURT_OPEN_MARKER = 0x8B`, `SPURT_OPEN_END_MARKER = 0x8C` (7 B başlık: işaret + `stream_id` + `codec` + `bundle_ms`), `SPURT_END_MARKER = 0x8D` (2 B). İşaret `msg_version` konumundadır (bulgu P2-02).
- **`bundle_ms` kodlaması 125–165:** `BUNDLE_MS_EXACT_MAX = 200`, `BUNDLE_MS_COARSE_STEP = 10`; `bundle_ms_from_code(c)`: c ≤ 200 → c ms, üstü `200 + (c−200)×10` (255 → 750 ms); `bundle_code_from_ms` tersi, 10'un katı olmayan ya da > 750 ms → `None` (P2-03).
- **`AmpLimits::from_capabilities` 186–214:** `MAX_BODY_BYTES = min(65535, unit − 5)` (denetimli çıkarma, sığmazsa `UnitTooSmallForHeader`), `MAX_DECOMPRESSED_MESSAGE = MAX_BODY_BYTES`, `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − 18` (sığmazsa `UnitTooSmallForFileChunk`). Erişimciler 216–232; `LimitsError` + `Display` 235–271.
- Uyumlu: **24** (A2 formüllerinin üçü, A1 alan genişlikleri, §2 reserved maske ve aralıklar, §5 sabitleri, FILE_RECEIPT 9 B, tipli hata, sessiz sıfıra sabitleme yok).

### codec/header.rs (151)

- **Konum sabitleri 9–16** (0, 1, 2, 3).
- **`MsgType` 18–83:** 0x01…0x0B registry; `ALL` kod sırasıyla; `from_code` reserved/bilinmeyen → `None`; `is_reserved_code` = 0x00 ya da ≥ 0xF0.
- **`parse_record` 102–132:** sıra — boş → DROP `record-too-short`; `msg_version ≠ 1` → DROP `unknown-msg-version` (başlığın geri kalanına bakılmaz); < 5 B → DROP `record-too-short`; reserved tip → REJECT; reserved bayrak → REJECT; `len − 5 ≠ body_len` → DROP `record-len-mismatch`; kayıtsız tip → DROP `unknown-msg-type`. Çıktı: tip, sıkıştırma biti, gövde dilimi (kopya yok).
- **`write_record` 135–151:** gövde u16'ya sığmazsa `BodyTooLarge{max=65535}`; 5 B başlık + gövde, kapasite önceden.
- Denetim sırası spec'te adım adım yok (aranan: AMP §2/§6 "sıra"); `vectors/amp-record-reject-v1.json` sabitler (§7 kapısı).
- Uyumlu: **7** (§2 alanlar, §6 dört hüküm, sürüm-önce kuralı, sınır-önce-tahsis).

### codec/body.rs (359)

- **`Reader` 18–81:** her okuma kalan baytı ÖNCE denetler, aşan → DROP `inner-field-exceeds-body-len`, hiçbir şey tüketilmez; `rest()` örtük kuyruk; `finish()` artık bayt → DROP `trailing-body-bytes`.
- **`utf8` 83–87:** geçersiz → DROP `invalid-utf8`. **`unknown_enum` 89–91.**
- **`decode_body` 94–189:** on bir tipin her biri §3 alan listesi sırasıyla; değişken alan kendi uzunluk alanından sonra (`text_len u16`, `name_len u8`, `payload_len u16`, `blob_len u16`); `FILE_ABORT.reason`, `PTT_OPEN.codec`, `RECEIPT.kind`, `FILE_RECEIPT.status` registry dışı → DROP `unknown-enum-value`; `MEDIA_BUNDLE.codec` opak u8 (spec "kap"); sonunda `finish`.
- **`plain_body_len` 193–224:** doymalı toplam; `FILE_BEGIN.name > 255` → `NameTooLong`; işaretli biçimlerin "gövdesi" işaret baytı hariç (kompakt/END: 1 + yük; açılışlı: 6 + yük).
- **`encode_body` 227–359:** boy `MAX_BODY_BYTES`'a karşı TAHSİSTEN ÖNCE; u16 uzunluk alanları `try_from`; alanlar big-endian; `debug_assert_eq!(out.len(), body_len)`.
- Uyumlu: **18** (A1 her alan; bounds-before-alloc; tek kanonik biçim; FILE_RECEIPT kısa/uzun gövde DROP §4; varint yok ADR-05).

### codec/compress.rs (115)

- **`compression_default_on` 33–46:** her tip için `false`; TEXT spec'te AÇIK ama zstd kararına dek kapalı (P2-12).
- **`zstd_compress` 50–52:** hep `None`. **`zstd_decompress` 58–60:** hep `Err(CompressionUnavailable)` — tipli stub, `todo!()` yok.
- **`compress_for_send` 63–80:** kural 5 (< 64 B yok), kural 6 (tip), kural 7 (çıktı > düz gövde → sıkıştırma yok; eşitlikte sıkıştırır). Bugün `zstd_compress` `None` döndüğü için her yol `None` (P2-13).
- **`open_compressed` 87–115:** önek sığmazsa DROP inner-field; `compression_id ≠ 1` → REJECT; kural 1 (`> MAX_DECOMPRESSED_MESSAGE`, açmadan) → kural 4 (`declared > 64 × body_len`, u64 doymalı çarpım, payda önek dahil) → aç (kural 3) → kural 2 (uzunluk eşitliği).
- Uyumlu: **6** (§5 alıcı kuralları 1–4 ve sırası, gönderici kural 5 ve 7, tipten bağımsız alıcı denetimi — YAMA 2026-10-06b F07.4).

### codec/mod.rs (1305)

- **Kayıt defterleri 43–180:** `FileAbortReason` (0x01–0x05; 0x02/0x03 kayıtlı, üretilmez), `FileReceiptStatus` (0x00–0x04), `PttCodec` (opus 0x01, lyra 0x02), `ReceiptKind` (0x01/0x02); her birinde `ALL`, `code`, `from_code`.
- **`AppMessage` 182–307:** spec'in on bir tipi + üç işaretli PTT biçimi (`MediaBundleCompact`, `MediaBundleOpen{end}`, `MediaBundleEnd`). `msg_type()` 309–330: üç işaretli biçim `MediaBundle` döner.
- **`Debug` 332–451:** metin, ad, yük, blob, özet yazılmaz; yalnız uzunluk.
- **`DropReason`/`RejectReason`/`DecodeError` 453–563:** kararlı gerekçe dizgeleri (ret vektörleriyle aynı); `zstd-decoder-unavailable` spec dizgesi değildir, stubun gerekçesidir.
- **`EncodeError` 565–602.**
- **`encode` 612–644:** işaretli biçimde `[işaret] + gövde` (başlık ve sıkıştırma yok, `CompressionPolicy::Never`); diğerlerinde gövde → `compress_for_send` → `write_record`.
- **`compact_stream_tag` 646–650:** `stream_id`'nin düşük 8 biti.
- **`decode` 658–711:** ilk bayt 0x8B/0x8C → < 7 B DROP too-short, > 1 + MAX_BODY_BYTES DROP len-mismatch, `MediaBundleOpen` (codec doğrulanmaz); 0x8D → < 2 B too-short, > 1 + MAX_BODY_BYTES len-mismatch; 0x8A → < 2 B too-short, > **5** + MAX_BODY_BYTES len-mismatch (P2-11); aksi hâlde başlık → (sıkıştırma) → gövde. İşaretli biçimler hiçbir bayrağa bağlı değildir (P2-02).
- **Testler 713–1305** (sabitledikleri): her tipin gidiş-dönüşü ve hiçbirinin sıkıştırılmaması; A2 formülleri ve en dar limitte FILE_RECEIPT'in sığması; tam sığan / 1 B fazla gövde; ad > 255; bugün sıkıştırma yok; `bundle_ms` kodu (≤ 200 özdeşlik, 300→210, 420→222, 750→255, ters gidiş-dönüş); kompakt 2 B başlık (13 → 2 B), v1 ayrıştırıcının onu `unknown-msg-version` ile atması, 0x8A tavanı `5+MAX` kabul / `5+MAX+1` DROP; açılışlı/END biçimleri ve kanonik `1+MAX` tavanı; kısa kayıt, reserved yüksek tip, kayıtsız tip (0x0C, 0xEF), uzunluk uyuşmazlığı, artık bayt, geçersiz UTF-8, bilinmeyen enum; sıkıştırılmış başlık denetimleri (oran sınırı tam/1 fazla, id REJECT, önek sığmıyor); Debug gizliliği; gerekçe dizgelerinin tekliği.
- Uyumlu: **16** (§2/§3/§5/§6 eşlemeleri, gizlilik, tipli hatalar, vektör dizgeleri).

### signaling.rs (1080)

- **Sabitler:** `FIRST_SIGNALING_ID = 0` (uygulamanın), `SIGNALING_RESEND_TTL = 60 s`, `SIGNALING_RESEND_PER_PEER = 256` (sahip kararı DUR-K4G-14a, AMP §3 K4G-14a), `MAX_SIGNALING_RECORDS = MAX_PEERS × 256` (türetilmiş), `const _: assert!(256 > 0)`.
- **`max_blob_bytes` 217–224:** `MAX_BODY_BYTES − 2` (doymalı).
- **`SignalingEvent` + gizli `Debug` 226–255; `SignalingError` 257–310; `SignalingStats` 312–332.**
- **`send` 402–448:** sıra boy → oturum → kimlik tükenmesi → eşin kayıtlarının yeniden sunumu (`refresh(Some(peer))`) → kodla → `send(Reliable, TrafficClass::Sig)` → kabulde kimlik tüketilir (`checked_add`, sarma yok) ve kayda girer.
- **`handle` 455–479:** `SIGNALING_BLOB`'u tüketir (dedup yok, eşin oturumu olmasa da teslim); `Ready` geçirilmeden önce o eşin kayıtları sunulur; diğerleri aynen.
- **`pump` 487–490, `refresh` 494–525:** ömrü dolanlar baştan budanır (`records_expired`), sonra sunum; boş eş girdisi silinir.
- **`remember` 530–541, `evict_oldest` 544–560:** eş doluysa eşin kendi en eskisi, tablo doluysa en küçük kimlik düşer; sınır eklemeden önce.
- **`alive` 564–566:** `now − sent_at < 60 s` (ilk kabul anı; yeniden sunum uzatmaz).
- **`offer_queue` 572–609:** eşin güncel oturumu yoksa hiçbir şey; aynı tutamaçtakini atlar; kabulde işaretler; geçici retle durur (sonrakiler de bekler); kalıcı retle kaydı düşürür.
- **`is_transient` 614–622:** Backpressure, UnknownSession, SessionSuperseded, NotConnected geçici; diğerleri kalıcı (P2-17).
- **Testler 624–1080** (sabitledikleri): kimlik tükenmesi tipli ve yeniden sunumdan önce döner; eş başına / toplam tavan ve en eski düşmesi; dolu eşin kendi en eskisini düşürmesi; yedi `TransportError` türünün sınıfı (joker kolsuz `match`); `pump`'ın bütün eşleri sunması; `handle` geçirgenliği; Debug'ın blob'u hiçbir biçimde basmaması.
- Uyumlu: **15** (AMP §3 K4G-14a dedup yok, yeniden sunum kuralı, 60 s / 256 / taşmada en eski, bellekte kayıt D8; seam §1/§4 opaklık; bounded tablo; sarma yok).

### playout.rs (454)

- **Sabitler 51–64:** `PLAYOUT_DELAY_MIN = 1,5 s`, `MAX = 2,5 s`, `DEFAULT = 2,0 s` (T1-SERITLER §7: "T ∈ [1,5; 2,5] s, `Default` 2,0 s"); `PLAYOUT_MAX_BUNDLES_PER_STREAM = 256` (`[KALİBRASYON]`, Kotlin 256 × bundleMs ile eş — EK-A §10); `PLAYOUT_MAX_STREAMS = 2 × MAX_PEERS × PTT_MAX_ACTIVE_STREAMS` (türetilmiş).
- **`PlayoutConfig::new` 77–82:** aralık dışı → tipli hata.
- **`Stream::due` 180–192:** kuyruk varsa `max(next_due, varış)`; bitmişse `son veriliş + bundle_ms` (hiç verilmemişse 0).
- **`close_superseded` 255–270:** eşin güncel oturumu başlangıç oturumundan farklıysa yerel sonlandırma (KK-T1-20); oturum yoksa dokunmaz.
- **`on_started` 280–316:** aynı anahtarın eski döngüsü varsa önce o sonlanır; tablo doluysa bitmiş en yakın akış tahliye; yine doluysa akış tamponsuz geçer (sessiz geri dönüş, türetmeye göre erişilemez); `bundle_ms` kodu `bundle_ms_from_code` ile ms'ye çevrilir.
- **`on_audio` 322–367:** sonlandırılmış → elenir; tamponsuz akış → hemen; oturum değişimi / tavan → yerel sonlandırma; ilk demet `next_due = varış + T`.
- **`on_ended` 371–381, `release_due` 385–419:** planından erken verilmez; geç demet beklenir, takvim `now + bundle_ms`'ye yeniden çapalanır (`playout_stalls`); kuyruk boş + bitmiş → `PttEnded`.
- **`terminate` 423–439, `evict_ended` 443–453.**
- Ürün çağıranı yok: `Ptt::set_playout` yalnız `tests/ptt.rs`'te çağrılıyor (P2-14).
- Uyumlu: **12** (T1-SERITLER §7 ve SAHIP-KARARI-v4 EK-A §3 madde 4–6, §10 T1: erken verme yok, geç demette bekleme, oturum değişiminde eleme, taşmada yerel sonlandırma, yaşam döngüsü olayı düşmez, bounded bellek).

### ptt.rs (2351)

- **Sabitler:** `FIRST_STREAM_ID = 0`; `PTT_STREAM_IDLE_TTL = 120 s` (PTT-R4); `PTT_BUNDLE_DEADLINE = 1 s` (P2-01; `runtime::PTT_DEADLINE_NS` ile elle eşit, P2-07); `SPURT_OPEN_REPEAT = 3`, `SPURT_OPEN_EVERY = 4` (P2-06); `SPURT_RX_HOLE_WAIT = 100 ms` (`transport::PTT_HOLE_SKIP` ile elle eşit, P2-07); `SPURT_GAP_BUNDLES = 2`, `SPURT_PILOT_GAP_BUNDLES = 4`; `SPURT_FORMAT_EPOCH = 1`; `SPURT_PILOT_BUNDLE_MS = 300` → `SPURT_PILOT_BUNDLE_CODE = 210` (derleme zamanı `match`, `panic!` yalnız const bağlamında); `PTT_MAX_ACTIVE_STREAMS = 2` (PTT-R5); `MAX_INCOMING_STREAMS = MAX_PEERS × 2`.
- **`spurt_gap_end` 315–324:** `1 s + 100 ms + n × bundle_ms(kod)`, pilot kodunda n = 4, diğerlerinde 2 (300 ms pilotta 2,3 s).
- **`SpurtCaps::capable` 385–393:** bu oturumun bant içi bildirimi önce; yoksa yönetilen ∪ aynı dönemle öğrenilen.
- **`bounded_insert` 396–402**, **`PttStats::note_spurt_tx` 404–419** (gözlem).
- **`max_bundle_payload_bytes` 444–450:** `MAX_BODY_BYTES − 8`.
- **`PttEvent` + gizli Debug 452–525; `PttError` 527–607; `PttStats` 609–673.**
- **Ayarlar 765–881:** `set_spurt_coalesce` (varsayılan kapalı), `set_peer_spurt_provisioned` (≤ MAX_PEERS), `on_peer_spurt_capability` (`false` öğrenilmişi siler, bu oturumdaki birleşik akışları ayrı kayda döndürür, `force_reopen`), `remember_spurt_capability` (dönem eşleşmeli), `learned_spurt_capabilities`, `set_spurt_open_repeat` (≥ 1), `set_spurt_open_every`, `set_compact_bundles` (yetenek anlaşmasız), `set_playout`.
- **Gönderen `open_with` 992–1039:** sıra StreamActive → StreamTableFull → NoSession → depo ayırma → birleşik karar (`Compatible` dışı, bayrak açık, eş yetenekli) → pilot kipinde süre 210 kodu → birleşik değilse `PTT_OPEN` `Reliable` → akış kaydı. `open` / `open_compatible` / `open_press` bu yolun kipleri.
- **`reserve_stream_id` 1049–1064:** `outgoing.len()+1` denemeyle sınırlı atlama; u32'ye sığmayan ya da yineleyen depo → `Store(Corrupt)`; `Exhausted` → `StreamIdExhausted`.
- **`send_spurt_bundle` 1147–1207:** bilinmeyen akış / END gönderilmiş → `UnknownStream`; yük sınırı kopyalamadan önce; oturum; `reopen` (D5); kayıt seçimi (birleşik → `spurt_record`, kompakt bayrak → 0x8A, aksi → tam `MEDIA_BUNDLE`); politika HER ZAMAN `DeadlineBounded{deadline}` (P2-01); son tarih `send_bundle`'da `Instant::now()+1 s`, `send_bundle_from`'da `audio_start+1 s` (P2-08).
- **`close` 1220–1259:** akışı her durumda kaldırır; `reopen` reddi yutulur; birleşik akışta END gönderildiyse tele bir şey çıkmaz, değilse yüksüz END `DeadlineBounded`; ayrı kayıtta `PTT_CLOSE` `Reliable`, tek deneme.
- **`poll` / `poll_through` / `poll_direct` 1279–1368:** bekleyen birleşik olaylar → D5 yeniden `PTT_OPEN` → oturumu kalmamış eşin akışları `PttEnded` → Messenger olayları → Messenger boşsa PTT-R4/sessizlik sonu. Tamponlu yolda `retain_terminated`, `close_superseded`, zamanı gelmiş tampon olayları önce.
- **`end_orphaned` 1381–1390, `end_idle` 1394–1408:** `idle_ttl` birleşik akışta `spurt_gap_end`, diğerlerinde 120 s.
- **Alıcı `on_open` 1467–1517:** oturumsuz eş → DROP (`opens_without_session`); kapanmış akış kaydında → DROP (`opens_after_close`); etkin → idempotent (`duplicate_opens`); eşin 2 etkin akışı → DROP (PTT-R5); tablo dolu → DROP; aksi `PttStarted`.
- **`on_close` 1522–1530:** etkin akışı bitirir; değilse yok sayılır (PTT-R2, `unknown_closes`); iki durumda da kapanmış akış kaydına.
- **`recently_closed`/`remember_closed` 1539–1568:** kayıt ömrü 120 s, eş başına 2, toplam MAX_INCOMING; en eski kapanış tahliye; sınır eklemeden önce.
- **`on_compact_bundle` 1574–1600:** `stream_tag` eşin tek etkin akışını tutmalı; yoksa PTT-R3, birden çoksa `compact_ambiguous`. Bayrağa bağlı değildir.
- **`on_spurt_open` 1607–1639, `on_spurt_end` 1643–1678, `spurt_audio_and_end` 1683–1720, `flush` 1723–1728:** bayrak kapalıysa `spurt_refused`; kapanmış kayıttaki kimlik → `spurt_after_end`; bilinmeyen akış `on_open` kurallarıyla açılır, `coalesced = true`; ses `on_bundle`'dan; END yalnız birleşik açılmış akışı bitirir (ayrı açılmışta `spurt_refused`); bir kayıttan ≤ 3 olay.
- **`on_bundle` 1732–1754:** etkin değil → PTT-R3 (`ptt_orphan_bundles`); codec/bundle_ms farklı → DROP (S9, `bundles_mismatched`); kabulde PTT-R4 saati sıfırlanır.
- **`reopen` 1766–1808:** güncel oturum = `opened_on` ve `force_reopen` yoksa hiçbir şey; birleşik + yetenekli → sayaç sıfırlanır, `opens_resent += 1`, tele bir şey çıkmaz (P2-09); yeteneksizse ayrı kayda döner; `PTT_OPEN` `Reliable`.
- **`OpenRule` 1812–1829, `spurt_record` 1833–1861:** `i < repeat || (every > 0 && i % every == 0)` açılışlı; yüksüz END açılış taşımaz.
- **`send_record` 1892–1903:** her PTT kaydı `TrafficClass::VoRel`.
- **Testler 1905–2351** (sabitledikleri): kapanmış akış kaydının eş başına / toplam tavanı ve ömrü (sınırdan bir adım önce bastırır, sınırda bastırmaz); `stream_id` tükenmesinde tipli hata ve sarma yok; sözleşmeye aykırı depoda `Store(Corrupt)` ve PTT_OPEN gönderilmemesi; giden / gelen tablo tavanları ve açık akış değerlerinin atlanması; Debug'ın ses yükünü basmaması.
- Uyumlu: **21** (ayrı kayıtlı yolda PTT-R1…R5, PTT_OPEN/PTT_CLOSE `Reliable` (PTT-R0'ın 2026-11-22 sonrası kalan kısmı), PoC-KAPSAM-001 §2 "aktif akışın PTT_OPEN'ı her yeni oturumda bir kez yeniden gönderilir" (D5), `stream_id` kalıcı depodan ve sarmasız, bounded tablolar, PTT kaydı sıkıştırılmaz, gizlilik, karar 9'un "bilinmeyen eşe çağıranın süresi ve ayrı kayıtlar" kuralı `open_press`'te).

## Bulgular

| fid | tür | ciddiyet | konum | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| P2-01 | UYUŞMAZ | orta | ptt.rs:1189–1200, 1241–1253, 264–267 | `MEDIA_BUNDLE` ve birleşik demetler `DeliveryPolicy::DeadlineBounded` (1 s) ile gider; Transport son tarihte bırakır. Sınıf yine `VoRel` ("güvenilir ses"). | AMP §3 PTT-R0: "`PTT_OPEN`, `MEDIA_BUNDLE`, `PTT_CLOSE` Reliable taşınır; … geç gelen parçayı **düşürmez**"; PoC-KAPSAM-001 EK-2 madde 1 aynı. Kod 2026-11-22 sahip kararına ("PTT ölçütü media'nınki gibi olsun") dayanıyor; bu cümle depoda ve #175 taramasında bulunamadı (DOĞRULANAMADI). 7082724206 madde 4 yalnız bant ve ölçüm kuralını verir. Spec yaması yok (kodun kendi notu: "yaması sahibin tarihli kaydını bekler"). | kilitli (karar 3) — kod değişmez; sahibin tarihli kaydıyla AMP PTT-R0 ve PoC-KAPSAM EK-2 yamanmalı |
| P2-02 | UYUŞMAZ | orta | codec/mod.rs:659–703; limits.rs:95–123; ptt.rs:1446–1449, 1574–1600 | İlk baytı 0x8A–0x8D olan kayıt, `msg_version` bilinmeyen sayılıp DROP edilmek yerine PTT biçimi olarak çözülür. 0x8A kompakt demet alıcıda HİÇBİR bayrağa bağlı değildir: varsayılan ürün alıcısı onu kabul eder. 0x8B–0x8D ise `coalesce` kapalıyken PTT katmanında atılır. | AMP §2 "`msg_version` 0x01 (v1); bilinmeyen → DROP"; §6 "`msg_version` bilinmeyen → DROP · başlık değişimi = `msg_version` artar". Sahip 2026-11-22 (7082724206 m.4): "AMP biçim değişikliği (bundle_ms u8 sınırı, başlık) **adayda sınanmak üzere** onaylı". Spec yaması yok. | kilitli (karar 3) — kodun değişmesi gerekmez; ürün alıcısına girişi için spec yaması ya da 0x8A'nın `coalesce`/yetenek bayrağına bağlanması sahip kararı ister |
| P2-03 | UYUŞMAZ | düşük | limits.rs:125–165; playout.rs:302–305; ptt.rs:323 | `bundle_ms` u8 kodu > 200 ise 10 ms adımlı süre (210 → 300 ms, 255 → 750 ms). | AMP §3 0x06/0x0A `bundle_ms u8` (ms; dönüşüm tanımı yok). Sahip 2026-11-22 (7082724206 m.4) "bundle_ms u8 sınırı" değişikliğini aday için onayladı; karar 9 300 ms pilotu seçer. Spec yaması yok. | kilitli (karar 3) — spec yaması sahipte |
| P2-04 | TANIMSIZ | orta | ptt.rs:457–469 (`PttStarted.bundle_ms` "Demet süresi (ms)"); kama-node `event.rs:105` | Olay alanı ms diye belgelenmiş ama tel KODUNU taşır. Pilot akışta (300 ms) 210 gelir. kama-node bunu `NodeEvent::PttOpened{bundle_ms}` ("ms; yorumlanmaz") olarak mobil tarafa geçirir. Kotlin'in 256 × bundleMs kuyruk tavanı ve aralık hesabı 210 ms okur. | T1-SERITLER Ek A / SAHIP-KARARI-v4 §10 T1 "Demet kuyruğu zaman tabanlı sınırlı (256 × bundleMs)". Kodun dönüştürücüsü `bundle_ms_from_code` yalnız Rust tamponunda kullanılıyor. | kilitli (karar 3) — en az belge "kod; ms için `bundle_ms_from_code`" diye düzeltilmeli (davranış-nötr); olayın ms taşıması dondurma sonrası karar |
| P2-05 | UYUŞMAZ | orta | ptt.rs:40–77, 1008–1024, 1240–1256, 1602–1720, 1863–1872 | Birleşik (PTT-BİRLEŞİK) akışta `PTT_OPEN`/`PTT_CLOSE` tele çıkmaz. Açılış demetlere yayılır, kapanış END işaretidir. Hareketsizlik sonu PTT-R4'ün 120 s'si yerine `spurt_gap_end` (≈ 1,5–2,3 s). END ayrı açılmış akışı bitirmez. | AMP PTT-R0 (OPEN/CLOSE Reliable), PTT-R4 (120 s). Sahip 2026-11-23 (7087443631: ilk temas üç kaynak, K = 2 → 3) ve karar 9 (2026-11-25: "PoC testi 300 ms pilotu açıkça seçsin … V2 ürünün genel varsayılanı olarak kalır"). `SPURT_PILOT_GAP_BUNDLES = 4` için atıf yapılan "SAHİP KARARI 2026-11-23" metni bulunamadı (DOĞRULANAMADI). Spec yaması yok. | kilitli (karar 3, karar 9) |
| P2-06 | TANIMSIZ | düşük | ptt.rs:269–285 | `SPURT_OPEN_EVERY = 4`: ilk K demetten sonra her dördüncü demet açılış bilgisi taşır. `SPURT_OPEN_REPEAT = 3`. İkisinin belgesi de "sahip onayı bekliyor" der. | 7087443631 m.3 yalnız K'yı verir ("K = 2 ile başlanacak … yetersizse K = 3 denenecek"). Periyodik yeniden açılış hiçbir kararda bulunamadı (aranan: #175 2026-11-22/23 sahip kararları, KARAR-KAYDI). | kilitli (karar 3: "tekrar" dondurulmuş) — değerlerin sahip onayı kaydedilmeli |
| P2-07 | TANIMSIZ | düşük | ptt.rs:264–267, 287–290 | `PTT_BUNDLE_DEADLINE = 1 s` ve `SPURT_RX_HOLE_WAIT = 100 ms`, client-core'daki `runtime::PTT_DEADLINE_NS` ve `transport::PTT_HOLE_SKIP` ile elle eşit tutulur. Derleme zamanı bağı yok, biri değişirse sessizlik sonu sessizce yanlış türetilir. | CLAUDE.md §9 (türetilmemiş sabit yok) / proje kuralı "magic-constant yasak". client-core kama-app'e bağımlı olamaz (`client-core/Cargo.toml:21`); kama-node ikisine de bağlı. | kod-duzeltmesi — kama-node'da `const _: () = assert!(...)` ile iki eşitlik (client-core sabiti `pub(crate)`'ten `pub`'a). Davranış-nötr. |
| P2-08 | TANIMSIZ | düşük | ptt.rs:1085, 1247 | Son tarih `std::time::Instant::now()` ile kurulur, motorun geri kalanı enjekte `MonotonicClock` kullanır. Sahte saatli test son tarihi yönetemez, iki saat sınıfı karışık. | CLAUDE.md §9 / ADR-17 belirlenimlilik ruhu. Spec'te saat sınıfı kuralı bulunamadı (aranan: AMP §3 PTT, signaling-seam). | kilitli (karar 3: PTT zaman aşımı) |
| P2-09 | TANIMSIZ | düşük | ptt.rs:1783–1787 (sayaç belgesi 642–644) | Birleşik ve yetenekli akışta oturum değişince tele hiçbir şey gitmeden `opens_resent` artar. Belge "Transport'un kabul ettiği yeniden `PTT_OPEN`'lar" der. | Kendi sayaç sözleşmesi; spec'te sayaç yok. | kod-duzeltmesi — ayrı sayaç (`spurt_reopens`) ya da artırmayı kaldır. Gözlem, davranış-nötr. |
| P2-10 | TANIMSIZ | düşük | ptt.rs:852–853; ptt.rs:345–354 | `set_spurt_open_every` belgesi "varsayılan `SPURT_OPEN_EVERY` = her demet" der, değer 4'tür. 345–354'teki belge bloğu `SPURT_PILOT_BUNDLE_MS`'ten kopup `SPURT_PILOT_BUNDLE_CODE`'a yapışmış. | Belge doğruluğu (CLAUDE.md §9). | kod-duzeltmesi (yalnız belge) |
| P2-11 | UYUŞMAZ | düşük | codec/mod.rs:691–698 (test 1018–1020 sabitliyor) | 0x8A kompakt kayıtta tavan `5 + MAX_BODY_BYTES`. Kodlayıcının en uzun kaydı `1 + MAX_BODY_BYTES` olduğu için 4 B fazlası kabul edilir. 0x8B–0x8D'de bu #233'te kanonik `1 + MAX`'a çekilmişti (666–669). | AMP §2 A1 "tek canonicalization", §4 `body_len ≤ MAX_BODY_BYTES` (işaretli biçimde gövde = işaretten sonrası, body.rs:216). | kilitli (karar 3: PTT biçimi). Meşru trafikte davranış-nötr; dondurma sonrası `1 + MAX`, test 1018'de `1+MAX+1`. |
| P2-12 | EKSİK | orta | codec/compress.rs:33–46, 50–60; mod.rs:476, 502 | TEXT varsayılan sıkıştırması kapalı. zstd kodlayıcı ve çözücü yok. Alıcı geçerli sıkıştırılmış kaydı `zstd-decoder-unavailable` ile atar; bu gerekçe spec'te yok. | AMP §5 kural 6 "`TEXT` → ON"; app-rpc §3 AMENDMENT-4 "`compress = zstd; metin mesaj için default ON`". Spec'te alıcının çözücüsüz davranışı tanımlı değil. | karar-gerekir (zstd bağımlılığı ADR'de yok; ayrıca karar 3 metin gönderim politikasını dondurdu) |
| P2-13 | ÖLÜ | düşük | codec/compress.rs:63–80, 108–114; mod.rs:634–638 | `zstd_compress` hep `None` döndüğü için kural 7 dalı, `FLAG_COMPRESSED` / `AppCompressed` kodlama yolu ve alıcıda kural 2 dalı erişilemez. Tipli stub bilinçli. | CLAUDE.md §6 (yarım kod = tipli stub, uygun). | karar-gerekir (P2-12'ye bağlı) |
| P2-14 | ÖLÜ | düşük | playout.rs (tamamı); ptt.rs:863–865, 1176–1180, 846–856 | Ürün yolunda çağıran yok. `Ptt::set_playout` yalnız `tests/ptt.rs`'te; kama-node / kama-mobile çağırmıyor. `set_compact_bundles` (kol A gönderici) ve `set_spurt_open_repeat/every` de yalnız testlerde. Kompakt alıcı yolu canlıdır, birleşik akış onu kullanır. | T1-SERITLER §7 "gösterim; kapılı PoC ölçümü Aralık"; 7082724206 m.4 (kol A aday denemesi). | karar-gerekir (gösterim/deney kollarının kalıp kalmayacağı sahipte; karar 3 sürerken dokunulmaz) |
| P2-15 | EKSİK | orta | signaling.rs:214–224, 402–414 | Blob tek AMP kaydına sığmak zorunda: ürün biriminde (`RuntimeConfig::MAX_UNIT_BYTES = 1024 − önek`, Crypto sınırıyla kırpılır) en çok ≈ 1 KB − 7 B. Parçalama yok, aşan blob `TooLarge` alır. | signaling-seam-v0 §3: "max_blob_size: tek signaling mesajı ≤ birkaç KB (SDP+ICE tipik) — büyük ICE bundle fragment". Seam v0 dondurulmuş değil ("API'Yİ DONDURMAZ"). AMP §4 büyük içeriğin kayıt düzeyinde bölünmesini Application'a verir. | karar-gerekir (blob parçalama ya da medyanın blob tavanı; seam → kontrat) |
| P2-16 | UYUŞMAZ | düşük | signaling.rs:89–96 (belge), 426–433, 587–592 | Belge "blob'un teslim çabası gönderimden itibaren süreyle sınırlıdır (deadline-bounded reliable)" der. Her sunum ise son tarihsiz `DeliveryPolicy::Reliable`. Aynı oturumda kabul edilmiş blob uzun bir kesintiden sonra 60 s'den geç teslim edilebilir; TTL yalnız yeniden sunumu sınırlar. | AMP §3 K4G-14a: sunum `Reliable`, TTL yeniden sunum içindir (kod buna uyar). signaling-seam-v0 §3: "deadline-bounded reliable". Sahip gerekçesi "60 s'den eski bir teklifin yeniden sunumu bayat çağrı üretir". | karar-gerekir (`DeadlineBounded{sent_at + 60 s}` yeni kural olur; en az belge cümlesi düzeltilmeli) |
| P2-17 | TANIMSIZ | düşük | signaling.rs:614–622; 455–467 | (a) Geçici ret kümesi {Backpressure, UnknownSession, SessionSuperseded, NotConnected}, gerisi kalıcı. (b) Eşin güncel oturumu yokken gelen blob da teslim edilir. | AMP §3 K4G-14a yalnız "geçici retlerde kabul edilene dek" der, kümeyi tanımlamaz (aranan: AMP §3, S1 sözleşmesi `kama-contracts/src/s1.rs` belgesi taranmadı → DOĞRULANAMADI). (b) için spec'te kural bulunamadı. | karar-gerekir |
| P2-18 | TANIMSIZ | düşük | ptt.rs:1475–1478, 1381–1390, 1539–1568, 1744–1747, 1000–1002 | Spec dışı alıcı ve gönderen kuralları: oturumsuz eşten `PTT_OPEN` DROP; eşin oturumu kalmayınca gelen akışlar `PttEnded`; PTT-R1 "bilinen" = etkin ∪ 120 s'lik kapanmış akış kaydı (eş başına 2); codec/bundle_ms uyuşmazlığında demet DROP (S9); eş başına tek giden akış. | AMP PTT-R0…R5 bunları tanımlamaz. Kod S7/S8/S9 (Abkarada 2026-10-08) ve plan D5'e atıf yapıyor; bu metinler depoda yok (DOĞRULANAMADI). | kilitli (karar 3) |
| P2-19 | TANIMSIZ | düşük | codec/mod.rs:671–677; ptt.rs:1627–1630, 663–664 | Açılış bilgili demette `codec` kodek katmanında doğrulanmaz (`PTT_OPEN`'da registry dışı codec `unknown-enum-value` ile DROP). PTT katmanında üç farklı neden (bayrak kapalı, bilinmeyen codec, ayrı akışa END) tek `spurt_refused` sayacına karışır. | AMP §3 0x06 "codec u8 (0x01 opus · 0x02 lyra)"; bilinmeyen enum → DROP (§3 genel kural). | kilitli (karar 3) |

## Okunan otorite bölümleri

- `specs/application-message-protocol.md` 1–142 (tamamı: YAMA KAYITLARI, §1–§8, PTT-R0…R5, K4G-09, K4G-14a).
- `specs/signaling-seam-v0.md` 1–70 (tamamı).
- `specs/application-protocol-and-rpc-spec.md` 100–113 (§3 AMENDMENT-4/6 sıkıştırma); PTT/SIGNALING/MEDIA_BUNDLE grep'i (bu dosyalarla ilgili başka hüküm yok, Plane 1 kapsamı).
- `docs/decisions/KARAR-KAYDI-2026-11.md` 1–148 (tamamı; PTT/kodek hükmü yok).
- `docs/decisions/PoC-KAPSAM-001.md` PTT satırları (26, 56, 62, 98, 105; grep).
- `docs/decisions/SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md` 34–108 (§3 T1 oynatma, §10 T1 eki).
- `docs/design/T1-SERITLER.md` §7 ve Ek A satırları (grep: KK-T1-11/19/20, T aralığı, 256 tavanı).
- #175 sahip kararları: 7171705736 (dokuz karar; karar 3 ve 9), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 (bu dosyalara doğrudan hüküm yok); ek olarak 7082724206 (2026-11-22 kararları, madde 4 AMP biçimi), 7070112500 (PTT bant hedefi), 7087443631 (2026-11-23 #233 kararları). 2026-11-21 sonrası #175 yorumlarında "PTT ölçütü media'nınki gibi", "SAHİP KARARI 2026-11-23" (`SPURT_PILOT_GAP_BUNDLES`) arandı, bulunamadı.
- CLAUDE.md §4 (INV-ENGINE-SOC; bu dosyalar transport'a anahtar vermiyor, uyumlu), §5 ADR-05 (varint yok, uyumlu), §6, §8, §9.
- `kama-contracts/src/s1.rs` 75–100 (`TrafficClass::VoRel` "güvenilir ses (PTT; D-10 B)").
