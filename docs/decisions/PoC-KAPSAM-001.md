# PoC-KAPSAM-001 — Traser saha PoC kapsamı (KARAR)

| Alan | Değer |
|---|---|
| Karar sahibi | Abkarada |
| Tarih | 2026-10-08 |
| Durum | **KESİN** — yeniden açılmaz; değişiklik yalnız Abkarada'nın tarihli ekiyle |
| Üstünlük | Kapsam konusunda bu not, diğer tüm belgelerin üstündedir: `PLAN.md` (kama-app), `KAMA-APP-ENG-kartlari_S3-S5.md`, SR-UX-002, `ui-api-v0.md`, `SPRINT-BACKLOG-FREEZE.md`. Çelişen satır üstü çizilir ve buraya atıf yapılır. |
| Yetki | Kapsam kararı yalnız Abkarada'nındır. "Zorunlu gereksinim" başlığı taşıyan istekler bu nota girmeden kapsam değildir. |
| Yerleşim | `kama-core/docs/decisions/PoC-KAPSAM-001.md` (kaynak) · `kama-app/docs/PoC-KAPSAM-001.md` ve `kama-engine/docs/PoC-KAPSAM-001.md` (kopya, "kaynak kama-core" notuyla) |

## 1. Amaç ve ölçüt

Sahne: biri trafo sahasında, biri merkezde iki Android telefon; aynı anda WhatsApp, Signal, Zello, Cellcrypt. Beş iş: PTT, sesli arama, mesaj, fotoğraf, görüntülü arama.

**Ölçüt:** kötü ağda rakipler çalışmazken Kama işlevsel kalır. Kazanılacak yer PTT ve ses; mesaj ve fotoğraf "çalışıyor" olmalı; görüntülüde eşitlik yeter.

Güvenlik iddiası demoda tek cümledir: *"Güven = önceden dağıtılmış anahtar listesi; listede olmayan hiçbir oturum açamaz."* Sertifika zinciri, PKI, sertifikasyon demoya girmez.

## 2. İÇERİDE

| Konu | Karar |
|---|---|
| Mesaj | AMP `TEXT` + `RECEIPT(delivered)`. Mesaj durumu **iki**: *gönderiliyor* / *teslim edildi* (RECEIPT ile). `AT_SERVER` üretilmez. Okundu bilgisi yok. |
| Fotoğraf | AMP `FILE_BEGIN/CHUNK/END/ABORT` + yeni `0x0B FILE_RECEIPT`; telefonda yeniden kodlama (hedef ≈150 KB; `ImageDecoder` + yeniden kodlama, EXIF/GPS kalmaz). Bütünlük BLAKE2s-256 (`whole_file_hash`). |
| PTT | Transport üstünde **Reliable** klip: `PTT_OPEN` (Reliable) → ses `MEDIA_BUNDLE 0x0A` içinde Lyra çerçeveleri → `PTT_CLOSE`. Alıcıda progresif oynatma; ağ takılırsa ses **bozulmaz, durur** ("bekliyor" göstergesi). Hedef: bas → ilk ses ≤ 1,5 s (iyi ağ) / ≤ 3 s (kötü ağ). Aktif akışın `PTT_OPEN`'ı her yeni oturumda bir kez yeniden gönderilir. |
| Sesli / görüntülü arama | Medya motoru (Resul–Hasan), mevcut E2E oturum anahtarıyla; sinyalleşme `SIGNALING_BLOB 0x09` içinde **protobuf**. Kurulum ≤ 1 s (0-RTT); kötü ağda çözünürlük düşer, arama **düşmez**. ~~Medya `DeliveryPolicy::BestEffort` / `DeadlineBounded` ile gider (ARQ yok, geç gelen düşer).~~ **EK-3 (2026-10-12):** kama transport PoC'de yalnız Reliable taşır (motor birim terk etmiyor; gerçek BestEffort = BatchPayload geçişiyle, sonraki sprint). Canlı medya paketleri PoC'de **medya motorunun kendi UDP yolundan** gider: anahtar oturumdan türetilir ve `SIGNALING_BLOB` ile taşınan kurulum mesajıyla bağlanır; geç gelen paketi medya motoru düşürür. Hub bu paketleri de yalnız iletir. Yol ve anahtar türetimi medya ekibi + Abkarada kararıdır (hub sprintinde yazılır). |
| Tek hub | Tek VPS (İstanbul), UDP, Docker; Yaaz deploy eder. Hub **hiçbir şeyi açmaz**; `hub_handle` ile iletir. Hub kodu bir sonraki sprintin işi; bu sprintte INT-02 direct-mode. **EK-2:** hub **saf röledir**; store-and-forward (mailbox) PoC'de **yoktur**. Alıcı çevrimdışıyken mesaj gönderenin kalıcı outbox'ında ve transport ARQ'sunda bekler. |
| Süreç ömrü | **Kalıcı foreground service** (tip `specialUse`; APK yan yüklü, Play beyanı yok; Hasan doğrular). Heartbeat `HEARTBEAT/HEARTBEAT_ACK 0x1B/0x1C`, ön plan 8 s / arka plan 25 s. `K=1024` sonrası `SESSION_REBIND`. Süreç sahada hiç ölmez. |
| Provisioning | `kama-provision` CLI laptopta cihaz başına config üretir: kendi anahtarları, tüm eşlerin `dest_hash` + statik X25519 + CompactDeviceCert, hub adresi ve anahtarı. Dosya telefona aktarılır; uygulama uygulama-özel dizine (`noBackupFilesDir`) yazar. QR yalnız gösteriş, opsiyonel. **EK-1:** kök gizli anahtarı (`root.json`) yalnız laptopta kalır; cihaz config'ine, Git'e, release paketine girmez. Her CLI koşusu yeni kimlik üretir; yeniden kurulum = yeniden provisioning. |
| Anahtarlar | Statik X25519 + Ed25519, CompactDeviceCert **test köküyle**; INIT'te `cert_mode = ref`. Zincir/iptal doğrulaması **yok**: önceden doğrulanmış eş dizini güven çapasıdır (ccr §35.4 kapalı-sistem kipi, PR #101). |
| Kalıcılık (Rust) | `redb` (saf Rust): ~~kimlik/provisioning,~~ eş başına `msg_id` / `stream_id` / `transfer_id` sayaçları, `init_counter`, dedup penceresi, outbox~~, mesaj geçmişi, ek blob dizini (dosya sistemi)~~. **EK-1:** kimlik ve eş dizini provisioning dosyasından bellekte; mesaj geçmişi PoC'de kalıcı değil (K2: sahada uygulama yeniden başlatılmaz); alınan dosya diske yazılır, yolu olayla verilir. **At-rest şifreleme PoC dışı** ~~(Android sandbox + FLAG_SECURE yeter)~~ **EK-2:** cihaz gizli anahtarları ve redb Android uygulama-özel depolamasında tutulur; bu PoC için **kabul edilmiş risktir**. `noBackupFilesDir` yedeklemeyi sınırlar. `FLAG_SECURE` yalnız ekran/yakalama yüzeyi içindir, at-rest koruması **sayılmaz**. |
| Android | minSdk 28 · tek süreç (`android:process` yasak) · FLAG_SECURE açık · `allowAudioPlaybackCapture="false"` · ADM çıkışı `VOICE_COMMUNICATION` · ölçüm overlay'i (RTT, kayıp, bitrate, PTT bas→ilk ses, epoch, son heartbeat yaşı; ekranda ve logda). |
| Sözleşmeler | S1 `kama-contracts::s1` (FROZEN) · S3 `kama-contracts::s3` v1.1 (FROZEN) · **S4** Transport↔Hub `KM-SEAM-TH-001` ~~(bu sprintte taslak, hub sprintinde FROZEN)~~ **EK-2:** bu sprintte NON-GATING arayüz taslak notu (plan K4G-13); yapılmazsa hub sprintinin ilk kartı; hub sprintinde G-S4 ile FROZEN · **S5** kama-app↔Kotlin = `ui-api v0.3` (Hasan; FROZEN sayılır, değişmez; Rust karşılığı UniFFI yüzeyi). |

## 3. DIŞARIDA

Bu PoC'de yazılmaz. Biri "gerekli" derse tek soru: **trafo sahasındaki telefonda karşılığı var mı?**

PKI / RA / HSM · sertifika zinciri ve iptal doğrulaması · revocation / CRLite / CT · discovery (VOPRF), DHT, hub-mesh, çoklu hub, failover · ~~mailbox kotası / prune~~ **hub mailbox / store-and-forward (EK-2: tamamen dışarıda; hub saf röle)** · `DELIVERY_STATUS` · okundu bilgisi · enrollment akışı (davet, telefon, SMS kodu, admin onayı) · SMS servisi · push (FCM, HMS, UnifiedPush) · Google / Huawei servisleri · cihaz sağlık kontrolü · Direct Boot · Play politikaları · HarmonyOS · çoklu cihaz · grup · ratchet · ciphertext-domain FEC (ARQ-only) · HubChannel gerçek hop anahtarı (no-op kalır) · hop-level path migration · Keystore / StrongBox anahtar saklama · at-rest DB şifreleme · Telecom kill switch · internetsiz kanal · zstd sıkıştırma (`compression_id = NONE`; çözücü kararı sonraki sprint) · uzun TEXT bölme (tek TEXT ≤ 1002 B; demo mesajları kısa).

## 4. Kurallar

**K1 — SICAK-SICAK ÖLÇÜM.** Karşılaştırma yalnız iki taraf da kurulu bağlantıdayken yapılır. Kama "sıcak" = her eş için epoch-1 `Confirmed` + hub kanalı bağlı + son 25 s'de heartbeat yanıtı (overlay gösterir). Rakipler "sıcak" = uygulama açık, son 1 dk'da bir mesaj teslim edilmiş. Gerekçe: rakiplerin TCP+TLS oturumu ve sertifika zinciri iyi koşulda önceden kurulmuştur; soğuk Kama ile sıcak rakibi karşılaştırmak transport farkını ölçmez.

**K2 — ISINMA PROTOKOLÜ.** Merkezde Wi-Fi'de: iki telefon oturum kurar, iki yönde epoch-1 onaylanır, rakip uygulamalarda birer mesaj atılır, saat kaydedilir; sonra saha. Sahada hiçbir uygulama yeniden başlatılmaz; çöken "çöktü" diye kaydedilir, yeniden başlatılıp ölçüme devam edilmez.

**K3 — SOĞUK BAŞLANGIÇ İKİNCİL ÖLÇÜM.** Kama soğuk INIT = tek datagram, 0-RTT (`early_unit` ilk mesaj); CompactDeviceCert ≈ 152 B, TLS zincirinden küçük. Sahada ayrıca ölçülür ve raporlanır; birincil karşılaştırmaya girmez.

**K4 — IP DEĞİŞİMİ = LİNK KAYBI.** Transport'ta adres değişimini yaşatan mekanizma **yoktur**: `hub_handle` demux niyeti var (transport-tdd "NAT-robust"), dönüş yolu güncelleme kuralı ve path validation yok; `INV-RESUME-1` ayrı resume çerçevesi tanımlamaz. Kodda hub tarafı eşi `SocketAddr` ile tanıyor. PoC davranışı: `PlatformSignal.LinkChanged` → hop bağı düşer → her aktif eş için `SESSION_INIT_REF` + `early_unit` (0-RTT, tek datagram) → alıcıda provisional supersession (DR-CONNECT-1 H6) → teslim edilmemişler `msg_id` ile yeniden. Ölçülür, iddia edilmez. Gerçek path migration HubChannel gerçek anahtarıyla (M4) gelir: mühürlü heartbeat yeni adresten = adres kanıtı.

**K5 — HUB EŞ KİMLİĞİ = `hub_handle`** (`SocketAddr` değil). Bir handle'ın dönüş adresi = o handle için son bind/INIT'in geldiği adres; veri datagramıyla **güncellenmez** (hijack yüzeyi). Bilinmeyen handle → sessiz drop. Heartbeat yalnız mevcut PeerSlot'u yeniler, slot yaratmaz.

**K6 — FACADE DEĞİŞMEZ, DAR İMPLEMENTE EDİLİR.** `ui-api v0.3` yüzeyi aynen kalır; gerçek motor PoC'de: Enrollment → provisioning içe aktar → `Ready` (`FakeSeed.Mode.Enrolled` eşdeğeri) · Contacts = provisioning listesi, `syncDirectory`/`lookupUsername` etkisiz · Devices tek satır · Push, TrustFailure çıkışı, Gate-3 bildirimi etkisiz gövde · `background.drain()` = ~~yeniden bağlanınca mailbox çek~~ **EK-2:** yerel outbox ve transport yeniden gönderim kuyruğunu yeniden pompala, gerekirse `connect`; hub mailbox yok (periyodik worker **yok**) · `notifications.state`, `attachments.open`, `onPlatformSignal`, `CallsApi` **kullanılır**.

**K7 — SERİLEŞTİRME.** AMP kayıt gövdeleri (`TEXT/FILE_*/PTT_*/RECEIPT/FILE_RECEIPT`) ve `MEDIA_BUNDLE` başlığı sabit genişlik (AMP A1); `SIGNALING_BLOB` içi ve medya kontrol mesajları protobuf. Protobuf paket-başına medyaya girmez.

**K8 — DEMO ÖLÇÜTLERİ** (saha koşulu; aynı telefon, aynı yer, aynı 5 dk, dört uygulama sırayla, overlay açık):

| Özellik | Hedef | Rakipte bakılan |
|---|---|---|
| PTT | bas→ilk ses ≤ 1,5 s / ≤ 3 s; sıfır bozuk hece | Zello: hece kaybı / bozulma |
| Mesaj | teslim ≤ 2 s / ≤ 10 s; %100; 60 s kesinti sonrası aynı oturum | WhatsApp/Signal: "gönderiliyor"da takılma |
| Sesli arama | kurulum ≤ 1 s; 10 dk kesintisiz, kopma 0 | Cellcrypt/WhatsApp: kopma, kurulum |
| Görüntülü | kurulum ≤ 2 s; çözünürlük düşer, arama düşmez | kopma |
| Fotoğraf | 1 MB → ≈150 KB; 100 kbps'te ≤ 30 s; %100 | aktarım süresi, başarısızlık |

**K9 — SAHİPLİK.** Rust (kama-core: kripto, transport, Rust application katmanı, UniFFI yüzeyi, hub): Abkarada karar, Claude Code icra; Hayri sonraki sprintten itibaren Rust application. Kotlin facade + fake + Android servis katmanı + UI köprüsü: Hasan. UI tasarımı: Meriç. Hub deploy, CI, APK/AAR hattı: Yaaz. Görüntü: Resul. Hiçbir belge Abkarada'ya tarihli teslimat atamaz; Rust tarafı teslim tarihleri kama-core sprint planından gelir.

**K10 — GÜVENLİK DÜZELTMELERİ PoC'DE ATLANMAZ.** Kapsam daraltması tasarım iddiasını daraltır, bilinen kusuru bırakmaz. Örnek: açılan çerçevenin `ProtectionContext`'ine bakmayan runtime (sahte ACK ile ARQ birimi sildirme) PoC'den önce düzeltilir.

## 5. Bilinen ve beyan edilen PoC açıkları

Demoda söylenmez; sorulursa açıklanır; INT-02 ve demo raporuna yazılır.

| Açık | Neden kabul | Kapanış |
|---|---|---|
| INIT meta alanları (`recipient_dest_hash`, `sender_cert_ref`, `init_counter`) telde açık | HubChannel gerçek hop anahtarı yok | M4 |
| Hop'ta kimlik doğrulama yok; hub ACK'i direct-mode'da Session bağlamında | Hop anahtarı yok, hub yok | M4 / hub sprinti |
| IP değişiminde yeniden handshake (0-RTT) | §4 K4 | M4 |
| INIT yeniden gönderim kadansı yok | DR-CONNECT-1 IMPL hub sprintinde | hub sprinti |
| zstd yok | çözücü kararı | sonraki sprint |
| Tek TEXT ≤ 1002 B | fragmenter yok | sonraki sprint |
| At-rest DB şifreleme yok | Keystore kapsam dışı | M6 |
| Yanıtlayan tarafta eş başına provisional oturum tavanı yok (KS-DOS-1) | istismar için dizindeki bir eşin anahtarı gerekir | sonraki sprint |
| `H-PROGRESS` gerçek hedefte yalnız `Measuring` | `Suspended/LongOutage` tetikleri hub sprintinde | hub sprinti |
| **(EK-1)** Kalıcı durum tamamen kaybolup (veri silme, yeniden kurulum) aynı eski kimlik aynı provisioning dosyasıyla yeniden kullanılırsa `msg_id` tekrarı ve yanlış "teslim edildi" mümkündür | yerel tespit imkânsız (geçmişi bilen kaynak yok); kural: yeniden provisioning zorunlu | PoC sonrası (sunucu tarafı kimlik epoch'u) |
| **(EK-1)** Mesaj geçmişi kalıcı değil; uygulama kapanırsa sohbet ekranı boşalır | K2: sahada uygulama yeniden başlatılmaz | sonraki sprint (D8 KATMAN 6) |
| **(EK-1)** Cihaz gizli anahtarları düz JSON'da (`noBackupFilesDir`; ~~FLAG_SECURE~~ EK-2: at-rest koruması yok) | at-rest şifreleme ve Keystore kapsam dışı | M6 |
| **(EK-2)** Hub store-and-forward yok: alıcı çevrimdışıyken mesaj gönderenin outbox'ında bekler; **iki taraf aynı anda çevrimdışıysa** ikisi de dönene kadar teslim yok | hop-ARQ (M4) olmadan hub mailbox doğru çalışamaz; FGS + kalıcı outbox + ARQ PoC için yeter | hub sprinti sonrası (M4 ile) |

## EK-1 — 2026-10-11 (Abkarada; KAMA-PLAN-4GUN v1.1 ile birlikte)

Değişenler: §2 "Kalıcılık (Rust)" daraltıldı (kimlik/eş dizini depoya girmez; geçmiş ve blob dizini PoC dışı); §2 "Provisioning" satırına kök gizli anahtarı sınırı eklendi; §5'e üç beyanlı açık eklendi. Başka hiçbir satır değişmedi; K1–K10 aynen geçerli.

## EK-2 — 2026-10-11 (Abkarada; Claude Code kapsam incelemesi sonrası; bu belge bundan sonra **yeniden açılmaz**)

1. **PTT taşıma:** §2 PTT satırı olduğu gibi geçerlidir (Reliable klip). Plandaki K4G-10 yalnız canlı sesli/görüntülü arama medyası için BestEffort/DeadlineBounded uygular; PTT `PTT_OPEN`/`MEDIA_BUNDLE`/`PTT_CLOSE` Reliable taşınır, geç gelen ses düşürülmez, progresif oynatma durur ve devam eder (plan K4G-08 PTT-R0).
2. **Hub mailbox PoC dışı:** §2 "Tek hub" satırı, §3 listesi ve K6 `background.drain()` buna göre düzeltildi. Hub saf röledir; store-and-forward yoktur; §5'e beyanlı açık eklendi.
3. **`FLAG_SECURE` at-rest gerekçesi değildir:** §2 "Kalıcılık" ve §5 satırları düzeltildi; at-rest koruması yok, kabul edilmiş PoC riski.
4. **S4 NON-GATING:** §2 "Sözleşmeler" satırı plan v1.1 K4G-13 ile hizalandı.

## EK-3 — 2026-10-12 (Abkarada; DUR-K4G-10 ikinci cevap sonrası; tek satır, belge yine kapalı)

§2 "Sesli / görüntülü arama": kama transport PoC'de yalnız Reliable; canlı medya medya motorunun kendi UDP yolundan, oturumdan türetilen anahtarla. BestEffort/DeadlineBounded transport'ta sonraki sprint (BatchPayload geçişiyle). PTT satırı değişmedi (Reliable klip). §5'e beyan: "Canlı medya kama transport üzerinden sınanmadı; medya yolunun anahtar türetimi hub sprintinde yazılır."
