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

## Ek A — Onay gerektiren değişikliklerin TAM listesi (liste; yama metni DEĞİL)

**Kural:** hiçbir donma sessizce aşılmaz. Her satır, kod yazılmadan önce sahibin yazılı kararını (CEVAP 16 ya da sonraki onay turu) ister; yama metinleri (üstü çiz + tarihli YAMA KAYDI + `word_coverage`) tasarım onayından SONRA, kodla aynı PR'da yazılır. Deney dalı (`k4g/06-deney`) bu listedeki dosyalara yalnız "DENEY" etiketiyle dokunur ve merge edilmez (EK-1 §3). DUR-05g-1'in dar S1 K7 istisnası (C15 v3 §3) bu kalemlerin hiçbirinin yerine geçmez. Sütunlar: FROZEN = donmuş sözleşme/spec/ürün profili; "Tel" = telde bayt değişimi (zarf içi); "Drift" = `scripts/ci/seam-api-drift.py` / `seam-coverage.json` beyanı.

| # | Dosya / öğe | Değişiklik (tasarım düzeyi) | FROZEN | Onay | Tel / vektör | Drift beyanı |
|---|---|---|---|---|---|---|
| A.1 | `crates/kama-contracts/src/s1.rs` — tipler | Yeni: `FlowClass {Control, Voice, BestEffort, Background}`, `FlowMode {Ordered, Live}`, `FlowSpec`, opak `FlowRef` (oturum + akış; `PeerRef`/`SessionHandle` gibi yorumlanmaz). `Delivered`'a `flow: FlowRef`, `class`, `flow_seq: u32`. `SessionEvent`'e `FlowOpened(FlowRef, FlowClass)` ve `FlowEnded(FlowRef, FlowEnd {Finished, Reset})`. `TransportCapabilities`'e `max_control_unit_bytes` | EVET (S1 sözleşmesi; YOL v5 §4) | sahip — dar S1 revizyonu (DUR-06-7) | yok (API) | §1 trait bloğu + kapalı kümeler aynı PR'da; `closed_sets_are_exhaustive` güncellenir |
| A.2 | `s1.rs` — `KamaTransport` metotları | Yeni: `open_flow(session, FlowSpec) -> Result<FlowRef, _>`, `close_flow(FlowRef)` (FIN), `reset_flow(FlowRef)` (iptal), `hold_flow(FlowRef, bool)` (alıcı tüketici durması). `send`'in hedefi `FlowRef` olur (DUR-06-7: değiştir ya da `send_on` ekle). `recv()` imzası aynı; anlamı "tutulmamış akışlardan" | EVET | sahip (DUR-06-7) | yok | drift kapısı trait metot kümesini eşler; 12 `impl KamaTransport` yeri (Ek C, G) |
| A.3 | `s1.rs` — `TransportError` | Yeni varyant `UnknownFlow` (kapanmış/bilinmeyen akış). Akış kredisi/akış sayısı/VO profili dolu → mevcut `Backpressure`; VO profili desteklenmiyor → `Backpressure` mı ayrı varyant mı DUR-06-7 | EVET (kapalı küme) | sahip | yok | §3 hata kümesi; `transport_error_is_a_std_error_with_stable_messages` |
| A.4 | `s1.rs` — `DeliveryPolicy` | Değişmez (Reliable / DeadlineBounded / BestEffort). Anlamı değişir: `DeadlineBounded` süresi dolunca motor boyunca iptal (bugün yalnız sunumda denetlenir, `0c22a07d:transport.rs:1297-1305`) | EVET (§4 anlam) | sahip | yok | §4 metni |
| A.5 | `crates/kama-contracts/src/handles.rs` | **Değişiklik önerilmez**: `FlowRef` `s1.rs`'tedir; `SessionHandle`, `PeerRef`, `CloseReason` aynen | EVET | "değişmedi" beyanı yeter | yok | beyan |
| A.6 | `crates/kama-contracts/src/s3.rs`, `kama-noise-wire` (`envelope.rs`, `protector.rs`), hub-server codec | **Değişmez**: 22 B SESSION_DATA, PN/nonce kuralları, `ProtectionContext` | EVET | beyan | yok | beyan |
| A.7 | `docs/seams/KM-SEAM-AT-001.md` | §1 trait bloğu (A.1–A.3) · §3 hata kümesi · §4 "aynı session içinde Reliable birimler gönderim sırasında" → "aynı **akış** içinde"; `Live` akışta sırasız teslim + `flow_seq`; INV-ACK-DELIVER-1 akış başına; kredi ve "kapasite durması ACK kesilerek gösterilmez" maddesi · §6 yeni satırlar H-FLOW-ISOLATION, H-FLOW-CREDIT, H-LIVE-EXPIRY, H-CTRL-SERVICE · YAMA KAYDI | EVET (G-S1) | sahip | yok | `seam-coverage.json`'a 4 satır; `seam-api-drift.py` S1 bloğu |
| A.8 | `crates/client-core/src/wire.rs` | Yeni iç çerçeveler: `FLOW_DATA` (W2: `[type][flags][flow_id u16][flow_seq u32]`, 8 B), FEC çerçevesine `flow_id u16` (+2 B), RECOVERED'a `flow_id u16`, yeni kontrol çerçeveleri `FLOW_CREDIT` (çoklu akış), `FLOW_BLOCKED`, `FLOW_RESET`. 0x00/0x05 S1-dışı örtük kip (bench) için aynen | EVET (Transport v1.1 tel/codec, `0c22a07d:runtime.rs:237-238` "tel, codec değişmez" beyanı) | sahip (DUR-06-5, -8) | **EVET**, zarf içi; hub zarfı değişmez. Yeni vektör paketi yalnız Python referansından (A.17) | `api_surface_v0` wire-v0 pini (A.19) |
| A.9 | Codepoint kaydı: `kama-transport-schema/src/inner_kind.rs` + kama-noise-wire-tdd §2.56 | Yeni kontrol codepoint'leri (FLOW_CREDIT / FLOW_BLOCKED / FLOW_RESET) ÖNCE spec kaydına; v0 kabuğundaki çerçeve tipleri (0x08…) ile S2 eşlemesi aynı PR'da | EVET (noise-wire spec) | sahip ("codepoint önce spec") | EVET (yeni codepoint) | beyan |
| A.10 | `crates/client-core/src/runtime.rs` — gönderen | Store girdisine (akış, sıra); akış başına kredi kapısı (oturum geneli mesafe kapısının yerine, `0c22a07d:runtime.rs:2511`, :2575-2581); offer'ın akış bütçesi dışında birim taşımaması; ServiceTag'li `OpaqueUnit`; FEC bloğu akış başına (`:2010-2014`); expiry/iptal kablosu; BLOCKED üretimi | EVET (ürün profili davranışı) | sahip + EVOLUTION §22 altı beyan | EVET (A.8) | — |
| A.11 | `runtime.rs` — alıcı (`DeliveryGate`, `AcceptState`, `DeliveryLedger`) | (oturum, akış) kabul durumu; pencere = kredi; `RefuseWindow` = kredi ihlali (`credit_violations`); `RefuseFull` → `capacity_defect`; S1 kipinde sıra başı rezervi kredi taahhüdüyle yer değiştirir (`0c22a07d:runtime.rs:3284-3297`); FLOW_CREDIT ACK yolunda; havuzlar | EVET (05c A.6 kapsamının devamı; INV-ACK-DELIVER-1 akış başına) | sahip | EVET | KM-SEAM-AT-001 §4 (A.7) |
| A.12 | `runtime.rs` — `RuntimeConfig` / `KamaV1Profile` | Yeni alanlar: sınıf havuzları, `W0`, `MAX_FLOWS0`, ρ_C, ρ_V,max, Q_BE:Q_BK, T_block, D_VO [CAL]; `TRANSPORT_V1_PROFILE_ID` / `_ARCHITECTURE` (`:236`, `:241`) yeni kimlik (DUR-06-13) | EVET (`transport_v1_freeze.rs`, `product_profile_v1.rs`) | sahip + §22 beyanı | yok | `api_surface_v0` `RuntimeConfig` deseni (A.19) |
| A.13 | `crates/client-core/src/transport.rs` | `Reorder` (oturum, akış) başına; tek `inbox` → akış başına hazır kuyruk + `hold_flow`; `open/close/reset_flow`; `FlowOpened/FlowEnded`; `Delivered` yeni alanlar; 05g etkinleştirme kollarıyla uyum (akış yalnız `Ready` sonrası) | Hayır (adaptör); davranışı S1'e bağlı | tasarım onayı | yok | H-* testleri real hedefte |
| A.14 | `crates/adaptive-transport/src/sender.rs` (S7 (b)) | `OpaqueUnit` + `ServiceTag {tier, flow, expires_ns}`; katman/akış başına DATA kuyruğu; tek saf seçici (düzey 1 katmanlı DRR + CTRL/VO kovaları; düzey 2 = DUR-38 kuralı); `serve_copy_share`/DUR-31 rezervi seçicinin içine (`0c22a07d:sender.rs:7136-7162`); expiry/iptal olayı + `take_expired_units`; geç ACK iptal edilmiş birimi teyit etmez | **EVET (FROZEN v1.1 motor)** | sahip — yazılı v1.x/v2 revizyonu (DUR-06-2, -13) | yok (motor teli yok); `tv_*` değişmez HEDEFİ — kayarsa DUR | — |
| A.15 | `crates/adaptive-transport/src/arq.rs` | **Değişmez hedefi**: akış içi plan sırası aynen (`0c22a07d:arq.rs:124-127`); akış seçimi `sender.rs` seçicisinde. `tv_arq` pini korunur | EVET | beyan | yok | — |
| A.16 | `specs/adaptive-transport-engine-spec.md` (§3 ana döngü, §9, §11, §13 sınıflar arası sıra, §16.2) + `specs/transport-layer-tdd.md` §12 yeni AMENDMENT-13 + `docs/transport-v1/{INVARIANTS, PRODUCT-PROFILE, ARCHITECTURE, EVOLUTION §22}.md` | İki düzeyli seçim, ServiceTag, katman 0 kontrol hakkı, expiry; 05b'nin §13 v1.1 TEK İSTİSNA metninin seçiciye taşınması (`0c22a07d:specs/adaptive-transport-engine-spec.md:1016-1018`); kimlik | EVET (KAT GO-FREEZE) | sahip — yama yetkisi (CEVAP 5 benzeri, bölüm listesiyle) | yok | — |
| A.17 | `vectors/` + yeni Python referansı (`ref/transport/` önerisi) + `crates/kama-vectors` + `fuzz/fuzz_targets/wire_v0.rs` | Yeni çerçevelerin pozitif/ret vektörleri YALNIZ Python referansından; `SCHEMA.json` değişmez; spec-coverage ratchet'ine giriş; fuzz hedefine yeni decode'lar | vektör kuralı | sahip (vektör kapsamı) | EVET (yeni paket); mevcut `amp-*`, `ccr-*` değişmez | — |
| A.18 | `specs/application-message-protocol.md` §3 | PTT-R0 ("üçü de Reliable") → OPEN/CLOSE Reliable, MEDIA_BUNDLE `DeadlineBounded`; PTT-R3 (OPEN'sız demet DROP) → sınırlı ön tampon; "FILE_BEGIN her zaman önce gelir" gerekçesi aynı akışa bağlanır (`0c22a07d:file_transfer.rs:74-75`); RECEIPT/FILE_RECEIPT/FILE_ABORT kontrol akışında; kayıt baytları DEĞİŞMEZ | EVET (AMP donmuş metin; YAMA KAYDI ile) | sahip | AMP baytları ve `amp-record*-v1.json` değişmez | — |
| A.19 | `crates/client-core/tests/api_surface_v0.rs` + `docs/design/P4-TRANSPORT-API-FREEZE.md` | Yüzey pini: wire-v0 imzaları, `Delivery`, `RuntimeConfig` deseni, `OpaqueUnit` re-export | EVET (P4 API freeze) | sahip (DUR-8 biçimi) | — | — |
| A.20 | kama-app: `messaging.rs`, `file_transfer.rs`, `ptt.rs`, `signaling.rs` | Akış ataması (§10); dosya tüketicisi `hold_flow`; RECEIPT kontrol akışına; PTT ön tampon + `flow_seq`; 05d M2 kapısı (`gated_by_receipts`) ve A1'in akış bütçesine dönüşmesi | Hayır (ürün kodu); 05d kararlarını değiştirir | tasarım onayı + DUR-06-14 | yok | — |
| A.21 | 05d DUR-05d-7 A1 / DUR-05d-8 (W_F₀ = 240, H_R/H_C) kararları (C14 §4, C15 v3 §5) | A1 ayrı kodlanmaz; W_F → BK penceresi; H_C → açılışta ayrılan kontrol yuvası (§7.6) | karar metni | sahip (DUR-06-14) | yok | — |
| A.22 | 05f rev2 (`e5514e7a` reddedildi) | Dosya ilerlemesi = dosya akışının teyit tabanı; "yerel vazgeçme politikası" zarfı | — | sahip (C15 v3 §6) | yok | — |
| A.23 | `crates/kama-mobile/src/error.rs` (UniFFI aynası `TransportError` `:218-250`, `PttError` `:374-440`), `event.rs` | `UnknownFlow` aynası; PTT "desteklenmeyen profil" hatası; yeni sayaçlar `NodeMetrics`'te | EVET (UniFFI yüzeyi) | sahip — ayrı karar (C15 v3 §5 (iii) deseni) | yok | — |
| A.24 | `crates/kama-node` (`event.rs`, `probe.rs`, `node.rs`) + `crates/kama-integration/examples/int02_node.rs` | Yeni sayaçlar (`capacity_defect`, `credit_violations`, `media_expired_tx`, `media_dropped_rx`, `late_after_reset`, kredi/BLOCKED baytları); INT-02 kolları (Ek B) | Hayır | tasarım onayı | yok | — |
| A.25 | INT-02/INT-03 kapı tanımı (C15 v3 §7 Aşama 7; C9 §5 hükümleri) | "kapasite reddi kaynaklı boşa bayt ≈ 0" → "`capacity_defect = 0` (yalnız geçerli krediye uyan güvenilir veri)"; ağ kaybı / protokol ihlali / medya expiry ayrı raporlanır; PTT eşikleri DUR-06-4 yöntemiyle | kapı metni | sahip | — | — |
| A.26 | `scripts/ci/layer_check.py` | Yeni kural önerisi: `adaptive-transport` kama-app'e ve kama-contracts'a bağımlı olmaz (ServiceTag motorun kendi tipidir) — mevcut kuralların kapsamı doğrulanır | Hayır | tasarım onayı | — | — |

**Değişmez beyanı (aynı listenin parçası):** `handles.rs`, `s3.rs`, kama-noise-wire zarfı ve protector, hub-server codec, ACK_V2 codec'i ve `tv_ack`, `arq.rs` planı ve `tv_arq`, AMP kayıt baytları ve `amp-record*-v1.json`, `ccr-*` vektörleri, `vectors/SCHEMA.json`. Bunlardan biri uygulama sırasında değişmek zorunda kalırsa iş durur ve ayrı DUR açılır.

## Ek B — Kabul testleri (ayırt edici test planı)

**Ortak kurallar.** Fikstür ön koşulları AÇIK assertion'dır: kurulmadıysa test KIRMIZI ("ölçülecek iş oluşmadı" yeşil sayılmaz; `None` = kırmızı). Dışarıdan ACK, kayıp ilanı ya da kredi ENJEKTE edilmez; yalnız üretim saatleri ve zamanlayıcıları ilerler (ağ kaybı ağ düzeneğinde, ACK yolu kaybı yalnız belirtilen testlerde). Sınırlar koddan ÖNCE türetilir ve commit'lenir; testte ölçülen değer türetilen üst sınırın altında kalmalıdır; sonuç "ölçüldü" diye yazılır. Aşama: **D** = deney dalı (`k4g/06-deney`, YEREL/SHA kuralıyla), **U** = Aşama 6 ürün uygulaması, **I** = Aşama 7 INT-02 (iki süreç + gerçek UDP, 3 tohum). Ön-kayıtlı beklentiler her testte "taban" satırındadır; tutmayan beklentide önce türetim, sonra kod sorgulanır.

**Sayaç sözlüğü (B7 bunları ayırt eder):** `net_loss_recovery_bytes` (ağ kaybından doğan ARQ/kopya/parite) · `credit_violations` (krediyi aşan birim, ACK'siz ret — protokol ihlali) · `media_expired_tx` (gönderende süresi dolup iptal edilen VO birimi) · `media_dropped_rx` (alıcıda VO havuzu/oynatma nedeniyle düşen) · `media_late_unplayed` (geç geldiği için oynatılmayan) · `capacity_defect` (geçerli krediye uyan güvenilir birim için yer bulunamaması — **kabul kapısı yalnız bu sayaca bağlıdır**, sahibin notu 2) · `late_after_reset` · `ctrl_credit_bytes`, `ctrl_blocked_bytes` (kredi trafiğinin bant maliyeti).

### B1 — §9.5 ilk kabul testi: durmuş dosya tüketicisi + kayıp dosya birimi
- **Amaç:** dosya tüketicisi durmuşken VE dosyada bir birim kayıpken TEXT teslim edilir, medya süre politikasına uyar, bellek sınırlıdır; tüketici açılınca dosya tamamlanır.
- **Ön koşullar (assert):** alıcı dosya akışını `hold_flow` ile tuttu; dosya birimi k en az bir kez ağda düştü (düzenek sayacı ≥ 1) ve k'den büyük en az bir dosya birimi alıcıda kabul edildi; dosya akışının kredisi tükendi (gönderende `flow_blocked` ≥ 1); TEXT ve (U/I'de) VO birimleri bu durum kurulduktan SONRA sunuldu.
- **Ölçülen:** her TEXT'in sunumdan uygulamaya teslim süresi; dosya akışı tampon doluluğu (birim, bayt); alıcının bütün tamponlarındaki toplam birim/bayt; VO sayaçları; B7 sayaçları.
- **Geçer:** bütün TEXT'ler dosya tutulu iken teslim edilir; dosya tamponu ≤ dosya kredisi, toplam ≤ havuz; `capacity_defect = 0`, `credit_violations = 0`; VO yalnız süre politikasına göre düşer (oynatılabilir oran B11'de); tüketici açılınca dosya bütün birimleriyle ve sırayla tamamlanır, özet tutar. **Taban (05c, ön-kayıtlı):** TEXT k kurtarılana dek teslim edilmez → KIRMIZI.
- **Aşama:** D (TEXT + dosya, VO yok, FEC kapalı) · U (VO dahil) · I.

### B2 — EK-1 §1.2 motor karşı örnek testi (ZORUNLU)
- **Amaç:** motora daha önce kabul edilmiş eski dosya biriminin kurtarma işi yeni TEXT/medyayı bekletiyor mu.
- **Ön koşullar (assert):** offer kanalı boş, `tx_queue`'da dosya DATA'sı yok; eski dosya birimi u teyitsiz ve kurtarma işi canlı — üç kol: (a) RECOVER_ARQ'da `pending_lost`'ta u (ARQ planında baş), (b) `dup_queue`'da vadeli u kopyası (baş), (c) u'nun bloğunun bekleyen paritesi `pending_fec`'te; dönüş yolu kesik (u teyit edilemez). Sonra TEXT birimi t ve (U'da) VO birimi v sunulur.
- **Ölçülen:** t'nin (ve v'nin) ilk emisyonuna kadar u için yapılan emisyon sayısı ve bayt; t'nin ilk emisyon gecikmesi.
- **Geçer:** t, türetilmiş düzey-1 sınırı T_BE içinde ilk emisyonunu alır (§5 madde 3); u'nun kurtarma emisyonları t'yi bu sınırın ötesine itemez; aynı ölçüt v için T_VO. **Taban (ön-kayıtlı):** mevcut motorda (a) ve (b) kolunda t, u'nun bütün uygun kurtarma emisyonlarından sonra gelir → KIRMIZI; kırmızı çıkmazsa §6 çıkarımı yanlıştır, S7 kararı yeniden açılır. B2 S7(a)'nın tek başına kabul edilemeyeceğinin sınamasıdır: (a) ancak bu testi gerçek kurtarma yollarıyla geçerse kabul edilir.
- **Aşama:** D (motor düzeyi, deterministik saat; kol (a), (b); kol (c) FEC kapalı olduğundan "ölçülmedi") · U (üç kol, uçtan uca).

### B3 — Kredi güncellemesi kaybı
- **Amaç:** kredi çerçevelerinin kaybı veri kaybı ya da kalıcı durma üretmez.
- **Ön koşullar (assert):** dosya akışı kredi sınırında bloklu; alıcı tüketiciyi açtı (yeni kredi üretildi); sonraki N_k FLOW_CREDIT çerçevesi düzenekte düşürüldü (sayaç = N_k).
- **Ölçülen:** gönderende `FLOW_BLOCKED` sayısı ve aralıkları; blokta kalma süresi; `ctrl_credit_bytes`, `ctrl_blocked_bytes`.
- **Geçer:** akış, türetilmiş "BLOCKED geri çekilme + RTT + ilan" sınırı içinde ilerler; veri kaybı yok; `capacity_defect = 0`; kontrol baytı raporlanır.
- **Aşama:** D (en küçük hâli) · U · I.

### B4 — Sırasız / bayat / tekrarlanan / yanlış oturumdan kredi
- **Amaç:** mutlak + artan kredi modelinin sağlamlığı.
- **Ön koşullar (assert):** daha büyük sınırlı çerçeve işlendikten sonra daha küçük sınırlı çerçeve teslim edildi; aynı çerçeve iki kez; eski oturum bağlamlı kredi çerçevesi (yenileme sonrası) gönderildi.
- **Ölçülen:** gönderenin etkin sınırı; gönderilen en büyük `flow_seq`.
- **Geçer:** etkin sınır hiç azalmaz; gönderen hiçbir anda etkin sınırı aşmaz (`credit_violations = 0` alıcıda); eski oturum kredisi yeni oturum akışına uygulanmaz (bağlam-önce dağıtım sayacı artar).
- **Aşama:** D (birim düzeyi) · U.

### B5 — İki yönlü doygunluk
- **Amaç:** iki uç aynı anda dosya + TEXT + (U/I) PTT gönderirken kilitlenme ve aç bırakma yok.
- **Ön koşullar (assert):** iki yönde de dosya akışı kredi sınırında bloklu kalmış en az bir bölüm; iki yönde de bekleyen RECEIPT/FILE_RECEIPT; oturum değişmedi.
- **Ölçülen:** her yönde tamamlanma; RECEIPT/FILE_RECEIPT gecikmesi; kredi çerçevesi gecikmesi.
- **Geçer:** bütün TEXT/RECEIPT/dosya tamamlanır; canlı RECEIPT silinmez (05d); `capacity_defect = 0`; kontrol yolu her iki yönde ilerledi (kredi ilanı sayacı her iki yönde > 0).
- **Aşama:** U · I.

### B6 — Sürekli yeni yük: sonlu hedef küme biter
- **Amaç:** kapasiteyi aşan sürekli üretici altında önceden kabul edilmiş hedef kümenin adaleti.
- **Ön koşullar (assert):** hedef küme BAŞTAN sabit (ör. bir dosya + 20 TEXT); ayrı arka plan üreticisi BE'de kapasitenin üstünde yük SÜRDÜRÜR ve hiç durmaz; arka plan `Backpressure` aldı (≥ 1).
- **Ölçülen:** hedef kümenin her biriminin tamamlanma süresi; arka plan reddi sayısı.
- **Geçer:** hedef kümenin her birimi türetilmiş sınır içinde tamamlanır (§5); tamamlanma arka planın durmasına bağlı değildir; yeni iş backpressure alır, kabul edilmiş iş aç kalmaz.
- **Aşama:** U · I.

### B7 — "HER BİT DÜŞMAN": sebeplerin ayrık sayılması
- **Amaç:** dört sebebin (ağ kaybı, protokol ihlali, medya expiry, kapasite kusuru) ayrı sayaçlara düştüğünü ve kabul kapısının yalnız kapasite kusuruna bağlandığını göstermek.
- **Ön koşullar (assert):** dört alt koşu, her birinde yalnız bir sebep kurulur: (1) ağ kaybı (düzenek kaybı > 0, kredi uyumlu gönderen); (2) krediyi aşan sahte gönderen (test kancası); (3) VO süresi dolumu (D_VO küçük); (4) kapasite kusuru — yalnız test kancasıyla alıcı havuzunu yapay küçülterek.
- **Ölçülen:** sözlükteki bütün sayaçlar, her alt koşuda.
- **Geçer:** her alt koşuda yalnız kendi sayacı artar (diğerleri 0); (4)'te `capacity_defect > 0` ve kabul hükmü KIRMIZI üretir; (1)–(3)'te `capacity_defect = 0` ve hüküm o sebepten kırmızı olmaz. Ürün koşularında (B1, B5, B6) `capacity_defect = 0` ŞARTTIR; kredi baytları ayrıca raporlanır ("boşa bayt ≈ 0" tek başına kapı değildir).
- **Aşama:** D (1, 2, 4) · U (dördü) · I (ürün koşularında sayaç sütunları).

### B8 — PTT altında TEXT gecikmesi
- **Amaç:** sürekli canlı ses altında TEXT'in ilerlemesi.
- **Ön koşullar (assert):** VO akışı desteklenen profilde açık ve ölçüm penceresi boyunca demet üretiyor; TEXT ölçüm penceresinde sunuldu.
- **Ölçülen:** TEXT teslim gecikmesi dağılımı (p50/p95/en çok), VO sayaçları.
- **Geçer:** ölçülür/raporlanır; ayrıca her TEXT türetilmiş üst sınır içinde teslim edilir (ρ_V ile §5 madde 3).
- **Aşama:** U · I.

### B9 — Sürekli TEXT altında dosya biter (aç bırakılmama)
- **Amaç:** BE sürekli yükteyken BK dosyasının tamamlanması.
- **Ön koşullar (assert):** BE akışı ölçüm penceresi boyunca backlog'lu (kuyruk hiç boşalmadı — örneklemeyle assert); dosya ölçüm başında kabul edildi.
- **Ölçülen:** dosya tamamlanma süresi; BK'nin aldığı bayt payı.
- **Geçer:** dosya, Q_BK/(Q_BE+Q_BK) payından türetilen sınır içinde tamamlanır; BK payı türetilen alt sınırın altına inmez.
- **Aşama:** D (BE+BK) · U · I.

### B10 — EK-1 §1.1 kontrol testleri (iki yönlü)
- **(a) Sürekli PTT altında kontrol ilerler.** Ön koşul: VO ölçüm boyunca backlog'lu. Ölçülen: kredi ilanı, FLOW_RESET, PTT_OPEN/CLOSE ve RECEIPT gecikmesi. Geçer: her biri türetilmiş kontrol sınırı içinde (§5 madde 5).
- **(b) Sürekli kontrol yükü altında kabul edilmiş veri aç kalmaz.** Ön koşul: kontrol katmanı ölçüm boyunca uygun işte (ör. çok sayıda RECEIPT üreten TEXT seli; sentetik kontrol kancası); BK ve BE'de kabul edilmiş iş var. Ölçülen: BK/BE bayt payı. Geçer: kontrol tüketimi ≤ ρ_C·r·τ + b_C; BK/BE türetilen alt sınırın altına inmez.
- **(c) Döngüsel bağımlılık yok.** Ön koşul: bütün veri akışlarının kredisi sıfır (bloklu). Ölçülen: kredi ilanı ve BLOCKED'in yine de çıkıp çıkmadığı. Geçer: kontrol çerçeveleri veri kredisi olmadan gider ve işlenir; akışlar açılır.
- **(d) Ayrı kaynak.** Ön koşul: P_BK tam taahhüt edilmiş, dosya kredisi bitmiş. Geçer: RECEIPT/FILE_RECEIPT/FILE_ABORT ve TEXT kabul edilir (P_CTRL, P_BE ayrı).
- **Aşama:** D ((c), (d)) · U (dördü) · I ((a)).

### B11 — EK-1 §1.3 PTT ölçüleri ve eşik sabitleme
- **Amaç:** "bütün sesi düşürerek geçmek" yok: desteklenen profilde zamanında oynatılabilir oran, konuşma başlangıç gecikmesi, uçtan uca gecikme; dosya/TEXT yükü altında korunması; geç demetin oynatılmaması; sebep sayaçları.
- **Ön koşullar (assert):** profil (hız, RTT, kayıp, kesinti) yazılı ve ağ düzeneğiyle kuruldu; VO açıldı (desteklenen profil); B1/B9'daki dosya ve TEXT yükü aynı anda.
- **Ölçülen:** oynatılabilir oran = süresi içinde oynatma noktasına ulaşan demet / üretilen demet; konuşma başlangıcı = `open` çağrısından ilk demetin oynatılmasına; uçtan uca gecikme dağılımı; `media_expired_tx`, `media_dropped_rx`, `media_late_unplayed`, ağ kaybı ayrı.
- **Geçer (eşik sabitleme kuralı, sahibin notu 1):** (1) bir ya da daha çok **KALİBRASYON** koşusu yapılır ve "kalibrasyon" etiketiyle raporlanır — kabul hükmü üretmez; (2) sahip eşikleri ürün hedefi + kalibrasyondan seçer ve eşik dosyası (`PTT-ESIK-v1`, sha256) "INT-02 GEÇTİ" kabul koşusundan ÖNCE commit'lenir ve onaylanır; (3) kabul YENİ bir koşuyla (yeni koşu kimliği, yeni tohumlar) yapılır; kalibrasyon koşusu kabul sayılmaz, kabul koşusunun sonucuna bakılarak eşik değiştirilmez; (4) eşik değişirse yeni dosya + yeni kabul koşusu. Geç demet hiç oynatılmaz (`media_late_unplayed` sayılır, oynatma olayı yok).
- **Aşama:** U (sayaçlar ve ölçüm araçları) · I (kalibrasyon + kabul).

### B12 — Gönderici deadline'ı: süresi dolan medya hiçbir motor kuyruğunda yaşamaz
- **Amaç:** expiry bütün sahiplik zincirinde; geç ACK çifte serbest bırakma ya da yanlış teyit yaratmaz.
- **Ön koşullar (assert):** VO birimi v'nin süresi dolmadan önce v `tx_queue`, `pending_lost` ve `dup_queue`'da (üç kol ayrı) bulunuyor; dönüş yolu kesik.
- **Ölçülen:** süre dolumundan sonra v'nin motor kuyruklarında ve store'da varlığı; v için emisyon; süre dolumundan sonra gelen v ACK'inin etkisi.
- **Geçer:** süre dolumundan sonraki ilk turda v hiçbir DATA/ARQ/kopya/parite adayında değildir ve tele çıkmaz; store'dan bir kez düşer; geç ACK teyit üretmez, payload'u ikinci kez serbest bırakmaz, `media_expired_tx` = 1. Güvenilir birim için aynı test yükümlülüğün KORUNDUĞUNU gösterir (dosya birimi süre nedeniyle iptal edilmez).
- **Aşama:** D (motor düzeyi; DENEY) · U.

### B13 — EK-1 §1.4 kredi yaşam döngüsü
- **(a) Açılış:** yeni akışın ilk birimi kontrol turu beklemeden gider ve kabul edilir (ilk kredi profil sabiti); ilk uygulama kontrolü kredisiz gider. Ön koşul: hiçbir FLOW_CREDIT görülmedi.
- **(b) FIN ve RESET:** FIN'den sonra akış durumu, tampon ve taahhüt serbest kalır (sayaçla); RESET'te kabul edilmiş ama gelmemiş veri beklenmez, geç birim `late_after_reset` ile ACK'lenip atılır, `capacity_defect = 0`; kaynakların serbest kalma anı ölçülür.
- **(c) Oturum değişimi:** supersession/yenileme sonrası eski oturumun kredisi, DATA'sı ve ACK'i yeni oturumun akışına uygulanmaz (ön koşul: eski oturumdan geç çerçeve gerçekten geldi).
- **(d) Medya expiry muhasebesi:** eski konuşmanın kayıp/süresi dolmuş demetleri yeni konuşmanın teslimini durdurmaz; kredi/sıra muhasebesi DUR-06-1 kararına göre tutarlı.
- **(e) Açılıp kapanma:** K döngüde akış aç-kapa (dosya ve PTT); kayıt sayısı ve bellek her örnekte ≤ türetilmiş tavan; kapalı akış kaydı ≤ watermark + MAX_FLOWS.
- **(f) Gelecek yükümlülüğe yer:** RESET yuvası ve Live akışın OPEN/CLOSE kredisi ayrılamıyorken `open_flow` → `Backpressure`; açık akışların hepsi her durumda kapatılabilir (CLOSE/RESET hiç reddedilmez).
- **(g) Rezerv korunur:** P_BK tam taahhütte (kullanılmamış kredi dahil) TEXT ve kontrol kabul edilir.
- **Aşama:** D ((a), (b) FIN, (g)) · U (hepsi).

### B14 — EK-1 §1.5 T_birim (akışlar arası, birim düzeyi)
- **Amaç:** her teyitsiz birimin son hizmet yaşı türetilmiş T_birim ile sınırlı.
- **Türetim (koddan önce):** girdiler açık: canlı rakip birim sayısı, kuyrukta önce hizmet alacak işler, yeniden uygun hale gelme süresi, durum geçişleri, çağrılar arası azami aralık; "uygun iş" = vadesi gelmiş, teyitsiz, bu turda gönderilmemiş; token yokluğu iş yokluğu sayılmaz; PRE_CONVERGE istisnası ve hizmet yaşının başladığı an yazılı.
- **Ön koşullar (assert):** kabul edilmiş sonlu canlı küme: ≥ 2 akışta (BE, BK) ≥ 2'şer teyitsiz zincir; dönüş yolu kesik; dışarıdan olay enjekte edilmez; ≥ 2 × T_birim boyunca üretim saati ilerler.
- **Ölçülen:** her birimin son hizmet yaşı ve yeniden seçilmesi; durum geçişlerinde (ARQ → kopya) aynı birimin izlenmesi.
- **Geçer:** her birim için ölçülen ≤ T_birim; sonuç "ölçüldü" diye yazılır. 31 ve 100 B/s'de.
- **Aşama:** D (iki akış) · U.

### B15 — FEC akışları yeniden bağlamaz
- **Amaç:** kurtarılan birim aynı akış ve kredi muhasebesinden geçer; farklı akışlar aynı bloğu beklemez.
- **Ön koşullar (assert):** BE ve BK akışları aynı anda FEC açık; her akışta bir blokta kalıcı kaynak kaybı, parite yeterli.
- **Ölçülen:** kurtarılan birimin teslim sayısı, kredi tüketimi, blok kapanış süresi, parite ve küçük paket bayt maliyeti.
- **Geçer:** her blok tek akışın birimlerinden oluşur; kurtarılan birim tam bir kez teslim edilir ve ikinci kredi tüketmez; kısmi blok T_block içinde kapanır; maliyet raporlanır.
- **Aşama:** U.

### B16 — Kuyruk derinliği ölçüsü (S7 denetiminin testi)
- **Amaç:** §6 tablosundaki her katmanda yeni TEXT biriminin önünde dosya işi kalmadığını ölçmek.
- **Ön koşullar (assert):** dosya backlog'u her katmanda gerçekten var (offer bütçesi dolu, motor BK kuyruğu dolu, BK kurtarma işi canlı).
- **Ölçülen:** TEXT birimi sunulduğu anda, sırasıyla offer → store → motor seçimi → tel yolu üzerinde, ondan önce hizmet alacak BK baytı.
- **Geçer:** ≤ bir iletimdeki BK birimi (fiziksel sınır, §11) + türetilmiş DRR payı; FIFO kaynaklı ek BK baytı = 0.
- **Aşama:** D (offer + motor) · U (bütün katmanlar).

### B17 — DUR-38 regresyonu: tek akışta 05b davranışı
- **Amaç:** ortak seçicinin tek akışlı yapılandırmada DUR-38 kararlarını koruması.
- **Ön koşullar / ölçülen / geçer:** C15 v3 §2.4 T1 ve T2 aynen (31 ve 100 B/s; birim düzeyi; enjeksiyonsuz; ayrı arka plan üreticisi), tek akışla.
- **Aşama:** U (D'de seçici tam değilse "ölçülmedi").

### B18 — Korunan doğruluk düzeltmeleri
- **Amaç:** 05b/05c/05d/05g'nin akış başına korunması.
- **Kapsam:** INV-ACK-DELIVER-1 akış başına (tutulamayan birim ACK'lenmez — sahte ihlalcıyla); 05b teyit/payload sahipliği (budanmış kaydın geç ACK'i teyit etmez) akış etiketiyle; 05d canlı RECEIPT silinmez ve oturum değişiminde taşınır; 05g: `Ready` olmayan oturumda `open_flow` → hata, `Closed` sonrası `FlowRef` → `UnknownFlow`; INT-02 `ek-tuketici-durak` kolu 3/3 (bütün düğüm durur; tasarım bunu değiştirmez).
- **Geçer:** ilgili mevcut testler (Ek C) yeni iddialarıyla yeşil; INT-02 kolu 3/3.
- **Aşama:** U · I.

### B19 — Akış kimliği tükenmesi ve VO kabulü
- **(a)** `flow_id` uzayı test kancasıyla sona yaklaştırılır → yeni `open_flow` tipli hata + `RenewRequired`; yeni oturumda akış açılır; eski akışlar sessiz düşmez.
- **(b)** Ölçülen hız VO profilini ρ_V,max içinde taşımıyorken `open_flow(Voice)` reddedilir ("desteklenmeyen profil"); taşıyorken açılır. Eşik sayısı [CAL].
- **Aşama:** U.
