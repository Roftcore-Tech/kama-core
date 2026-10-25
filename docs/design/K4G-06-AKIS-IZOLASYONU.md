# K4G-06 — Akış izolasyonu ve teslim sözleşmesi — tasarım notu (ADIM 1: sözleşme/arayüz taslağı + ayırt edici test planı)

## 0. Künye

- **Kart / dal:** K4G-06 · `k4g/06-tasarim` (taban main `1b1425d2`) · **TASLAK**. Kod yok, ölçüm yok, deney dalı (`k4g/06-deney`) AÇILMADI (EK-1 §3 adım 2, ayrı tur).
- **Otorite sırası:** CEVAP 15 EK-1 > CEVAP 15 v3 (§9, §5, §6, §2, §7) > 14 > … ; sahibin metni `SAHIP-K4G-06-GEREKSINIM.txt`; sahibin iki bağlayıcı ek notu (PTT eşikleri kalibrasyondan sonra sabitlenir ve YENİ koşuyla doğrulanır · "kapasite reddi = 0" yalnız geçerli krediye uyan güvenilir veri için). İnceleme kayıtları (INCELEMECI-6/-7) kanıt girdisidir, talimat değil.
- **Atıf biçimi:** `SHA:dosya:satır`. Dosya adları kısaltılmıştır; tam yol: `runtime.rs`/`transport.rs`/`wire.rs` = `crates/client-core/src/`; `sender.rs`/`arq.rs` = `crates/adaptive-transport/src/`; `s1.rs`/`handles.rs` = `crates/kama-contracts/src/`; `messaging.rs`/`ptt.rs`/`file_transfer.rs` = `crates/kama-app/src/`; `node.rs` = `crates/kama-node/src/`; `envelope.rs` = `crates/kama-noise-wire/src/`.
- **SHA'lar (origin'de doğrulandı, 2026-10-25):** `0c22a07d` = `k4g/05c-kod` (05b + 05c birleşik kod; öneksiz atıfların çoğu bu SHA'dadır) · `1b1425d2` = main · `cf270a6c` = `k4g/05d-receipt` · `e5514e7a` = `k4g/05f-dosya-ttl` · `9e0ea164` = `k4g/05b-retx-fix` (DUR-38 Ek B.6) · `0b216f07` = `k4g/05g-sahte-ready` origin ucu. 05g yerel ucu `c9c2ea22` origin'de DEĞİL → **YEREL**; bu notta ona dayanan hiçbir sonuç onaya girmez.
- **Etiketler:** [olgu] = koddan okundu · [hesap] = aritmetik, ölçülmedi · [çıkarım] = olgulardan akıl yürütme, test edilmedi · [öneri] · [CAL] = deney başlangıç değeri, son değer ölçümle.

## 1. Amaç ve değişmeyenler

**Amaç.** Tek güvenli oturum altında birden çok mantıksal akış: akış başına sıra, WMM-örnekli sınıf önceliği (PTT → AC_VO, TEXT → AC_BE, FILE → AC_BK; AC_VI yok), alıcının ÖNCEDEN ilan ettiği kredi, ortak ve adil gönderim bütçesi. Dosyanın eksik birimi ya da durmuş tüketicisi TEXT/PTT'yi bekletmez; olağan kapasite durması ACK kesilerek kayıp gibi gösterilmez ("HER BİT DÜŞMAN"). Bu bir **teslim sözleşmesi revizyonudur** (C15 v3 §9.1); gerekli donma değişiklikleri Ek A'da açık listelenir.

**Değişmeyenler.**
- **D1 Hub zarfı:** hub'ın gördüğü 22 B SESSION_DATA ve hub-server codec'i değişmez [olgu: `0c22a07d:envelope.rs:80-81` legacy 22 B, :97]. Akış alanları client-core iç çerçevesindedir, AEAD ile doğrulanmış içeriktir.
- **D2 Noise:** sayaç/nonce akış başına sıfırlanmaz; yön başına tek PN uzayı; transport ARQ kimliği PN'dir (D1a) [olgu: `0c22a07d:runtime.rs:2028-2034`]. Akış, PN'e ikinci bir anlam yüklemez.
- **D3 Doğruluk düzeltmeleri korunur:** 05b birim sahipliği (payload yalnız motor teyidinde düşer, `0c22a07d:runtime.rs:2019-2024`), 05c INV-ACK-DELIVER-1 (tek kabul kararı, ret ACK'siz, `0c22a07d:runtime.rs:3414-3423`), 05g sahte `Ready` kapanışı, 05d yükümlülük koruması (canlı RECEIPT silinmez). Çoklu akış bunların yerine geçmez; onları akış başına genelleştirir.
- **D4 INV-ENGINE-SOC:** motor payload'u ve kama-app tiplerini görmez; yalnız generic hizmet etiketi (katman, opak akış no, süre) görür (§5).
- **D5 INV-RATE-1:** tek pacer; akış başına bağımsız pacer YOK; kayıp hızı düşürmez.
- **D6 INV-FEC-1** (code-then-encrypt-authenticate-shards) aynen; FEC blokları akış başına olur (§8), sıra değişmez.
- **D7 ADR-05/ADR-17:** fixed-width big-endian, varint yok; seçim deterministik (aynı girdi → aynı karar).
- **Kapsam dışı:** Wi-Fi/DSCP işaretleme, hub rölesi/hop-ARQ, media plane (video), gerçek kodlayıcı.

## 2. Bugün neden birbirlerini bekliyorlar [olgu → çıkarım]

- **Alıcıda tek sıralama alanı:** S1 adaptörü sırayı oturum başına tek `Reorder{next, held}` ile tutar [`0c22a07d:transport.rs:370-373`], boşluğu atlamaz [:396-419]; teslimler düğüm geneli tek `inbox`a [:449; `MAX_INBOX` 1024 :118] ve düğüm geneli tek teslim kanalına [`0c22a07d:runtime.rs:3053`; `delivery_queue` 1024 :271] akar.
- **Kabul penceresi oturum geneli:** `DeliveryGate` penceresi `max_inflight_units` [`0c22a07d:runtime.rs:3447`, :3480-3482]; kanal doluysa ACK'siz ret [:3484-3493].
- **Gönderende tek FIFO + oturum geneli mesafe kapısı:** S1 `send` → tek offer kanalı [`0c22a07d:runtime.rs:1705`, 1024 :270]; alım `store.len() < 4096 ∧ distance_ok` iken [:2608], mesafe en eski teyitsiz birimden ölçülür [:2511, :2575-2581].
- **Motor sınıf körü:** `OpaqueUnit{id, size}` [`0c22a07d:sender.rs:75-80`]; yeni DATA FIFO [:2492, :7225-7247]; kopya kuyruğu başı en küçük birim kimliği [:7103-7110]; ARQ sırası kayıp ilanı anı [:3846-3849 `deadline_ns: now_ns` + `0c22a07d:arq.rs:124-127`]; parite FIFO [`sender.rs:1787`]; sınıflar arası sıra `schedule_transmit` [:4622-4691].
- **FEC blokları trafikleri bağlar:** blok = `unit_id / K`, oturum geneli ardışık kimlik [`0c22a07d:runtime.rs:2010-2014`].
- **Süre yalnız sunumda:** `DeadlineBounded` yalnız `send` anında denetlenir [`0c22a07d:transport.rs:1297-1305`]; sonra yalnız FEC politika girdisidir [`runtime.rs:2612-2613`; `sender.rs:4716-4721`]; ARQ süresi dolanı düşürmez [`arq.rs:92-94`].
- **kama-app:** PTT'nin üç kaydı da `Reliable` [`0c22a07d:ptt.rs:20-24`]; dosya pompası kanal reddedene dek doldurur [`file_transfer.rs:866-909`]; olay zinciri tek okuyucu [`messaging.rs:652-660`; `node.rs:465-485`].
- **Sonuç [çıkarım]:** (i) oturum sırası 100'deki kayıp dosya birimi 101 TEXT ve 102 PTT'yi alıcıda tutar; (ii) gönderende yeni TEXT, önündeki ≤ 1024 + 4096 dosya biriminin ve motorda eski dosya biriminin kurtarma işinin arkasındadır (05d bulgusu ile aynı: `cf270a6c:docs/design/K4G-05d-RECEIPT.md:88`); (iii) alıcı kapasitesi ilan edilmez, dolunca ACK kesilir → gönderen kayıp sanır → kurtarma trafiği.

## 3. Akış modeli

**Akış** = oturum içinde **tek yönlü** mantıksal kanal; açan (gönderen) taraf numaralar. `flow_id` u16, oturum ve yön başına monoton, **yeniden kullanılmaz** (tükenince oturum yenilenir, DUR-06-9). Akış 0 = kontrol şeridi (örtük açık; kredi tüketmez; tavanlı, §4).

**Üç kimlik (sahip §9.3 (1)):**

| Kimlik | Nerede | Rolü | Yeniden gönderimde |
|---|---|---|---|
| Paket no (PN) | Noise zarfı (`meta.full_wire_pn`) | ACK, nonce | her emisyonda yeni |
| Birim kimliği (`OpaqueUnit.id`) | yalnız gönderen runtime + motor (oturum yerel, monoton) | sahiplik, teyit, payload | sabit |
| (`flow_id`, `flow_seq`) | iç çerçeve (zarf içi) | teslim yeri + akış içi sıra; alıcının bütün kararları | sabit |

- **Telde birim kimliği yerine akış koordinatı [öneri W2, DUR-06-5]:** yeni iç çerçeve `FLOW_DATA = [type u8][flags u8][flow_id u16][flow_seq u32] ‖ payload` → 8 B başlık (bugünkü 0x00 DATA 11 B, `0c22a07d:wire.rs:3`, :17) [hesap]. `flags` = sınıf (2 bit) · kip (1 bit) · FIN (1 bit) · ayrılmış. Alıcı ACK için birim kimliğine ihtiyaç duymaz (ACK PN uzayında); kabul, dedup, sıra, FEC bloğu ve kredi akış koordinatıyla yürür. Gönderen runtime, store girdisinde birim kimliği ↔ (akış, sıra) eşlemesini tutar (girdi ≤ store). Çerçeve her emisyonda store'dan yeniden kodlandığı için [`0c22a07d:runtime.rs:2287`] bu eşleme ek kayıt gerektirmez.
- **Akış bilgisi genel sıralamadan ÖNCE görülür:** alıcının ilk kararı (kabul kapısı) akışı başlıktan okur; oturum geneli sıra alanı S1 kipinde kalkar. Akış bilgisini AMP yükünün içine koymak bu şartı karşılamaz (sahip §9.3 (1)).
- **Örtük açılış:** akış ilk çerçevesiyle açılır; sınıf/kip her çerçevenin `flags`'ında olduğu için ilk varan çerçeve hangisi olursa olsun yeter. İlk kredi sınıf başına profil sabitidir (§7.3); açılış için kontrol turu yok.
- **Teslim kipleri:**
  - `Ordered` (güvenilir akışlar): akış içinde sıralı, boşluk atlanmaz (05c R1 akış başına).
  - `Live` (canlı medya): akış içinde **sırasız** teslim; `flow_seq` uygulamaya verilir; alıcı transport tutmaz; geç/eksik kararı oynatıcıdadır [öneri, DUR-06-6]. Birim başına politika korunur: `Live` akışta `Reliable` birim (PTT_OPEN/PTT_CLOSE) yeniden gönderilir, süresi yoktur.
- **Oturum sınırı:** akışlar oturuma aittir; yenileme/supersession akış taşımaz (S1 K7 aynen); yeni oturum yeni akış uzayıdır. Bağlam-önce dağıtım [`0c22a07d:transport.rs:56-62`] eski oturumun DATA/ACK/kredisinin yeni oturuma ulaşmasını zaten engeller.

## 4. Sınıflar ve kontrol sınıfı

| Trafik | Sınıf (WMM adı = niyet) | Kip / politika | Akış |
|---|---|---|---|
| Canlı PTT | VO (AC_VO) | `Live`; demet `DeadlineBounded`, OPEN/CLOSE `Reliable` | konuşma başına bir akış |
| TEXT | BE (AC_BE) | `Ordered`, `Reliable` | oturum boyu tek akış |
| SIGNALING_BLOB | BE | `Ordered`, `Reliable` | ayrı akış [öneri, DUR-06-15] |
| Dosya | BK (AC_BK) | `Ordered`, `Reliable` | aktarım başına bir akış |
| RECEIPT, FILE_RECEIPT, FILE_ABORT | CTRL (uygulama kontrolü) | `Ordered`, `Reliable`, birim boyu tavanlı | uygulama kontrol akışı |
| Kredi / BLOCKED / RESET | CTRL (transport iç) | §7 | akış 0 ya da ACK yolu |

WMM adları Kama'da gecikme/adalet **garantisi vermez** (WMM MAC erişim kuyruklarını düzenler); garanti §5'teki zamanlayıcıdan ve onun türetiminden gelir (EK-1 §1.1).

**Kontrol sınıfı (S6, EK-1 §1.1).** `PTT > TEXT > FILE` yalnız uygulama verisi içindir. Kontrol VO'nun altında tek sıra DEĞİLDİR; ayrı bir katmandır (§5 katman 0):
- **Tavan:** kontrol token kovası (oran ρ_C·r, kova b_C) [CAL] + kontrol birim boyu tavanı `max_control_unit_bytes` (yeni yetenek alanı; aşan → `UnitTooLarge`) + bekleyen kontrol kaydı tavanı.
- **Koşullu asgari hizmet hakkı:** uygun kontrol işi varken ve kovada token varken katman 0 her şeyden önce seçilir → uzun dönemde ≥ ρ_C·r hizmet; kova boşsa bekler, diğer katmanları aç bırakmaz.
- **Veri kredisinden bağımsızlık:** kredi ilanları alıcının ACK yolundan gider (sunucu soketi; bugün ACK'in gittiği yol `0c22a07d:runtime.rs:3971-3981`) — veri akışının arkasında değildir ve kredi tüketmez; gönderenin kontrol akışı da kredi tüketmez (alıcıda protokol tavanlı CTRL havuzu). "Kontrol göndermek için kontrol bekleme" döngüsü yoktur.
- **Ayrı kaynak:** dosya kredisi bitse de CTRL havuzu ve uygulama kontrol akışı kullanılabilir (havuzlar ayrık, §7.2).

**Kontrol envanteri ve bağımlılıklar** (EK-1 §1.1 (a)):

| Kayıt | Katman / yol | Kime ilerleme sağlar | Hizmet hakkı | Maliyet sınırı |
|---|---|---|---|---|
| ACK_V2 (transport ACK, PN) | alıcı sunucu soketi; gönderen pacer'ından geçmez | gönderen uçuş/teyit/RTT | bugünkü gibi (ACK zamanlayıcısı 20 ms, `0c22a07d:runtime.rs:69`) | değişmez |
| FLOW_CREDIT (yeni) | alıcı ACK yolu | gönderenin yeni verisi | tetikli (§7.4), kredi tüketmez | birleştirilmiş; CTRL kovası |
| FLOW_BLOCKED (yeni) | gönderen katman 0 | alıcının yeniden ilanı | koşullu asgari hak | blok bölümü başına 1 + geri çekilmeli tekrar |
| FLOW_RESET (yeni) | gönderen katman 0, güvenilir | alıcının akış kaynakları | akış açılırken yeri ayrılır | ≤ açık akış sayısı |
| PARAM_SET/ACK, RECOVERED, CS_PROBE/ECHO | mevcut | FEC bağlaması, teyit, kuruluş | mevcut | değişmez (RECOVERED'a `flow_id`, §8) |
| RECEIPT(delivered) — **uygulama alındısı, transport ACK değildir** | uygulama kontrol akışı | gönderen outbox kapanışı | katman 0 | gövde 5 B |
| FILE_RECEIPT, FILE_ABORT | uygulama kontrol akışı | dosya aktarımının sonu | katman 0 | gövde 9 B / küçük |
| PTT_OPEN, PTT_CLOSE | konuşmanın VO akışı, `Reliable` | alıcının akışı başlatma/bitirme | VO; yeri açılışta ayrılır (§7.6) | konuşma başına 2 |

## 5. İki düzeyli zamanlayıcı ve seçim sözleşmesi (S7 önerisi)

**Yer: motorun içinde, generic seçim sözleşmesiyle [öneri S7(b), DUR-06-2].** `OpaqueUnit` bir hizmet etiketi kazanır: `ServiceTag { tier: u8, flow: u32 (opak), expires_ns: Option<u64> }`. Motor kama-app tiplerini bilmez; yalnız sırayı ve süreyi bilir. Seçim, motor içinde tek, saf ve deterministik bir fonksiyondur: girdi = her (akış, iş türü) için uygun baş + pacer durumu; çıktı = seçilen baş ya da rezerv. Bu, "dışarıdan yönlendirilebilir scheduler" seçeneğinin (S7(c)) test edilebilirliğini çalışma zamanı geri çağrısı olmadan verir.

**Düzey 1 — hangi akış (her pacer hakkında):**
1. **Katman 0 CTRL:** uygun iş varsa ve kontrol kovası izin veriyorsa önce.
2. **Katman 1 VO:** uygun iş varsa ve VO kovası (oran ρ_V·r, kabul edilmiş VO profilinden; kova b_V) izin veriyorsa. VO kendi süresi dolan işini biriktirmez (§7.6).
3. **Katman 2 BE + BK:** bayt kuantumlu DRR. Sınıf kuantumu Q_BE > Q_BK [CAL; başlangıç 3:1]; sınıf içinde akışlar arası DRR (iki dosya birbirini kilitlemez).
- **Tek rezerv:** seçilen (akış, iş) başı token bulamazsa rezerv onundur; katman 2'deki rakipler rezervin altına inemez; katman 0/1 yalnız kendi kovası kadar geçebilir → rezervin ömrü sonludur. Aynı anda tek rezerv (05b DUR-31 ilkesi).
- **Token yokluğu iş yokluğu değildir:** DRR açığı token yokluğunda sıfırlanmaz, sıra geçilmez. **Vadesi gelmemiş iş** (kopya `due > now`, ARQ'ya düşmemiş kayıp şüphesi) uygun iş sayılmaz.

**Düzey 2 — akış içinde hangi iş:** yeni DATA · ARQ · kopya · FEC paritesi. Kural, DUR-38'in seçilecek kuralıdır (B′ karşılık kuralı, CEVAP 15 v3 §2.1 K1 (a)–(d)); akış içinde uygulanır. Tek akışlı yapılandırmada düzey 1 etkisizdir ve davranış 05b'ninkine indirgenir → DUR-38 T1/T2 regresyon testi olarak kalır (Ek B, B17).

**DUR-38 ile ortak mantık (EK-1 §1.2 son madde).** 05b'nin kopya payı istisnası (`serve_copy_share`, `0c22a07d:sender.rs:7136-7162`) ve DUR-31 rezervi `schedule_transmit` içinde ayrı istisna olarak büyümez; tek seçici fonksiyonun düzey-2 kuralı olur. B′ birim adaletini (T1 (ii)) sağlayamazsa düzey 2 de DRR olur (hiyerarşik DRR) — aynı seçici, aynı rezerv. Testleri geçemeyen mekanizma mevcut sırayı korumak için tutulmaz.

**Aç bırakılmama — ispat taslağı [çıkarım; sonuç testte "ölçüldü" olarak yazılır, "ispatlandı" değil].** Koşullar: sabit `r ≥` taban, çalışan zamanlayıcı, BURST ≥ en büyük baş, ρ_C + ρ_V < 1 (VO kabulü bunu korur, §10).
1. Herhangi bir τ aralığında katman 0 + 1 tüketimi ≤ (ρ_C + ρ_V)·r·τ + b_C + b_V.
2. Katman 2'de backlog'lu her akış f bir DRR turunda ≥ Q_f bayt alır; bir tur ≤ Σ_g (Q_g + S_max) bayt sürer.
3. Dolayısıyla f'nin iki hizmeti arası ≤ [Σ_g (Q_g + S_max) + b_C + b_V] / ((1 − ρ_C − ρ_V)·r) + Δ_çağrı (çağrılar arası azami aralık; ürün pompası 1 ms, `0c22a07d:runtime.rs:65`).
4. f içinde birim u için T_birim ≤ (önünde hizmet alacak N_f canlı rakip) × (3) + yeniden uygun hale gelme süresi (kopya aralığı ≤ RTO_MAX = 30 s; ARQ ilanı) + durum geçişi payı; "her birim" kabul edilmiş sonlu canlı küme üzerinde tanımlıdır; birim hizmet yaşı birimin uygun hale geldiği anda başlar; PRE_CONVERGE istisnası 05b'deki gibi beyanlıdır. Türetim Ek B B14'te koddan önce yazılır.
5. Kontrol: uygun kontrol başı en geç (S_iletimde + S_ctrl) / r + kova dolum süresi + Δ_çağrı içinde çıkar.
- Örnek [hesap]: r = 300 B/s, VO yok, ρ_C = 0,1, bir BE (Q = 3150 B) ve bir BK (Q = 1050 B) akışı, S_max = 1050 B → tur ≤ (6300 B + b_C) / 270 B/s ≈ 23,3 s (b_C ihmal); sürekli TEXT altında dosya akışı tur başına ≥ 1050 B alır. Bu, ölçülecek bir üst sınırdır.

## 6. Kuyruk derinliği denetimi (S7, EK-1 §1.2)

| Katman | Bugün [olgu] | TEXT'in önünde kalabilecek dosya işi [hesap/çıkarım] | K4G-06 |
|---|---|---|---|
| kama-app dosya pompası | ret gelene dek doldurur (`0c22a07d:file_transfer.rs:866-909`) | — (FIFO'ya iter) | akış bütçesi bitince yalnız o akış `Backpressure` |
| S1 `send` → offer mpsc | tek FIFO, 1024 (`runtime.rs:1705`, :270) | ≤ 1024 birim; 31 B/s'de 1024 × 1050 / 31 ≈ 9,6 sa | akış bütçesi dışında birim kabul edilmez → kanal başında HOL yok |
| runtime store + mesafe kapısı | ≤ 4096, oturum geneli en eski teyitsiz (`runtime.rs:2575-2581`, :2608) | eski dosya biriminin kimliği yeni alımı durdurabilir | akış başına kredi kapısı; store ≤ Σ pencere + yerel tampon |
| motor `tx_queue` | FIFO (`sender.rs:2492`, :7225-7247) | ≤ 4096 birim DATA | katman/akış başına kuyruk |
| motor `pending_lost` (ARQ) | kayıp ilanı sırası (`sender.rs:3846-3849`; `arq.rs:124-127`) | daha önce kaybolan dosya birimi önce | akış seçimi düzey 1'de; akış içi `arq.rs` sırası aynen |
| motor `dup_queue` (kopya) | baş = en küçük kimlik (`sender.rs:7103-7110`) | eski dosya kopyası her zaman önce | akış başına baş, düzey 1 seçer |
| motor `pending_fec` | FIFO; blok oturum geneli (`sender.rs:1787`; `runtime.rs:2010-2014`) | karışık blok paritesi | akış başına blok (§8) |
| pacer / soket / darboğaz | tek kova; iletime başlanmış paket kesilemez | S/r (31 B/s'de 1050 B ≈ 33,9 s; 300 B/s'de 3,5 s) | değişmez — fiziksel sınır (§11) |
| alıcı teslim kanalı | düğüm geneli FIFO 1024 (`runtime.rs:3053`) | dolunca ACK'siz ret | kapasite ≥ Σ taahhüt edilmiş kredi → kredili birim için yapısal yer |
| alıcı `Reorder` | oturum başına (`transport.rs:370-419`) | dosya boşluğu TEXT'i tutar | (oturum, akış) başına |
| alıcı `inbox` | düğüm geneli FIFO 1024 (`transport.rs:118`, :449) | durmuş tüketici herkesi tutar | akış başına hazır kuyruk; `recv()` tutulmamış akışlardan |
| kama-app olay zinciri | tek okuyucu (`messaging.rs:652-660`; `node.rs:465-485`) | yavaş dosya işleyici herkesi tutar | dosya tüketicisi akışını `hold` eder; zincir sürer |

**Karşı örnek (EK-1 §1.2) ve S7(a) hükmü [çıkarım, Ek B B2 ile sınanır].** Uygulama FIFO'su boş, motorda eski dosya birimi teyitsiz ve kurtarma işi canlı; sonra TEXT ve medya sunulur. Bugünkü motorda: kopya başı en küçük kimliktir → eski dosya kopyası yeni TEXT kopyasından önce; ARQ sırası kayıp ilanı anıdır → eski dosya kaybı önce; parite FIFO'dur. Üst katmanın yeni teklifleri sığ beslemesi bu üç kuyruğu sıralamaz. **Öneri: S7(a) reddedilir** — ancak B2 mevcut motorda kırmızı çıkarsa (ön-kayıtlı beklenti); yeşil çıkarsa çıkarım yanlıştır, önce türetim sorgulanır. **S7(b) önerilir**: motor FROZEN v1.1 → yazılı revizyon (Ek A).

## 7. Alıcı kredileri

**7.1 Birim.** Akış başına iki **mutlak, artan** sınır: `max_seq` (u32; `flow_seq < max_seq`) ve `max_bytes` (u32, akışın toplam payload baytı). Gönderen yeni birimi yalnız ikisi de izin verirse gönderir. Aynı birimin yeniden gönderimi, kopyası ve FEC ile kurtarılması aynı `flow_seq`'i taşır → ikinci kez kredi tüketmez. QUIC'in stream kredisine karşılık gelir; byte-stream modeli kopyalanmaz (Kama mesaj tabanlı: birim + bayt birlikte).

**7.2 Havuzlar ve taahhüt.** Alıcı oturum başına sınıf havuzları tutar: P_CTRL (kredisiz, protokol tavanlı), P_BE, P_VO (DUR-06-1), P_BK; düğüm toplamı = `max_peers` × Σ havuz. **Taahhüt** = Σ_f (`max_seq_f` − uygulamaya verilmiş_f), kullanılmamış kredi dahil; her zaman ≤ ilgili havuz. BK kredisi yalnız P_BK'den verilir → dosya yükü TEXT/kontrol rezervini tüketemez. İlan edilmiş kredi geri çekilmez; alıcı karşılar. Bayt yanında akış sayısı (`MAX_FLOWS`, mutlak artan, sınıf başına) ve FEC blok sayısı (`RX_BLOCKS_CAP` 32, `0c22a07d:runtime.rs:3193`) sınırlıdır. Kuyruktan kuyruğa taşımak boşalma sayılmaz: kredi yalnız uygulama birimi akıştan aldığında (`recv`) ilerler.

**7.3 İlk kredi ve ilk kontrol.** Sınıf başına `W0` (birim, bayt) ve `MAX_FLOWS0` profil sabitidir; iki uç bilir. Alıcı oturum kurulunca P_class ≥ MAX_FLOWS0 × W0 ayırır. İlk birim kontrol beklemez; ilk kontrol (CTRL akışı) kredi gerektirmez.

**7.4 Güncelleme.** `max_seq = alınan + W_class`. İlan: sınır W/2 kadar ilerleyince ya da BLOCKED gelince; birden çok akış tek FLOW_CREDIT çerçevesinde. Yol: alıcı ACK yolu (§4).

**7.5 Kayıp, tekrar, sırasızlık.** Mutlak değer + en büyükle birleştirme → bayat, sırasız ve tekrarlanan kredi zararsızdır (sınır hiç azalmaz). Kayıp: gönderen kredi yüzünden durursa `FLOW_BLOCKED(flow, seq)` gönderir (blok bölümü başına bir; yanıt yoksa geri çekilmeli tekrar, CTRL kovası içinde); alıcı değişmemiş olsa da güncel krediyi yeniden ilan eder. Eski oturumun kredisi bağlam-önce dağıtımla yeni oturuma ulaşmaz.

**7.6 Yaşam döngüsü (EK-1 §1.4).**
- **Açılış:** örtük (§3); ilk kredi 7.3.
- **FIN:** son birimde bayrak. Alıcı `fin_seq`'e dek her birimi uygulamaya verince akış durumu, tamponu ve taahhüdü serbest kalır; `MAX_FLOWS` ilerler. FIN'den sonra kullanılmamış kredi geçersizdir (gönderen FIN ötesine yazamaz).
- **RESET (iptal):** gönderen `reset_flow` → motor akışın bütün birimlerini iptal eder (ARQ/kopya/parite adaylığından çıkar; uçuş kayıtları yalnız muhasebe; geç ACK teyit üretmez, çifte serbest bırakma yok) → CTRL'de güvenilir `FLOW_RESET(flow, final_seq)`. Alıcı tamponu atar, akışı kapalı işaretler (monoton kimlik → `closed_below` + ≤ MAX_FLOWS küme); kabul edilmiş ama gelmemiş veri `final_seq`'ten bilinir, beklenmez; geç birim ACK'lenip atılır (`late_after_reset`; kapasite kusuru DEĞİL).
- **Gelecekteki CLOSE/RESET yükümlülüğüne yer:** gönderen akış açarken CTRL kuyruğunda bir RESET yuvası ayırır (toplam ≤ Σ MAX_FLOWS); `Live` akışta alıcı PTT_OPEN + PTT_CLOSE için 2 birimlik krediyi açılışta ayırır. Ayrılamıyorsa `open_flow` → `Backpressure` ("kuyruk şimdi dolu değil" yetmez; sessiz düşürme yok).
- **Oturum değişimi:** akış taşınmaz; uygulama yeni oturumda yeni akış açar (TEXT eski `msg_id` ile yeniden gönderilir; dosya `FILE_ABORT(superseded)`; PTT yeniden OPEN — bugünkü kama-app kuralları).
- **Medya expiry:** `Live` akışta sıra boşluğu teslimi durdurmaz (sırasız teslim); süresi dolan birim gönderende motor boyunca iptal edilir; eski ses kaybı yeni sesi durdurmaz. Kredi muhasebesi DUR-06-1'e bağlı.
- **Açılıp kapanma (churn):** eşzamanlı akış ≤ MAX_FLOWS; kapalı akış kaydı ≤ watermark + MAX_FLOWS; toplam kayıt ve bellek sınırlı.

**7.7 05c'nin ACK'siz reddi = savunma.** Krediyi aşan birim protokol ihlalidir: ACK'siz ret + `credit_violations`. Kredili birime alıcıda yer yapısal olarak ayrılmıştır; buna rağmen yer bulunamazsa `capacity_defect` sayılır. **Kabul kapısı yalnız `capacity_defect = 0`'a bağlanır** (sahibin notu 2; ağ kaybı, protokol ihlali, medya expiry ayrı sayaçlar, Ek B B7).

## 8. FEC / ARQ / kopya ile ilişki

- **FEC bloğu akış başına:** blok anahtarı (`flow_id`, `flow_seq / K`); FEC çerçevesine ve RECOVERED'a `flow_id` eklenir (Ek A). Kurtarılan birim normal gelen birimle aynı kabul kapısından (akış, sıra) ile geçer → kabul durumu Duplicate'i bilir; ikinci teslim ve ikinci kredi tüketimi yok.
- **İlk uygulamada canlı medya FEC dışıdır [öneri, DUR-06-11];** `Ordered` akışlarda kısmi blok kapanışı sınırlı süreyle (T_block [CAL]); parite ve küçük paket maliyeti ölçülür. Farklı akışların birimleri aynı blokta bulunmaz → FEC akışları yeniden bağlamaz.
- **ARQ ve kopya** akış etiketini taşır; seçimi §5 düzey 1 + 2 yapar. ARQ_RETX_CAP aşımında kopya sahibine devir (05b AMENDMENT-12) aynen; yalnız `Live` birimde süre dolunca devir yerine iptal.

## 9. 05c kapısının akış başına dönüşümü (EK-1 §1.2)

- `AcceptState` (oturum) → (oturum, akış); pencere = kredi (`max_seq`), `max_inflight_units` değil. `RefuseWindow` = kredi ihlali. `RefuseFull` kredili birim için yapısal olarak erişilemez (teslim kanalı ≥ Σ taahhüt); erişilirse `capacity_defect`.
- Sıra başı rezervi (`DeliveryLedger`, R = min(max_peers, C − 1), `0c22a07d:runtime.rs:3284-3297`) S1 kipinde kredi taahhüdüyle yer değiştirir (kredi = önceden ayrılmış yer); örtük kip (bench) aynen kalır.
- `Reorder` (oturum) → (oturum, akış); tek `inbox` → akış başına hazır kuyruklar; `recv()` uygulamanın tutmadığı akışlardan sınıf sırasıyla, sınıf içinde dönüşümlü seçer.
- Gönderen mesafe kapısı → akış başına kredi kapısı. Kabul bütünlüğü (yer yoksa ACK yok) ve sızıntı düzeltmeleri aynen; yeni akışlar eski dosyanın eksik biriminin kimliğine bağlanmaz.

## 10. kama-app ataması, PTT süre politikası, sesli mesaj (S5)

- **Atama:** §4 tablosu. TEXT oturum boyu tek BE akışı; dosya aktarım başına BK akışı (`FILE_BEGIN … FILE_END`, FIN); uygulama kontrolü CTRL akışı; PTT konuşma başına VO `Live` akışı: ilk birim `PTT_OPEN` (`Reliable`), demetler `DeadlineBounded`, son birim `PTT_CLOSE` (`Reliable` + FIN). AMP kayıt baytları değişmez; AMP PTT-R0 ("üçü de Reliable", `0c22a07d:ptt.rs:20-24`) ve PTT-R3 (OPEN'sız demet DROP) değişir → AMP yaması (Ek A).
- **Alıcıda PTT:** demetler `flow_seq` ile gelir; OPEN'dan önce gelen demet sınırlı ön tamponda bekler (N [CAL], süre ≤ oynatma bütçesi); CLOSE, `flow_seq`'i kendisinden küçük demetler geldiğinde ya da süreleri dolduğunda akışı bitirir. Oynatma noktası media/kama-app'tedir: geç demet oynatılmaz ve sayılır.
- **Süre politikası:** demet süresi = sunum + D_VO (profil, [CAL]); gönderen motor süresi dolanı DATA/ARQ/kopya/parite kuyruklarından iptal eder; güvenilir dosya/TEXT için "yükümlülüğü koru", canlı medya için "süre sonrası kurtarmayı bırak" ayrı ilkelerdir.
- **Desteklenmeyen profil:** VO akışı ancak ölçülen teslim hızı kabul edilmiş VO profilini ρ_V,max içinde taşıyorsa açılır; değilse tipli hata → uygulama "desteklenmeyen profil" gösterir. ≈ 300 B/s yalnız deney başlangıcıdır (S5); desteklenen profil ölçümle belirlenir.
- **Sesli mesaj:** canlı PTT'nin otomatik/görünmez yerine geçişi DEĞİLDİR; kullanıcıya ayrı davranış olarak sunulur (BK akışında eksiksiz kayıt). Maliyeti ayrı planlanır (DUR-06-12).
- **05d/05f ile:** RECEIPT ve FILE_RECEIPT CTRL akışındadır → dosya FIFO'sunun arkasında beklemez; 05d M2 öncelik kapısı ve A1 erişimcisi akış bütçesine dönüşür (DUR-06-14). 05f rev2: dosya ilerlemesi = dosya **akışının** teyit tabanı + yeni benzersiz teyit.

## 11. Fiziksel sınırlar ve desteklenmeyen profil

- Eksik parçayı bekleme kalkar; ortak hattın meşguliyeti kalkmaz. İletime başlanmış paket kesilemez: 1050 B paket 31 B/s'de ≈ 33,9 s, 300 B/s'de 3,5 s [hesap]. BK etkinken VO gecikmesi ≥ S_BK / r → VO açıkken BK/BE birim boyu tavanı ayrı karardır (DUR-06-10); aktarım ortasında parça boyu değişemez (alıcı L'yi ilk parçadan öğrenir, `0c22a07d:file_transfer.rs:55-63`).
- 1,2 kbit/s ses yalnız payload olarak ≈ 150 B/s → 31 B/s'de canlı ses sürdürülemez; 31 B/s'de dosya kapsamdadır (S2, yazılı ağ zarfıyla).
- Her kontrol datagramı ≥ mühür ek yükü 22/26 B + gövde [`0c22a07d:envelope.rs:80-83`]: 31 B/s'de bir kredi çerçevesi ≈ (26 + 11) / 31 ≈ 1,2 s hat süresi [hesap] → kredi trafiği ayrı ölçülür.
- Soket/çekirdek/darboğaz kuyrukları bu tasarımın denetiminde değildir; pacer ≈ BtlBw varsayımı ölçüm ister.

## 12. 05c/05d/05f/05g ile çakışma ve geçiş sırası

1. 05b #140 → 05g → 05c merge (C15 v3 §7 Aşama 2–4); K4G-06 bunları değiştirmez, üstüne kurulur.
2. **Deney dalı** `k4g/06-deney` (merge edilmez; FROZEN dosyalar yalnız orada "DENEY" etiketiyle): §15'teki en küçük kapsam.
3. Tasarım rev + Ek A kararları (CEVAP 16) → ürün kodu sırası (API-first): (i) S1 + `KM-SEAM-AT-001` + drift (typed-error stub'lar) · (ii) client-core çerçeve + akış/kredi + alıcı · (iii) motor S7(b) + engine §13 / AMENDMENT-13 · (iv) kama-app ataması + AMP yaması + 05d A1 / 05f rev2 akış başına · (v) UniFFI aynası + kama-node + INT-02 düzeneği.
- **Çakışmalar:** 05c `runtime.rs`/`transport.rs` kabul ve mesafe kodu (§9 dönüşümü); 05g `transport.rs` etkinleştirme kolları (akış yalnız `Ready`'den sonra açılır; `Closed` sonrası `FlowRef` geçersiz); 05d M1–M4 (kuyruk taşınır — korunur), M2/A1 (dönüşür); 05f rev1 reddedildi, rev2 akış başına yazılır; 05b B′ seçicinin düzey-2 kuralı olur.

## 13. DUR özeti (ayrıntı Ek D; karar CEVAP 16)

| DUR | Soru | Önerim |
|---|---|---|
| DUR-06-1 | Medya kredisi aynı mı ayrı mı | (b) ayrı sınırlı kabul: VO havuzu + gönderen VO kovası; alıcı düşürebilir, ayrı sayaç |
| DUR-06-2 | S7 seçim sözleşmesi API'si | (b) `OpaqueUnit` + `ServiceTag`, motor içi saf seçici |
| DUR-06-3 | Kontrol envanteri ve CTRL katmanı | §4 envanteri; katman 0 = kova (tavan + koşullu hak) + birim boyu tavanı |
| DUR-06-4 | PTT kabul eşiklerinin sabitlenme yöntemi | kalibrasyon koşusu → sahip eşik dosyasını hash'le sabitler → YENİ koşu doğrular |
| DUR-06-5 | Tel çerçevesi | W2: (`flow_id`, `flow_seq`) birim kimliğinin yerine, 8 B |
| DUR-06-6 | `Live` akış alıcı teslim kipi | (a) sırasız + `flow_seq` |
| DUR-06-7 | S1 yüzeyinin biçimi | `open_flow`/`send(flow)`/`close_flow`/`reset_flow`/`hold_flow` + olaylar; `UnknownFlow` |
| DUR-06-8 | Kredi taşıyıcısı | ayrı FLOW_CREDIT çerçevesi, ACK yolunda, tetikli |
| DUR-06-9 | `flow_id` genişliği | u16 + tükenmede `RenewRequired` |
| DUR-06-10 | VO açıkken BK/BE birim boyu | PoC'de beyan + ölçüm |
| DUR-06-11 | FEC kapsamı | akış başına blok; canlı medya ilk uygulamada FEC dışı |
| DUR-06-12 | Sesli mesaj | ayrı görünür davranış; PoC'de yalnız tasarım, kod sonra |
| DUR-06-13 | Sürüm kimliği | `transport-v2` / `kama-v2-full` |
| DUR-06-14 | 05d A1 ve M2'nin geleceği | A1 ayrıca kodlanmaz; akış bütçesi erişimcisine dönüşür |
| DUR-06-15 | Sinyalleşmenin sınıfı | BE, ayrı akış |
| DUR-06-16 | [CAL] parametreleri | deney dalında başlangıç değerleri; son değer ölçümle |

## 14. Bu notun sınırı

Bu not ADIM 1'dir: sözleşme/arayüz taslağı ve ayırt edici test planı. Hiçbir mekanizma uygulanmadı ya da ölçülmedi; §5'teki sınırlar türetim taslağıdır. Ek A tam yama metni değildir, onay gerektiren kalemlerin listesidir; yama metinleri tasarım onayından sonra yazılır.

## 15. Deney dalı (ADIM 2) — önerilen en küçük kapsam

EK-1 §3 (2) ile: iki akış (TEXT/BE + dosya/BK), akış başına sıra, tek kredi türü (birim sayısı), §1.2 karşı örnek testi, §9.5 ilk kabul testinin en küçük hâli.
- **K1 (önce, kod değişmeden):** B2 karşı örneği mevcut motora karşı yazılır; beklenti kırmızı (ön-kayıtlı). ≈ 0,5 gün.
- **K2 client-core:** `FLOW_DATA` (W2; FEC bu dalda kapalı), (oturum, akış) kabul durumu ve sıra, akış başına hazır kuyruk + test-util `hold_flow`, birim kredisi (`FLOW_CREDIT` ACK yolunda, mutlak), gönderen akış kredi kapısı. S1'e dokunulmaz; erişim test-util kancasıyla. ≈ 2–3 gün.
- **K3 motor ("DENEY"):** `ServiceTag { tier }`, iki katman (BE/BK) DATA kuyruğu, ARQ ve kopya başının akış etiketiyle seçimi (DRR 3:1). ≈ 1–1,5 gün.
- **K4 testler:** B1-en-küçük (dosya akışı tutulur + bir dosya birimi kaybolur → TEXT teslim, bellek ≤ kredi, açılınca dosya tamamlanır), B2 yeşil (ARQ ve kopya kolları; FEC kolu "ölçülmedi"), B3-en-küçük (kredi kaybı → BLOCKED → yeniden ilan). ≈ 1 gün.
- **Toplam ≈ 5–6 iş günü [tahmin, takvim sözü değil].** Vektör ve seam beyanı değişmez; sonuçlar "YEREL"/SHA + komut + çıktı kuralıyla bu nota girer.
