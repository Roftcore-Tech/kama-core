# kama-target-net-v2 — ÖN-KAYIT (koşudan ÖNCE donar)

Tarih: 2026-11-16 · Dal: `campaign/kama-target-net-v2` · Taban: `origin/main` `3abee42990494cff05abfb553e5a347cfe45d12f`
(#211 dahil; ilk donmada taban `b88a047f` idi, §13 taşıması)
· Ölçülen kodun SHA'sı ve donmuş kimlikler: §12 (ve `reports/kama-target-net-v2/FREEZE.json`).

Bu belge, ölçüm koşulmadan önce commit'lenir. Hücre, kol, N, horizon, tohum, metrik ve hüküm kuralı sonuç
görüldükten sonra değişmez. Sonuç raporunda yalnız "ölçüm bitti" denir; hiçbir hücre "üründe doğrulandı" diye
yazılmaz (ölçülen şey bench düzeneğindeki ürün veri yoludur; §3.1).

## 0. Karar cümlesi

**Güncel ürünün (CC-PORT motoru + şeritler + uyarlamalı FEC) TCP ve QUIC'e karşı hangi hedef-ağ hücrelerinde
aralıkları ayrışarak önde ya da geride olduğu, ürün ve pitch iddialarının hangi hücrelerde savunulabileceğini ve
FEC'in bant bedelinin hangi hücrelerde tamamlanma olarak karşılığını verdiğini belirler.**

Hedef ağ (ürünün açık hedefi): yüksek kayıp, yüksek gecikme, düşük bant ve zayıf/asimetrik dönüş yolu — TCP ve
QUIC'in zorlandığı koşullar. Her hücre bu hedeflerden en az birini temsil eder (§4).

## 1. Neden yeni kampanya

Önceki iki karşılaştırma bugünkü ürünü tarif etmiyor:

- **Kama ↔ TCP v1** (Eylül 2026, dal `campaign/kama-vs-tcp-v1`, birleşmedi): Kama kolu donmuş eski taşıma
  motoruydu; FEC tek başına kapatılamadığı için nedensel katkısı ayrıştırılamadı; QUIC, bant ekseni ve dönüş yolu
  yoktu. Sonuç: 30 hücrenin 13'ünde Kama önde, 0'ında TCP önde.
- **Kama ↔ quinn v1** (Temmuz 2026): manşet bulgusu ölçüm artefaktı olarak geri çekildi; eski motor, 5 tekrar.

O tarihten bu yana ürün motoru değişti: CC-PORT (quinn-proto BBR + RFC 9002 portu) varsayılan motor; şerit başına
sıra ve BDP'den dosya penceresi (#199); düşük bantta FEC parite tavanı düzeltmeleri (#190); PTO probunun bekleyen
öncelikli yeni birimi taşıması (#211, CC-PORT U17). Bu kampanya v1'in
hüküm kuralını ve v1 hücrelerini (süreklilik için) korur, üstüne QUIC kollarını, düşük bant ve asimetrik dönüş
eksenlerini ve FEC'siz ürün kolunu ekler.

## 2. Kapsam dışı (bilinçli; koşulmayacak)

| ne | neden |
|---|---|
| Burst (Gilbert–Elliott) kayıp satırı | Görev kuralı: yalnız `harness/netem-profiles.sh` destekliyorsa. O dosyada `gemodel` profili YOK (profiller: `p1_loss50` … `p7_bbr`, yalnız IID kayıp + hız + jitter). |
| Gerçek ürün düğümü (`kama-node` + `kama-app`, `f0_flows` düzeneği) | Kampanya bench kolunu ölçer (§3.1). Ürün düğümünün el sıkışma zarfları o düzenekte netem dışından geçer; ayrı bir iştir. |
| Gerçek Noise AEAD | Bench kolu `IdentitySeam` taşır (§3.1, §9). |
| TLS'li TCP, BBR'li TCP, KCP | v1 TCP kolu değişmeden kalır (görev kuralı); yeni bağımlılık yasak. |
| Gerçek RF / jammer / saha | Veri yok. |

## 3. Kollar

| kol | ne | komut özü (iki uç) |
|---|---|---|
| `kama-port` | Ürün varsayılan yapılandırması: CC-PORT motoru, uyarlamalı FEC (profil 0x05), şerit kipi (§3.1) | `bench serve/client --transport kama-product --lane text\|file`, ortam `KAMA_TRANSPORT_ENGINE=cc-port` |
| `kama-port-nofec` | Aynısı, FEC paritesi sıfır (§3.2) | + `--fec-mode static --fec-r 0` (iki uçta) |
| `quinn` | quinn 0.11.11 / quinn-proto 0.11.19, varsayılan tıkanıklık denetimi (CUBIC) | `bench serve/client --transport quinn --cc cubic` |
| `quinn-bbr` | Aynısı, quinn-proto'nun BBR'si (`BbrConfig`; kaynakta "Experimental") | `--cc bbr` (iki uçta) |
| `tcp` | v1 TCP kolu, DEĞİŞMEDEN: Linux TCP (çekirdek varsayılanı, CUBIC), `TCP_NODELAY`, TLS yok | `baselines/target/release/tcp_adapter server/client` |

Tam komutlar `scripts/tnet/tnet_exec.py` `commands()` içindedir; her satırın koşu dizinine `cmd.json` olarak yazılır.

quinn kollarında tek değişiklik: istemci yerel soketi sunucunun adres ailesinden kurar (IPv4 sunucu → `0.0.0.0:0`;
eskiden çift yığınlı `[::]:0`). Telde aynı IPv4 datagramı gider; değişikliğin nedeni IPv6'sı olmayan çekirdekte
`[::]:0`'ın kurulamamasıdır (os error 97). Tıkanıklık denetimi, akış modeli ve TLS değişmez.

### 3.1 Bench Kama kolu ürün yolunu kullanıyor mu — inceleme ve yapılan değişiklik

`bench --transport kama-product` (`crates/bench/src/kama_product.rs`) ürünün veri yolunu sürer: istemcide
`client_core::runtime::ClientEndpoint`, sunucuda `ServerEndpoint` — ürün S1 adaptörünün
(`client_core::transport::ClientTransport`) oturum başına açtığı uçların aynısı. Yapılandırma
`KamaV1Profile::runtime_config()`'ten başlar; motor `TransportEngine::default()` = `CcPort` (ürün varsayılanı).
Bayraksız yolda ürün profilinden sapma listesi boştur (`bayraksiz_kiyas_yolu_kanonik_profildir` testi).

Bulunan iki boşluk ve düzeltme (yalnız `crates/bench`; ürün davranışı değişmez):

1. **Motor kimliği koşu çıktısında yoktu.** `profile_delta` motoru kıyaslamaz; eski motorla koşan bir kol
   `kama-v1-full` etiketini taşıyabilirdi. Ayrıca `KAMA_TRANSPORT_ENGINE=` (boş değer) eski motoru seçer.
   Düzeltme: `KP-MANIFEST` satırına `engine=`, koşu çıktısına (`RunMeta.engine`) motor etiketi yazılır; yürütücü
   iki uçta `KAMA_TRANSPORT_ENGINE=cc-port`'u açıkça verir ve her satırda `engine == cc-port` doğrular
   (aksi hâlde `ENGINE-MISMATCH`).
2. **Şerit katmanı yoktu.** Ürün profilinde (`ClientTransportConfig::product`) şeritler açıktır: S1 birimi
   sınıfının şeridinden uca verilir; FILE şeridinde teyitsiz birim sayısı yolun BDP'sinden türeyen `W` ile
   sınırlıdır. Bench kolu birimleri doğrudan uca (şerit 0) veriyordu. Düzeltme: `bench client --lane text|file`
   (varsayılan `none` = eski davranış, bit-aynı). Pompa, `ClientTransport::pump_session`'ın tek şeritli eşleniğidir
   ve kararın girdilerini ürün kodundan alır: `lanes::file_window_for_path` (taban `LaneConfig::default()`),
   `ClientHandle::path_estimate`, `ClientHandle::lane_confirm_frontier`, `LaneConfig::default().admit_widths`,
   `unit_id::tagged`, `ClientSender::send_in_lane`. Tur aralığı 20 ms = ürün düğümünün boştaki tur aralığı
   (`kama-node` `NODE_TICK`); ayrıca her sunumdan hemen sonra bir tur. Eşlenik olmasının nedeni: ürünün
   `SessionLanes` tipine birim koymak `kama_contracts` tiplerini adlandırmayı gerektirir; bench o crate'e bağımlı
   değildir ve bağımlılık eklemek bu kampanyada yasaktır.

**Şerit eşlemesi (ön-kayıtlı):** `small` iş yükü (48 kısa mesaj) = metin → TEXT şeridi; `large` iş yükü
(101 × 995 B ≈ 100 KB) = dosya → FILE şeridi (BDP penceresi). Yürütücü her Kama satırında kol etiketinin
`-lane-text`/`-lane-file` ile bittiğini doğrular (aksi hâlde `LANE-MISMATCH`).

**Kalan, beyan edilen boşluklar** (KTP2-F0 kataloğundaki beyanla aynı): (i) kripto yok — `IdentitySeam` datagram
başına 16 B taşır, ürünün Noise zarfı 22 B'tır; (ii) alıcıda S1 sıralayıcısı (`Reorder`) yok — teslim ölçümü
göndericideki ilk teyit olduğu için tamamlanmayı etkilemez; (iii) el sıkışma yok — WARM kipte kurulum zaten
pencerenin dışındadır; (iv) `kama-app` katmanı (Messenger/FileTransfer çerçeveleri, alındılar) yok; (v) alıcıda
şerit kabul genişliği kurulmaz — iş yükleri (≤ 109 birim) genişliklerin (TEXT 256, FILE 4096) altında kalır,
sınır bağlamaz. **Bu yüzden Kama sayıları bench yolunda ürün için bir ÜST SINIRDIR**; özellikle 62 B/s dönüş
hücrelerinde 6 B/datagram fark görünür olabilir (§9).

### 3.2 FEC'siz kol — mevcut düğme

`--fec-mode static --fec-r 0` mevcut bir bench düğmesidir (`FecArm::StaticR { r: 0 }` → `FecRuntimeConfig
{ manual_r: Some(0) }`). FEC profili (0x05), tel çerçevesi, `SourcePrefix` ve PARAM önyüklemesi kurulu kalır;
blok kapanışında `close_fec_block_manual(…, r = 0, …)` hiç parite üretmez. Yani iki kol arasındaki TEK fark
paritenin (ve onun kurtarımının) varlığıdır — FEC'in nedensel katkısı ve bant bedeli bu farktır. `--fec-profile 0`
SEÇİLMEDİ: FEC alt-sistemini söker, birim boyunu ve tel çerçevesini de değiştirir (ikinci eksen). Bu kolun
beklenen sapma listesi tam olarak `["fec Some((5, Some(0))) != Some((5, None))"]`dir ve kimliği `kama-custom`'dır;
yürütücü bunu her satırda doğrular. Not: statik kipte uyarlamalı denetleyicinin (yapı kapısı, spekülatif kol) kararı
latch'e akmaz; o kararların tek çıktısı parite olduğu için fark yine paritedir.

### 3.3 Teslim tanımı (v1 ile aynı)

Gönderim anı = uygulamanın mesajı sunduğu an (burst: hepsi t0'da). Teslim anı = mesajın uygulama düzeyi teslim
teyidinin göndericiye varışı. Kama: birimin ilk ACK'i ya da FEC-RECOVERED teyidi (alıcı yalnız teslim yoluna
kabul ettiğini teyit eder). TCP: sunucunun mesaj başına yazdığı 8 B uygulama ACK'i. quinn: mesaj başına bi-stream,
sunucunun 1 B yanıtı. WARM: uç kurulur + 8 eş boy ısıtma mesajı, SONRA t0 (kurulum ve ısıtma pencerenin dışında;
el sıkışma başarısızlığı tamamlanmama sayılır).

## 4. Hücreler (54 hücre)

netem yalnız orta ad alanında, yön başına: ileri = istemci→sunucu (veri), dönüş = sunucu→istemci (ACK/teyit).
Topoloji `scripts/netem/kapi3_harness.sh` (üç ad alanı, IPv6 kapalı, dört arayüzde statik ARP — 0,5 kbit/s'de
ARP'nin netem kuyruğunda ölmesini önler) ; offload (TSO/GSO/GRO) ioctl ile kapatılıp geri okunur. netem `limit`
varsayılan (1000 paket), satır başına kaydedilir. Her satırda netem önce silinir sonra eklenir ve tohumlu dize
`tc -j` dökümüyle sayısal doğrulanır.

İş yükleri: `small` = 48 × 101 B (4 848 B), `large` = 101 × 995 B (100 495 B), burst sunum.
RTT sınıfları (tek yön): `low` 25 ms, `med` 125 ms, `high` 500 ms (v1), `field` 300 ms (saha profili).

| grup | hücreler | netem ileri | netem dönüş | neyi temsil eder |
|---|---|---|---|---|
| `control` | 2 (iki iş yükü) | `delay 0ms` | `delay 0ms` | bozulmasız; her kol N/N tamamlamalı (§8) |
| `main` | 30 = kayıp {10,20,30,40,50} % × RTT {low,med,high} × 2 iş yükü | `delay D loss L%` | `delay D loss L%` | v1 sürekliliği: yüksek kayıp × yüksek gecikme |
| `band` | 14 = RTT {med,high} × [small: kayıp {0,10,30,50} %; large: {0,10,30} %] | `delay D loss L% rate 100kbit` | `delay D loss L%` | düşük bant (ileri 12,5 kB/s) × kayıp |
| `asym` | 4 = ileri kayıp {0,20} % × 2 iş yükü, RTT field | `delay 300ms [loss L%] rate 100kbit` | `delay 300ms rate 0.5kbit` | zayıf dönüş (≈ 62 B/s), saha profili |
| `asym-twin` | 4 = aynısı, dönüş SINIRSIZ | `delay 300ms [loss L%] rate 100kbit` | `delay 300ms` | `asym`'ın ikizi: yalnız dönüş hızı farklı |

**Gerekçeler.**
- `band`/`large`/%50 kaldırıldı: v1'de hız sınırı olmayan ortamda bile `large` + %50 kayıp + RTT med/high
  hücrelerinde iki kol da ≤ 1/60 tamamladı; bant eklenince tamamlanma beklenmez, hücre en pahalısıdır ve bilgi
  üretmez. `small` + %50 korunur (v1'de üç RTT sınıfında da ayrıştı).
- `asym` saha profili ürünün kalıcı kapılarındaki profildir (ileri 12,5 kB/s, dönüş 62 B/s, tek yön 300 ms, ileri
  %20 kayıp, dönüş kayıpsız). %0 hücre saf dönüş etkisini ayırır. `asym-twin`, aynı hücrenin dönüşü sınırsız
  hâlidir; ikisinin farkı dönüş yolunun nedensel etkisidir.

### 4.1 Horizon ve deadline (a-priori, v1 formülü)

`H = ⌈10 s + 10·RTT + 10·ser⌉`, `ser = yük_baytı · 8 / ileri_hız` (hız yoksa 0); `D = 3·(ser + 1,5·RTT)` (betimsel
sütun, ürün SLO'su değil). Değerler: `main` small/large 11/11 (low), 13/13 (med), 20/20 (high) s; `band` small
17 (med) / 24 (high) s, large 93 (med) / 101 (high) s; `control` 11 s.

**İstisna — `asym` ve `asym-twin`: H = 180 s sabit.** v1 formülü dönüş darboğazını içermez (small için 20 s
verirdi). 62,5 B/s dönüşte 1 280 B'lık tek bir datagram 20,5 s hat süresi alır; v1 teslim tanımıyla mesaj başına
bir teyit taşıyan bir kol (TCP: 8 B ACK + 52 B başlık ≈ 60 B/mesaj) `large` için ≈ 6 kB, ≈ 97 s dönüş hattı ister.
180 s ≈ 11 kB dönüş kapasitesidir (bu alt sınırın ≈ 1,9 katı). İkiz aynı horizon'u kullanır (hücre çifti eş
koşulda kıyaslanır).

## 5. N, tohum, sıra, tavanlar

- **N = 30 / kol / hücre / replika, iki replika (R1, R2)** → hücre başına kol başına 60 (havuz). Toplam
  54 × 5 × 30 × 2 = **16 200 satır**.
- netem tohumu `sha256("KTN2|replika|grup|hücre|rep|yön")` türevi: aynı rep'in beş kolu AYNI tohumu paylaşır
  (eşleşmiş tasarım); R1/R2 ve iki yön ayrık. Aynı tohum aynı paket-indeksi kayıp dizisini üretir; protokol
  zamanlaması dizinin hangi pakete düştüğünü değiştirir → tekrar istatistikseldir, bit-bit değil.
- Hücre içinde bir rep'in beş kolu ardışık koşar; kol sırası `(rep + hücre_indeksi) mod 5` kadar döner.
- Blok = (replika, hücre, ardışık 10 rep) = 50 satır; bir işçi bloğu seri koşar. Bloklar tahmini maliyete göre
  azalan sırayla (LPT) en çok J eşzamanlı işçiye verilir. Her işçi kendi `unshare -rnm` ad alanında kendi
  topolojisini kurar.
- İstemci süreç tavanı `2·H + 90 s` (quinn el sıkışması ≤ H + ısıtma ≤ 10,3 s + pencere H + pay); aşılırsa
  `rc=124`, satır `ERROR-NO-RESULT` ve paydada başarısız.

## 6. Süre aritmetiği (12 çekirdek)

**İşçi sayısı J = 10** (12 çekirdekten 2'si çekirdek/netem yumuşak kesmeleri ve sistem için boş). Her satır iki
süreçtir ve çoğu zaman G/Ç bekler.

Model (`python3 scripts/tnet/tnet_manifest.py --estimate --jobs 10` çıktısı):

| grup | satır | model işçi-saat | × 1,28 (v1'in tahmini aşma oranı) |
|---|---|---|---|
| control | 600 | 0.2 | 0.3 |
| main | 9 000 | 47.7 | 61.0 |
| band | 4 200 | 27.0 | 34.5 |
| asym | 1 200 | 41.0 | 52.5 |
| asym-twin | 1 200 | 20.7 | 26.5 |
| **toplam** | **16 200** | **136.5** | **174.8** |

- J = 10, LPT: model duvar süresi **13,7 sa**; v1 aşım oranıyla **17,5 sa** (en uzun blok ≈ 2,2 sa).
  J = 12 ile 14,6 sa.
- Kötü durum sınırı (her satır kendi tavanında: 2H + 90 s + 3 s): 860 işçi-saat = 86 sa (J = 10). Bu bir sınırdır,
  beklenti değil; aşağıdaki pilot bunu ölçümle daraltır.
- Model kaynakları: `main`/`control` satır maliyeti v1 pilotundan ölçülen satır-başı ortalama saniye; quinn kolları
  × 1,25 + 1 s. `band`/`asym` için v1 verisi yok: `3 s + f(kayıp)·H`, `f` = {0: 0,15; 10: 0,30; 20: 0,45;
  30: 0,60; 50: 0,85} (kötümser); `asym`'da `f ≥ 0,5`, + 10 s ısıtma, quinn'e + 50 s el sıkışma (62 B/s dönüşte
  sunucunun el sıkışma uçuşu ≈ 2,5–3 kB).

### 6.3 Süre pilotu ve kesme kuralı (sonuca bakmadan)

Kampanyadan önce `--pilot` koşulur: 7 pahalı köşe hücre × 5 kol × 2 rep = 70 satır (replika `P`). Pilot YALNIZ
satır duvar süresi için okunur; tamamlanma değerlerine bakılmaz, analizör `P` replikasını reddeder.
`tnet_campaign.py --project <pilot dizini>` grup başına ölçülen/model oranını tam manifeste uygular.
İzdüşüm **> 26 sa** ise, bu sırayla ve yeni bir donma commit'iyle (gerekçesi yazılarak) kesilir:
(1) `band` ve `asym`/`asym-twin` N 30 → 20 / replika; (2) `main` %10 kayıp hücrelerinde N 30 → 20; (3) yine
> 26 sa ise koşu başlatılmaz, sahibe sorulur. Kesme yalnız N'yi düşürür; hücre, kol, horizon, tohum değişmez.

## 7. Metrikler ve hüküm kuralı

Hücre × kol başına (R1+R2 havuz; ayrıca replika başına):

1. **Horizon içinde tamamlanma** (v1 tanımı): `status = OK` ∧ teslim = N mesaj ∧ son teslim ≤ H. Oran + Wilson %95.
2. **Tamamlanma süresi p50/p90** (tamamlanan koşularda; en yakın sıra yüzdeliği; betimsel).
3. **Teslim edilen yük baytı başına tel baytı**: istemci veth arayüzünde (`va0`) koşu boyunca tx+rx bayt farkı
   (kurulum + ısıtma DAHİL — her kolda aynı kural, v1 ile aynı). İki biçim: `Σtel / Σ(teslim_edilen_mesaj ×
   yük)` (bütün koşular) ve tamamlanan koşularda `tel / (N × yük)` medyanı. Yön ayrımı (`wire_tx_bytes`,
   `wire_rx_bytes`) ve netem paket/düşürme sayaçları satırdadır.
4. Betimsel ek sütunlar: deadline tamamlanması (D), Kama parite baytı payı (`kama_fec_bytes`), ısıtma artığı.

**Hüküm (v1 ile aynı):** "ÖNDE" yalnız iki kolun Wilson %95 aralıkları ayrışıyorsa; aksi hâlde "ayırt
edilemedi". Karşılaştırmalar hücre başına: `kama-port` ↔ `tcp`, ↔ `quinn`, ↔ `quinn-bbr` (ürün iddiası);
`kama-port` ↔ `kama-port-nofec` (FEC): `kama-port` önde = "FEC karşılığını verdi", `nofec` önde = "FEC zarar
verdi", aksi = "ayırt edilemedi" — her durumda parite payı ve tel/yük oranı yanında yazılır. Çoklu karşılaştırma
düzeltmesi YAPILMAZ; hükümler hücre bazında okunur. Kama'nın geride olduğu hücreler ve tamamlanma süresinde geride
olduğu hücreler (iki kol ≥ %90 iken p50) önde olduğu hücrelerle AYNI görünürlükte raporlanır.

**Ek denetimler:** R1 ↔ R2 (kol × hücre aralıkları ayrışıyor mu); `load_flag` = `loadavg1 > 0,75 × nproc`
satırlar dışlanarak hükümlerin yeniden hesabı; satır muhasebesi (beklenen/üretilen/eksik/fazla/yinelenen/status≠OK).
Analizör: `scripts/tnet/tnet_analyze.py` → `ACCOUNTING.md`, `CELL-RESULTS.csv`, `GRAPH.csv` (v1 şekli: ilk 14 sütun
aynı sırada, v2 eksenleri sona eklenir), `TABLES.md`.

## 8. Geçerlilik ve DUR kuralları

- `status ≠ OK` satır paydada kalır ve başarısız sayılır; ayrıca listelenir.
- `ENGINE-MISMATCH`, `PROFILE-MISMATCH` ya da `LANE-MISMATCH` satırı varsa o kolun hücreleri ürün sonucu sayılmaz;
  analizör uyarı basar, sebep bulunup koşu düzeltilmeden rapor yazılmaz.
- `control` hücrelerinde bir kol N/N tamamlamazsa yorum yapılmaz; düzenek araştırılır.
- netem doğrulanamayan ya da bağlantı kapısı tutmayan satır üretilmez, logda `NETEM-SKIP` görünür; muhasebede eksik
  satır olarak raporlanır.
- Koşu sırasında ve sonrasında hücre, N, horizon, metrik, analizör değiştirilmez. Analizörde hata bulunursa
  düzeltme ayrı bir commit'te, eski ve yeni çıktı yan yana raporlanır.

## 9. Önceden beyan edilen geçerlilik tehditleri

1. **Emülasyon ≠ saha.** Bozulma Linux netem'dir; gerçek radyo/uydu/hücresel değil. Kayıp IID'dir; burst yok (§2).
2. **`kama-port` kolu uygulamanın bütünü DEĞİLDİR.** Kol = CC-PORT motoru, ürün veri yolunun uçları
   (`client_core::runtime` `ClientEndpoint`/`ServerEndpoint`) ve `crates/bench` içindeki şerit pompası eşleniği.
   Tam `ClientTransport` (S1 adaptörü) / `kama-node` / `kama-app` yığını YOKTUR; gerçek AEAD (Noise) YOKTUR. Sonuçlar
   taşıma motorunu ve veri yolunu tarif eder, uygulamanın bütününü değil; bir hücredeki hüküm "Kama uygulaması bu
   ağda şunu yapar" diye yazılamaz.
   **Bu kol ürün için üst sınırdır** (§3.1): 16 B sahte etiket (ürün 22 B), sıralayıcı yok, kama-app yok.
   62 B/s dönüşte datagram başına 6 B önemlidir: ACK datagramları ~ 40–60 B olduğundan ürünün dönüş baytı bench'ten
   yaklaşık %10–15 fazla olabilir. Analiz bunu sayıyla düzeltmez; `rev_pkts` sütunu ölçeği verir.
3. **Teslim tanımı rakipleri zayıf dönüşte zorlar.** v1 tanımıyla TCP ve quinn mesaj başına bir uygulama teyidi
   yollar; birikimli alındı kullanan bir TCP uygulaması 62 B/s dönüşte daha az bayt harcardı. `asym` hücrelerinde
   bir "kama-port ÖNDE" hükmü "mesaj başına teyit isteyen uygulama, TCP/QUIC üstünde" iddiasıdır; taşıma düzeyi
   teslim tanımlı rakibe karşı iddia DEĞİLDİR.
4. **Şerit pompası eşleniktir** (ürün kodunun kendisi değil; §3.1). Ürün düğümünün tur kadansı yük altında 20 ms'den
   kısa olabilir.
5. **FEC'siz kol statik r = 0'dır**: FEC alt-sistemi kurulu, parite sıfır. "FEC'i hiç olmayan ürün" ölçülmez.
6. **quinn'de TLS 1.3 kapatılamaz** (el sıkışma pencere dışında ama tel baytına dahil); TCP kolunda TLS yok → TCP
   lehine muhafazakâr.
7. **netem `limit` 1000 paket**: hız sınırlı hücrelerde kuyruk taşması olmaz; tamponu küçük bir radyo hattında
   TCP'nin yavaş başlangıç kayıpları ek olurdu.
8. **Paralellik.** 10 eşzamanlı işçi; kollar aynı blokta dönüşümlü koşar (olası yük etkisi kollara eşit düşer);
   `load_flag` duyarlılığı raporlanır.
9. **Isıtma artığı.** Yüksek kayıpta ve zayıf dönüşte 8 ısıtma mesajının teyidi tavana (Kama/TCP 30 s, quinn mesaj
   başına 10 s) kadar gelmeyebilir; artık trafik pencereye taşar. Satır başına `warmup_residue` raporlanır.
10. **Çoklu karşılaştırma.** 54 hücre × 4 karşılaştırma; düzeltme yok, hücre bazında okunur. N = 60'ta Wilson
    aralığı en geniş hâlinde ≈ ±0,12'dir: daha küçük farklar ayırt edilemez.
11. **Tek makine, tek çekirdek sürümü, CUBIC.** Çekirdek ve `tc` sürümü ENV kaydındadır.

## 10. Sonuç CSV şeması (`results.csv`, satır başına bir koşu)

`run_id, replica, group, cell, loss_pct, loss_dir, rtt_class, owd_ms, rate_fwd, rate_rev, workload, msgs, payload,
arm, mode, rep, horizon_ms, deadline_ms, status, delivered, complete_horizon, complete_deadline, t_complete_ms,
wire_bytes, wire_tx_bytes, wire_rx_bytes, fwd_pkts, fwd_drops, rev_pkts, rev_drops, netem_limit, netem_seed_fwd,
netem_seed_rev, loadavg1, nproc, load_flag, warmup_residue, setup_ns, connect_failed, profile_id, profile_delta,
engine, kama_arm, bin_commit, kama_fec_bytes, kama_retx_bytes, rc, row_wall_s, block, worker_pid, seq_no, unix_ts`

- `status` ∈ {`OK`, `ERROR-NO-RESULT`, `ENGINE-MISMATCH`, `PROFILE-MISMATCH`, `LANE-MISMATCH`, `SMOKE-NO-NETEM`
  (yalnız bulut dumanı; kampanya satırı değil)}.
- `t_complete_ms`: t0'dan son teslime (yalnız N/N teslimde). `delivered`: teslim edilen mesaj sayısı.
- `wire_*`: `va0` bayt farkı (tx = ileri, rx = dönüş). `fwd_*`/`rev_*`: orta ad alanı netem sayaç farkları
  (`packets` = iletilen, `drops` = düşürülen).
- `profile_id`/`profile_delta`/`engine`/`kama_arm`/`bin_commit`: Kama kolunun kendi beyanı (koşu çıktısı ve
  `KP-MANIFEST` satırı); rakiplerde boş ya da kol adı.
- `row_wall_s`: netem kurulumundan satır yazımına duvar süresi (pilot izdüşümü bunu kullanır).
- Ham koşu dizinleri (`runs/<run_id>/`: `cmd.json`, `c.err`, `s.err`, `out.jsonl`, `manifest.json`, `row.json`)
  çıktı dizininde kalır, repoya girmez.

## 11. Yürütme

`scripts/tnet/tnet_campaign.py --out <ÇIKTI> --jobs 10` (koordinatör; ad alanı dışında) → her blok için
`unshare -rnm scripts/tnet/tnet_exec.py`. Başlangıçta manifestler üretilir ve §12 sha256'larıyla karşılaştırılır;
ölçülen kodun donmuş koddan farkı (`git diff <code_sha> HEAD -- crates baselines scripts/tnet …`) ya da commit'lenmemiş
kod değişikliği varsa koşu başlamaz. ENV (`ENV.json`, her başlatmada `ENV-<n>.json`): çekirdek, `tc`/`ip` sürümü,
CPU modeli, nproc, bellek, git HEAD, ikili sha256'ları, sysctl'ler, miras alınan `KAMA_TRANSPORT_ENGINE`.
Devam: aynı komut; `results.csv`'deki `run_id`'ler atlanır, üstüne yazılmaz. Sonuç ve ENV, yerel koşucu tarafından
`reports/kama-target-net-v2/` altına commit'lenir. Ayrıntı: yerel iş metni.

## 12. Donmuş kimlikler

Ayrıntılı kayıt `reports/kama-target-net-v2/FREEZE.json` (bu belgeyle aynı commit; `python3
scripts/tnet/tnet_freeze.py --verify` ile doğrulanır).

| nesne | değer |
|---|---|
| ölçülen kod (`code_sha`) | `0dc5e862a91226698208ad0e4559630ec002f649` |
| taban `main` | `3abee42990494cff05abfb553e5a347cfe45d12f` |
| manifest R1 sha256 | `04ee4c3a4fc4651b6650e129a0d7f472b2bceb9d0fa3a2ecab4db6d11e31e91f` |
| manifest R2 sha256 | `f93c3de5189f54c29ee752033b901e41fcfc85ec21936d9788b0f3a231179030` |
| manifest P (pilot) sha256 | `1906c496b02044f132a79c41799e2fbf9f095e0f72038d6609197a622397f9c5` |
| `scripts/tnet/tnet_analyze.py` sha256 | `b25bf22d69c8cc204f04c6136f724d4b74e3a954a75fb0f2dfdd89601b3a7ef9` |
| `scripts/tnet/tnet_exec.py` sha256 | `012817c2a2abb63b43d151491e00884da82dbb798a5e98d5c53ce088c31b25ef` |
| `baselines/src/bin/tcp_adapter.rs` sha256 (v1 ile aynı) | `abfcb006f02625f3af364d4bf326f8b17cca3c0794bca2ceb3171ab1c2c21f93` |

İkililer donma kaydında yoktur (derleme makinesine bağlıdır): yerel koşu `code_sha` ağacından
`KAMA_GIT_SHA=<HEAD>` ile derler; her Kama satırı ikilinin commit'ini (`bin_commit`, `KP-MANIFEST`) ve ENV kaydı
ikililerin sha256'ını taşır. Koordinatör, `crates`, `baselines`, `scripts/tnet`, `Cargo.*` ve
`scripts/netem/kapi3_harness.sh` `code_sha`'dan farklıysa başlamaz.

## 13. Bu ön-kaydın değişme kuralı

Ölçülen kod SHA'sı yalnız İLK kampanya satırından (R1/R2) ÖNCE ve yalnız daha yeni bir `main`'e (ör. #211
birleşirse) taşınabilir: yeni bir donma commit'i, gerekçe ve yeni sha256'larla. Hücre, kol, N, horizon, metrik ve
hüküm kuralı o durumda da değişmez. İlk kampanya satırından sonra hiçbir şey değişmez.

**Uygulanan taşıma (2026-11-16):** #211 (CC-PORT U17, `adaptive-transport` göndericisi) `main`'e birleşti
(`3abee429`); hiçbir kampanya satırı koşulmamıştı. `origin/main` dala birleştirildi (`f2bf58a8`), ölçülen kod
`0dc5e862`'ye taşındı (ilk donma: kod `36f858cf`, donma commit'i `92cc0b2c`). Manifestler, yürütücü ve analizör
değişmedi (sha256'ları aynı). Değişen iki betik: `tnet_freeze.py` taban SHA'sını argümanla alır; `tnet_campaign.py`
(ve `tnet_env.py`) kod kapısında izlenmeyen dosyaları saymaz — birim testinin bıraktığı `__pycache__` gerçek koşuyu
"commit'lenmemiş kod" diye durduruyordu (taşıma sırasında bulundu). Yeniden koşulan ucuz denetimler: betik birim
testleri, loopback 5 kol × 2 iş yükü, `cargo test --release -p bench`, gerçek kipte başlangıç kapıları.
