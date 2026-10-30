# KTP-2 taslak 0.3 — denetim notları (F0 · Ajan E)

**Denetlenen:** `decisions/C17-KTP2-F0/KTP-2-PROTOKOL-TASLAGI-2026-10-29.md` (taslak 0.3, sha256 `c8655b9f…c3329e`).
**Kod tabanı:** taban `a858f3fc` (bu worktree; `crates/` farkı yok), aday `49864510` (`wt/ktp2-05b`). Aşağıda anılan kod satırları tabandadır; aday ağacında `transport.rs`, `wire.rs`, `budget.rs`, `kama-fec-codec` değişmedi, `runtime.rs`/`sender.rs`'te ilgili satırların içeriği aynı, yalnız numaralar kayar.
**Kapsam:** yalnız taslağın fiziksel/matematiksel tutarlılığı ve koddan iddiaları. Tasarım önerisi yok; her maddede yalnız doğru değer.

**Etiketler:** [hesap] = betikteki kesin aritmetik (Fraction/math.comb; ölçüm değildir) · [kod] = kaynak okuması (dosya:satır) · [çıkarım] = ölçülmemiş akıl yürütme · [betimsel] = karar dışı bilgi. Bu şerit koşu yapmadı; [ölçüldü] yalnız V-25 ve V-26'da, B ve C raporlarından aktarılan sayılardadır. [ölçülen itiraz] = taslağın varsayımına bu ölçümlerden gelen itiraz; hesap hatası değildir.

**Betikler** (`scripts/ktp2/f0/e/`, `python3 -I`, birleşik çıktı `CIKTI.txt`): `fec_olasilik.py` · `iletim_dusuk_bant.py` · `k6_ses.py` · `sinirlar.py` · `yuk_tablosu.py`. Her sayının yanında `betik · etiket` yazılıdır; etiket betik çıktısındaki `== ETİKET ==` satırıdır.
Yeniden üretim: `for f in fec_olasilik iletim_dusuk_bant k6_ses sinirlar yuk_tablosu; do python3 -I scripts/ktp2/f0/e/$f.py; done`

**Ortak model (aksi yazılmadıkça):**
- Tel boyu = yük + KTP-2 başlığı (zarf 22 + AKIŞ 9 = 31 B) + L. **Taslağın varsayımı L = 0** (UDP yükü). Varyantlar: L = 28 (IPv4 20 + UDP 8), L = 42 (Ethernet 14 + IPv4 20 + UDP 8; netem'in veth üzerinde saydığı çerçeve). ccr §35.6 kendi hesabını L = 28 ile yapar ("Kama SESSION_DATA / UDP / IPv4 = 50B + payload").
- Kayıp bağımsız Bernoulli. "İletim sayısı" ileri yönde ilk başarılı alıma kadardır; ACK yolu ve iki yönlü kayıp ayrıca yazılmıştır.
- "n iletimin süresi" = n × serileştirme (art arda gönderim; ARQ beklemesi yok) — taslağın K-3 modeli. ARQ yolu ayrı hesaplandı (V-4).

Sayım: **hata 15** (hesap/iç tutarlılık 10, kod iddiası 5) · **varsayıma duyarlı 26** (2'si [ölçülen itiraz]) · **doğrulanan 24** (hesap 20, kod 4).

---

## 1. Hatalar — hesap ve iç tutarlılık

**H-1 · §6 K-6 ses gecikme bütçesi örneği**
- Taslak: "200 + 200 + serileştirme + 300 + titreşim → ≈ 750 ms + serileştirme + titreşim".
- Doğru: 200 + 200 + 300 = **700 ms** + ser + titreşim [hesap; `k6_ses.py · K6-05`]. Örneğin K = 2'si §3.8 tablosunda yok; tablodaki K ile (200 ms demet, RTT 600 ms, ser ve titreşim hariç): %20'de K = 3 → **900 ms**, %10 ve %30'da K = 4 → **1100 ms**.
- Etki: aritmetik farkı 50 ms; asıl fark K'den. 400 ms son tarih RTT 600 ms'de tablo K'leriyle ancak demet ≤ 33,3 ms (K = 3) / ≤ 25 ms (K = 4) ile; 800 ms son tarih demet ≤ 166,7 / 125 ms ile; RTT 2 s'de ikisi de olanaksız (D − RTT/2 < 0) [betimsel; `K6-06`]. Ajan C'nin demet adayları (60/120/200 ms) RTT 600 ms'de 400 ms son tarihine tablo K'leriyle sığmaz.

**H-2 · §6 K-6 bütçe formülü ↔ §3.9 T_ser**
- Taslak: bütçede serileştirme = "(K + R) × paket boyu / hız; yalnız darboğazdaki kuyruk". §3.9: "T_ser, ses akışı etkinken standart ve arka plan için 2 s … arka plandaki bir parçanın serileştirmesi, araya girmek isteyen sesi … en çok T_ser kadar bekletir". K-6 koşulu "aynı anda dosya aktarımı".
- Doğru: bütçede kesilemeyen paket terimi (S_maks + 31)/r eksik. Ses etkinken §3.9 kuralıyla: 500 B/s'de S = 969 → **2,000 s**; 1 kB/s'de S = 1100 → **1,131 s**; 12,5 kB/s'de **0,090 s** (L = 42 ile 2,084 / 1,173 / 0,094 s) [hesap; `k6_ses.py · K6-07`].
- Etki: taslağın canlı ses için gösterdiği 4–8 kbit/s hatlarda bu terim tek başına 400 ve 800 ms son tarihini aşar. K-1 ölçütü terimi içerir ("2 × en büyük paket serileştirmesi"); K-6 bütçesi içermez.

**H-3 · §3.6 ses hız tavanı ↔ §3.8 FEC tablosu**
- Taslak: "ses sınıfının kendi hız tavanı (yapılandırılmış ses tel hızı × 1,5)"; K-6: "Ses tel hızı (başlık dahil, FEC'siz)"; tablo çarpanları 1,25 / 1,67 / 2 / 3,25 / 9.
- Doğru (tavanın K-6'daki FEC'siz tanımla okunması): 200 ms demette tavan 1,5 × 305 = **457,5 B/s**; tablodaki satırın gerektirdiği ses hızı %10'da 385,0 (sığar), **%20'de 518,3**, **%30'da 625,0**, %50'de 1025,0, %80'de 2865,0 B/s (ONARIM 12 B başlıklı onarımla) [hesap; `k6_ses.py · K6-04`, `K6-03`]. Gereken/FEC'siz oranı %20'de 1,70, %30'da 2,05 > 1,5. 60 ms demette tavan 1000 B/s = 8 kbit/s hattın tamamı.
- Etki: birincil ses profillerinde (%20, %30) tablodaki (K, R) tavana sığmaz. §3.6'daki "kontrol sürekli ses altında bile ilerler (ses tavanlı olduğu için)" gerekçesi 60 ms demette 8 kbit/s hatta tutmaz (tavan = hat hızı).

**H-4 · §3.2 ses sınıfı varsayılan kredisi "son tarih penceresi kadar parça"**
- Model [çıkarım, açık]: kredi = alıcıda yeri açılan en yüksek sıra + W (§3.7: mutlak, ACK'le taşınır). Yer varışta açılırsa kredi döngüsü ≈ RTT → sürekli ses için W·T ≥ RTT; yer oynatmada açılırsa W·T ≥ D + RTT/2.
- Doğru: W = ⌊D/T⌋ ile gönderilebilen çerçeve oranı ≤ min(1, W·T/RTT) (varışta açılma, en iyimser model): D = 400 ms → RTT 600 ms'de **%60–67**, RTT 2 s'de **%18–20**; D = 800 ms → RTT 600 ms'de %100, RTT 2 s'de **%36–40** (T = 60/120/200 ms). Gereken W: RTT 600 ms'de 3–10, RTT 2 s'de 10–34 parça [hesap; `k6_ses.py · K6-08`]. Oynatmada açılma modelinde W = D/T hiçbir RTT > 0'da yetmez.
- Etki: K-6'nın desteklenen profili (RTT ≤ 2 s) ve "son tarihe yetişen çerçeve ≥ %95" ölçütü, RTT > D olan her hücrede varsayılan ses kredisiyle sağlanamaz; bu FEC'ten bağımsızdır (onarım krediyi tüketmez).

**H-5 · §2 adım 4**
- Taslak: "%30 kayıpta 4 ses parçasına 3–4 onarım parçası eklenir".
- Doğru: (4, 3) çerçeve oynatılabilirliği **%92,33** (< %95), blok %87,40; tablonun %95 hedefini sağlayan satırı yalnız (4, 4): %96,22 [hesap; `fec_olasilik.py · FEC-03`].
- Etki: küçük; hikâye §3.8 tablosuyla çelişiyor.

**H-6 · §3.10 (ve §2 adım 6) ↔ §3.4 PTO içeriği**
- Taslak §3.10: "PTO yoklamaları 30 saniyede bir, en çok 48 baytlık paketlerle sürer (≤ 1,6 B/s)". §3.4: PTO'da "en yüksek öncelikli akışın en eski teyitsiz parçası yeniden gönderilir".
- Doğru: §3.4'e göre PTO paketi bir parça taşır: S = 64'te 95 B … S = 1100'de 1131 B → 30 s'de bir **3,17–37,70 B/s** (L = 42 ile 4,57–39,10 B/s) [hesap; `sinirlar.py · PT-01`]. 48 B'a yalnız zarf + YOKLAMA + en çok 4 aralıklı ACK, ya da en çok 17 B yüklü AKIŞ çerçevesi sığar.
- Etki: kesintideki bant tüketimi 2–24 kat düşük yazılmış; K-4 süresi hangi paketin gittiğine bağlı (V-12).

**H-7 · §3.4 kayıp ilanı eşikleri ("QUIC'in kanıtlanmış kayıp tespiti benimsenir, iki farkla")**
- Taslak: "(a) aralarında en az 3 paket varsa ya da (b) gönderim zamanı farkı 9/8 × RTT'yi aşıyorsa".
- Doğru (QUIC): (a) largest_acked ≥ pn + 3, yani arada **≥ 2 paket** (RFC 9002 §6.1.1 kPacketThreshold = 3; [kod] quinn-proto 0.11.18 `src/connection/mod.rs:1728`, `src/config/transport.rs:381`); (b) **şimdi − gönderim(pn) ≥ 9/8 × max(srtt, latest_rtt)** (RFC 9002 §6.1.2; quinn `mod.rs:1700`, `:1727`) — iki paketin gönderim zamanı farkı değil.
- Etki: (a) kayıp ilanı için bir paket fazla gerekir. (b) "iki paketin gönderim farkı" okunursa zaman eşiği yolunda kayıp ≈ 9/8 RTT yerine en erken ≈ 9/8 RTT + 1 RTT'de ilan edilir (RTT 2 s'de ≈ 2,25 s yerine ≈ 4,25 s) [çıkarım; ifade iki okumaya açık]. Taslağın saydığı "iki fark" (aralık listesi, ACK yokken ilan yok) bu sapmaları içermiyor.

**H-8 · §3.2 ARKA PLAN kredisi "16 parça / 64 KB"**
- Doğru: §3.9'a göre S ≤ 1100 B → 16 parça ≤ **17 600 B**; 64 KB (65 536 B) sınırı hiçbir parça boyunda bağlamaz [hesap; `sinirlar.py · KR-01`]. (Mesaj 32 parça / 32 KB ve sinyal 16 parça / 16 KB'ta bayt sınırı S > 1024 B iken bağlar.)
- Etki: yazım tutarsızlığı; pencerenin bant etkisi V-13'te.

**H-9 · §4 KREDİ "oturum bayt sınırı 4" + §3.7 "mutlak … değerler yalnız artar" + §3.4 "oturum zaman geçtiği için kapanmaz"**
- Doğru: u32 mutlak bayt sayacı 2³² B'ta taşar: 12,5 kB/s'de **3,98 gün**, 1 kB/s'de 49,7 gün, 25 B/s'de 1988 gün, 10 Mbit/s'de (K-9) **57,3 dk** [hesap; `sinirlar.py · KR-02`]. Taslakta sarma kuralı yok.
- Etki: kapanmayan oturumda kredi değeri ilerleyemez ya da sarar; K-9 koşusunda bir saatten kısa sürede.

**H-10 · §5 "bugünkü motorun aynı koşuldaki en kötü sınırı ≈ 6,4 gündür"**
- Kaynak [kod]: 05b notu, aday `49864510`, `docs/design/K4G-05b-RETX-FIX.md` B.1d satır 45b ve B.6 h.9.7: T_birim(R1) ≤ max(H_maks, A) + A + N_cap·T_max(kopya) + τ; N_cap = 4096 (`max_inflight_units`, `runtime.rs:259`); 1046 B birim; 31 B/s tam pace hızı → 553 350 s.
- Doğru: aritmetik tutuyor — baskın terim 4096 × 4 × 1046 / 31 = 552 828 s = 6,40 gün (notla %0,09 fark) [hesap; `sinirlar.py · S5-03`]. Ama (i) türetim **05b adayının** R1 rejimidir (DUR-45 (C) H-sahiplenmesi; 49864510'da var), taban `a858f3fc`'nin değil — H-sahiplenmesi 05b mekanizmasıdır (`adopt_due_ns` / `adopt_unowned` tabanın `sender.rs`'inde 0 kez, adayınkinde 6 kez geçer), taban için bu sınır türetilmemiş; (ii) koşul aynı değil: 1046 B birim / tam hız / 4096 birim ↔ taslağın S = 64 B / %20 pay / W = 16. Aynı baskın terim 64 B yükle (bugünkü tel 97 B), tam hızda **51 266 s ≈ 14,2 sa**; %20 payla ≈ 2,97 gün.
- Etki: 245 s ↔ 6,4 gün karşılaştırması farklı koşulların oranıdır; "bugünkü motor" ifadesi tabanı değil adayı anlatıyor.

## 2. Hatalar — kod iddiaları [kod]

**H-11 · §1 "64 baytlık sembollere 37 bayt çerçeve ekleniyor (taban yükü yaklaşık iki kat)"**
- Doğru: ürün FEC profili `0x05`, **E = 1024** (`client-core/src/runtime.rs:240`; `kama-fec-codec/src/profile.rs:86-91`). Ürün telinde kaynak birim düz DATA çerçevesidir, sembol tele çıkmaz (`runtime.rs:2089-2091`, `client-core/src/wire.rs:17`): kaynak ek yükü 33 B, FEC'siz yolla aynı. Onarım = 22 (Legacy zarf: `kama-node/src/wiring.rs:47`, `kama-noise-wire/src/legacy.rs:25`) + 12 (`wire.rs:105`) + 1024 = **1058 B** (`runtime.rs:2501`). "37 B" = noise-wire EXTENDED zarfı 26 + FEC iç başlığı 11 (`profile.rs:83` yorumu; `kama-noise-wire/src/inner.rs:77`); client-core ürün yolu bunu kullanmıyor.
- Etki [hesap; `yuk_tablosu.py · Y1-01`]: ürün yolunda FEC maliyeti "kaynakta ~2×" değil, onarım başına yükten bağımsız 1058 B: 65 B mesajda kaynağın **10,8 katı**, 300 B'da 3,2 katı, 1013 B'da 1,01 katı. 25 B/s'de tek onarım 42,3 s. (E = 64 + EXTENDED modelinde "≈ 2×" doğru: yük başına 1,91×.)

**H-12 · §4.1 "300 B mesajın FEC yolundaki tel boyu (onarım hariç) | 6 × (64 + 37) = 606 B" ve "FEC yolunda çerçeve yükü yaklaşık yarıya iner"**
- Doğru: ürün yolunda (parçalayıcı kapalı, `runtime.rs:266` `max_message_bytes: None`) 22 + 11 + 300 = **333 B**. 606 B yalnız noise-wire EXTENDED + E = 64 modelinin değeri (53 B/sembol × 6). E = 64 profili + parçalayıcı açık client-core yolunda 564 B. KTP-2'nin 331 B'ı ürüne göre **2 B (%0,6)** küçük. Onarım: bugün 1058 B, KTP-2'de 22 + 12 + 300 = 334 B [hesap; `yuk_tablosu.py · Y41-02`].
- Etki: §4.1'in FEC satırı ve "yarıya iner" cümlesi ürün yolu için geçersiz; ürün yolundaki fark onarım paketindedir.

**H-13 · §1 "FEC varsayılan olarak kapalı"**
- Doğru: ürün profilinde FEC **açık, uyarlamalı**: `KamaV1Profile::runtime_config` → `fec: Some(FecRuntimeConfig { profile_id: 0x05, manual_r: None })` (`runtime.rs:282-285`; yorum `:280-281` "parity oranını arbiter belirler, temiz yolda 0'dır"); `manual_r: None` → `FecActuationMode::Controller` (`runtime.rs:1863-1876`); ürün kurucusu `ClientTransportConfig::product` → `RuntimeConfig::default()` (`transport.rs:206-208`, `runtime.rs:300-302`) ← `kama-node/src/lib.rs:252`. Parite adedi blok kapanışında denetleyici kararıyla belirlenir (`fec_controller_latch`, `adaptive-transport/src/sender.rs:4979`).
- Etki: doğru ifade "açık; parite adedi denetleyiciye bağlı, temiz yolda 0". Tele çıkan parite oranı ölçüm konusudur (Ajan C) [çıkarım].

**H-14 · §1 "Alıcının yeri dolunca birimi teyit etmeden reddediyor. Gönderen bunu kayıp sanıp yeniden gönderiyor" (ve §4.1 "Kapasite reddi yüzünden yeniden gönderim: var")**
- Doğru: alıcı motoru kapasite reddi yapmaz; her yeni DATA PN'ini kaydeder, birimi hemen verir (`adaptive-transport/src/receiver.rs:236`). Teslim kanalı (`delivery_queue` 1024, `runtime.rs:261`) doluysa birim düşer (`push_delivery`, `runtime.rs:3010-3032`, sayaç `delivery_dropped`) **ama PN yine ACK'lenir**: sıra `on_datagram` → `admit_chunk` → koşulsuz `out.ack` gönderimi (`runtime.rs:3578-3605`). Teyitsiz düşüş yalnız iki yolda: oturum `Ready` değil (INV-ACK-DELIVER-1, `runtime.rs:3450-3451` yorumu, düşüş kolu `:3466-3469`, sayaç `not_ready_drops`) ve eş-yuvası tablosu dolu (`max_peers` 64, `dropped_new_peers`, `runtime.rs:3527-3538`). Aday 49864510'da `push_delivery` aynı (`runtime.rs:3091-3114`).
- Etki: kurulu oturumda "yer dolu" olayı yeniden gönderim israfı değil **teyitli kayıptır**: gönderen yeniden göndermez; S1 sıralaması o birimi bekler, 4096 tutulan birimi aşınca boşluğu atlar ve geç kopyayı düşürür (`transport.rs:394-402`; tavan `transport.rs:1054`). Taslağın tarif ettiği davranış yalnız NotReady ve eş-yuvası yolları için geçerli.

**H-15 · §1 "parite blok kapanışında ve ACK saatine bağlı gidiyor" — kısmen**
- Doğru: "blok kapanışında" tutuyor — ürün yolunda blok yalnız K_shape = 8 birim dolunca kapanır (`runtime.rs:2500`); kısmi blok yalnız ana döngü bittikten sonra, oturum boşaltmasında kapanır (`runtime.rs:2577-2579`); blok kimliği bütün trafiğin ortak `unit_id`'sinden türer (`runtime.rs:3617`). "ACK saatine bağlı" emisyon için tutmuyor: parite `pending_fec` kuyruğundan jeton kovasıyla (`sender.rs:5319-5329`), 1 ms'lik pace tikinde (`runtime.rs:65`, `:2548-2550`) ve DATA/kurtarma planlarından sonra (`sender.rs:4292` < `:4311`) gider; ACK beklemez. Parite adedi denetleyici kararından gelir (`sender.rs:4963-4971`); bu kararın girdilerinin ACK kaynaklı olduğu satır satır izlenmedi [çıkarım].
- Etki: yüksek RTT'de parite gecikmesinin koddaki kaynağı 8 birimlik blok dolumu (seyrek mesajda 8 mesaj beklenir) ve DATA-önce sıralamasıdır; "ACK saati" emisyon zamanlamasını yanlış tarif eder.

---

## 3. Varsayıma duyarlı (hata değil; değer varsayıma göre değişir)

**V-1 · Alt katman başlığı (genel).** Taslağın bütün tel/süre sayıları L = 0 varsayar. L = 28 / L = 42 ile [hesap]: K-3 96 B → 124 / 138 B: 25 B/s'de 4,96 / 5,52 s, 3,125 B/s'de 39,68 / 44,16 s (`K3-01`) · ses tel hızı 200 ms'de 445 / 515 B/s, 40 ms'de 1625 / 1975 B/s, başlık payı %66,3 / %70,9 (200 ms) (`K6-01`) · §3.9 eşiği 47,5 → 61,5 / 68,5 B/s, 31 B/s'de en küçük paket 3,97 / 4,42 s (`S39-01`) · §5 245 s → 353,5 s (L = 42) (`S5-02`) · A.4 38,7 → 39,6 / 40,1 s (`A4-01`).

**V-2 · K-3 "1 KB → 25 B/s'de ≈ 42 s".** Tek paket değeridir (1024 + 31 = 1055 B → 42,20 s; 1000 B ile 41,24 s). Taslağın kendi §3.9 kuralıyla 25 B/s'de (ses yok, T_ser 8 s) S = 169 → 7 parça → **49,64 s** (L = 42: 61,40 s); 1000 B ile 6 parça → 47,44 s [hesap; `iletim_dusuk_bant.py · K3-02`].

**V-3 · K-3 notu ve Ek A.1 madde 9 (spec'in "0,025 kbit/s / %80'de 1,5–2 dk" satırı).** Taslak spec'in mesajını "65 B yük (+31 B)" okur. Spec satırı "Mesaj (SESSION_DATA ~65B)" der (`specs/transport-layer-tdd.md:250`); ccr §35.6'ya göre SESSION_DATA UDP yükü = 22 B + yük. Bu okumayla tel 65 B: iletim 20,80 s, ortalama 5 × = **1,73 dk** (spec aralığının içinde), %90 11 × = **3,81 dk**, %95 14 × = 4,85 dk; L = 28: 2,48 / 5,46 dk; L = 42: 2,85 / 6,28 dk. Taslağın 96 B okumasıyla 2,56 / 5,63 dk [hesap; `K3-04`]. "Ortalamaya yakın, %90 güvenle değil" yargısı iki okumada da tutar; A.1 madde 9'daki "≈ 5–6 dk" yalnız 96 B ya da L ≥ 28 okumasında.

**V-4 · K-3 ölçütü "25 B/s %20 kayıpta p95 ≤ 3 × alt sınır (≈ 12 s)".** %95 için 2 iletim gerekir. Taslağın §3.8 güvenilir akış kuralıyla (K = 1 → R = 1) p95 ≤ **7,80 s** (L = 42: 11,16 s ≤ 16,56). Yalnız ARQ'da tek mesajın kaybı PTO ile anlaşılır: p95 = PTO + ser + RTT_y/2 ≥ **9,07 s** (srtt ≥ 5,20 s, rttvar = 0, yayılım RTT_y = 0); sınır ancak 1,5·RTT_y + 4·rttvar ≤ 2,45 s iken tutar; ilk örnek rttvar = srtt/2 ile 19,47 s [hesap; `K3-05`, `K3-06`]. (Düzeltme: ilk sürümdeki "yayılım + 4·rttvar" tek yön yayılımını atlıyordu.) **"≈ 12 s" sabit okunur ve L = 42 sayılırsa (B İSTEK-E1):** alt sınır 96 B yerine 134–140 B tel (V-1); yalnız ARQ p95 **14,10 s** (138 B, ACK 76 B, yayılım 0), d300'de 15,00 s, d1000'de 17,10 s, üçü de 12 s'nin üstünde. B'nin `fizik.py` değerleri (134 B, ACK 73 B: d300 14,54 s, d1000 16,64 s) yeniden üretildi. Kaynak + onarım 11,16 / 11,46 / 12,16 s [hesap; `K3-06`]. §3.11 ACK tavanı bütün ACK'lere uygulanırsa 25 B/s'de ACK aralığı ≥ 27,2 s (V-18), srtt/PTO buna göre büyür [çıkarım].

**V-5 · K-2 "%80 iki yönlü kayıpta 1 KB için ideal ARQ ≈ 25 iletim".** 25 = 1/(0,2 × 0,2): her alınan kopyaya tek ACK, gönderen teyit alana dek. K-2'nin ölçütü teslimdir: alıcıya ilk varış ortalaması **5** iletim. ACK alım başına 2/3/4 kez tekrarlanırsa 13,9 / 10,3 / 8,5. 8 ve 100 kbit/s'de 1 KB tek parçadır (S = 1100) [hesap; `K2-01`].

**V-6 · §9.** "%95'te ≈ 20 iletim" tek yön ortalamasıdır (silme kanalının 1/(1−p) alt sınırı da 20); iki yönlü teyitle 400; %90 teslim için 45 iletim. "FEC'le 2–10 katı": yalnız FEC çarpanı 1,25–9×; başlık dahil 200 ms demette 150 B/s'ye göre 2,54–18,30×, birincil kayıplarda (≤ %30) 2,54–4,07× [hesap; `S9-01`].

**V-7 · §5 hizmet payı (mesaj ≥ 560, dosya ≥ 140 B/s).** Kontrol boş ve ses 300 B/s varsayımıyla. Kontrol da doluysa 311,1 / 77,8; ses §3.6 tavanındaysa (457,5) 434,0 / 108,5; %30 FEC'li 200 ms ses (625) 300,0 / 75,0 B/s. Değerler tel baytıdır [hesap; `S5-01`].

**V-8 · §5 bekleme sınırı "j × (S + başlık) / pay + kayıp başına bir PTO".** Akışkan yaklaşım; kesilemeyen paket terimi (S_maks + 31)/r yok: 31 B/s'de 8,000 s (ses yok) / 3,065 s (ses etkin), 1 kB/s'de 1,131 s, 12,5 kB/s'de 0,090 s [hesap; `S5-05`]. Art arda kayıpta PTO iki katına çıktığı için "kayıp başına bir PTO" yalnız ilk denemeyi sayar.

**V-9 · §5 "31 B/s, W = 16, S = 64 B, pay %20 → ≈ 245 s".** S = 64 yalnız ses etkinken (T_ser 2 s); §9'a göre 31 B/s'de canlı ses yok, §3.9 bu durumda S = 217 verir → **640,0 s** (L = 42: 748,4 s). Kontrol da doluysa (pay 1/9) S = 64 ile 441,3 s [hesap; `S5-02`]. K-8 bu sınırı 31 B/s'de kullanır.

**V-10 · §5 israf sınırı "≤ 1,5 × p/(1 − p)".** Beklenen değer sınırıdır; tek koşuda ideal ARQ bile aşar [hesap, kesin negatif binom; `S5-04`]: n = 1 parça %10 / %20 / %30 (p = 0,1 / 0,2 / 0,3), n = 10'da %30,3 / %25,3 / %17,5, n = 30'da %13,2 / %10,2 / %7,0, n = 300'de ≤ %0,43. FEC onarımı "yeniden gönderilen bayt"a sayılırsa R formülünün kendisi sınırı aşar: K = 8, p = 0,1'de R/K = 0,25 > 0,167; K = 4'te üç kayıpta da. K-7 FEC'i ayrı sütunda saydığı için sınır yalnız ARQ baytı içinse çelişki yok.

**V-11 · §3.9 ifadeleri.** "47,5 B/s altında en kötü bekleme alt sınırı (95 B)/hız" alt sınır olarak doğru; 31 B/s'de ses yokken kuralın verdiği gerçek en kötü bekleme 8,00 s'dir (S = 217). "Daha küçüğünde 31 baytlık başlık paketin üçte birinden fazlasını yer": eşik S < 62 B (64 B'ta %32,6; L = 42 ile %53,3) [hesap; `S39-01`].

**V-12 · K-4 "kesinti sonrası ilk teslim ≤ 30 s + 2 RTT".** Serileştirme yok. PTO veri parçası taşırsa 30 + ser + RTT/2: 1 kB/s, RTT 600 ms'de **31,43 s > 31,20 s**; 25 B/s'de 38,30–39,00 s; 12,5 kB/s'de sınır içinde. Yalnız 48 B yoklama giderse (H-6) 30 + 48/r + 1,5·RTT + ser: 1 kB/s RTT 2 s'de 34,18 > 34,00 s [hesap; `K4-01`].

**V-13 · §3.2 kredi pencereleri ve bant-gecikme çarpımı.** Dosya penceresi 16 × 1100 = 17 600 B; 12,5 kB/s × 2 s'de (BDP 25 000 B) pencere/RTT = **8 800 B/s = hattın %70,4'ü**; diğer birincil hücrelerde pencere ≥ BDP [hesap; `KR-01`]. Kayıpta sıralı kredinin eksik parçada durması bu oranı düşürür [çıkarım; ölçülmedi].

**V-14 · §3.8 acil artıklık d = ⌈ln 0,1 / ln p⌉, en çok 8.** Bağımsız kayıpta P(en az biri gelir) ≥ %90 tasarımı p ≤ 0,7499'a kadar tutar; tavan sonrası p = 0,8'de **%83,2**, 0,9'da %57,0, 0,95'te %33,7. p = 0 ve p = 1'de formül tanımsız (IEEE kayan noktada p = 0 → d = 0) [hesap; `FEC-08`].

**V-15 · §3.8 ses tablosu.** Satırlar doğru (D-6) ama "≥ %95 çerçeve" için tek çözüm değil: %30'da (3, 3) aynı 2× çarpanla %95,11 verir, K bir eksik [betimsel; `FEC-04`]. Çarpan modeli onarımı kaynakla aynı boy sayar; ONARIM 12 B başlığıyla %20 / 200 ms'de 508,3 → 518,3 B/s (`K6-02`).

**V-16 · §3.8 güvenilir akış R örnekleri.** R = 3 (K = 8, %20) ve R = 5 (%30) ile tek blok çözme olasılığı %83,89 ve %83,46; kalan bloklar BLOK DURUMU turuna kalır [betimsel; `FEC-07`].

**V-17 · [kod] §3.8 "mevcut `kama-fec-codec`".** reed-solomon-simd 3.1.0 1–32 768 kaynak + 1–32 768 onarım taşır (crate README) → (4, 32) ve 255'e kadar onarım sırası matematiksel olarak sorun değil. Ama katalog yalnız K ∈ {2, 4, 8, 16}, R_max ≤ 8, E ∈ {64, 1024} (`profile.rs:54-99`, ürün `0x05` `:85-91`); E ≥ 11 ve çift (`profile.rs:29-37`); kodlayıcı esi < r_code_max ister (`codec.rs:91`). Tablodaki (3, 2), (4, 9), (4, 32) ve "sembol = parça" (6–30 B ses, 64–1100 B değişken boy) mevcut katalogda yok; 60 ms demetin 9 B yükü tek sayı.

**V-18 · §3.11 "ACK yükü ileri veri hızının %5'i".** Bütün ACK'lere uygulanırsa ACK aralığı ≥ 27,2 s (25 B/s), 0,68 s (1 kB/s), 0,054 s (12,5 kB/s); L = 42 ile 60,8 / 1,52 / 0,12 s [hesap; `AK-01`]. Cümle tavanın yalnız tekrarlanan ACK'lere mi uygulandığını açık bırakıyor.

**V-19 · §3.7 "kredi … sonraki 3 ACK'te".** Üçünün de kaybı p_dönüş³: %30'da %2,7, %80'de %51,2 (taslağın "krediyi tüketmiş görünüyorsa her ACK'te" yedeği bu durumu karşılar) [hesap; `KR-03`].

**V-20 · §4.1 "Kredi … akış başına 6 B".** Tek girdili KREDİ çerçevesi 6 + 6 = 12 B; 6 B ek girdi başınadır [hesap; `Y4-01`].

**V-21 · §3.1 "Akış numarası (u16)".** Her bas-konuş ve her dosya yeni akış, oturum kapanmıyor → numara yeniden kullanılmıyorsa oturum başına 65 536 akış. Yeniden kullanım kuralı yazılı değil [çıkarım].

**V-22 · §3.5 "taban hız … bugünkü 31 B/s".** 31 B/s, K-3 hatlarının üstünde (25 B/s'nin 1,24 katı, 3,125 B/s'nin 9,9 katı). KTP-2'de uçuş tavanı (2 paket) kuyruğu sınırlar. Bugünkü motorda 31 B/s yalnız başlangıç değil kalıcı tabandır [kod: D-K3].

**V-23 · §2 hikâye (8 kbit/s, %30, PTT + mesaj + dosya).** (4, 4) 200 ms ses 625 B/s (L = 0) = hattın %62,5'i ve §3.6 tavanının (457,5) üstünde (H-3); L = 42 ile 1045 B/s > 1000 B/s, hatta sığmaz [hesap; `K6-03`].

**V-24 · [kod] K-6 "Ses tel hızı (başlık dahil, FEC'siz)".** 925 / 667 / 408 / 305 B/s yalnız 31 B sayar; uygulama kaydının 13 B'ı dahil değil: `APP_RECORD_HEADER_BYTES` 5 (`kama-app/src/codec/limits.rs:24`) + `MEDIA_BUNDLE_PREFIX_BYTES` 8 (`:93`). PTT bugün bu kapla gider (`kama-app/src/ptt.rs:12`); taslak kaydın kalktığını yazmıyor (Ek A.1'de yok, §7 uygulama katmanı kalır). Kalırsa 40 / 60 / 120 / 200 ms → 1250 / 883 / 517 / 370 B/s (L = 42: 2300 / 1583 / 867 / 580). 200 ms FEC'li: %10 466, %20 627 (5,01 kbit/s), %30 755 B/s (6,04 kbit/s); L = 42 ile 729, 977 (7,81 kbit/s), 1175 B/s (9,40 kbit/s > 8 kbit/s). Bugünkü motor + uygulama 120 ms'de UDP 533, L = 42 ile 883 B/s (RAPOR-C §3.2 yeniden hesaplandı) [hesap; `K6-09`].

### Ölçülen itirazlar (hesap hatası değil)

Ölçüm bugünkü motorundur (taban, aday); KTP-2 ölçülmedi, taslağa aktarım [çıkarım]dır. ✱ = ham CSV'den yeniden sayıldı (`reports/ktp2/f0/c/tam/kosular.csv`; `ktp2/f0-b` dalında `reports/ktp2/f0/b/tam/kosular_b.csv.gz`, `a10_kosular.csv`).

**V-25 · [ölçülen itiraz] §3.5 "bugünkü teslim hızı örnekleyicisi ve bant filtresi aynen kullanılır" (§7: `bw` kalır).**
- Beklenti: bu iki bileşen aynen alınırsa KTP-2'nin gönderen kilidini (B-K14 / B-C9) ve hız çöküşünü (A-10) taşıması beklenir [çıkarım].
- Mekanizma [kod]: örnek = teyit edilen bayt / aralık, kaybolan bayt sayılmaz (`bw.rs:98-109`); filtre pencereli azami (`bw.rs:221-245`); hız = kazanç × BtlBw, en çok cwnd/srtt (`rate.rs:164-180`). Hat tıkalı değilken örnek ≈ pacer × (1 − p) [çıkarım; B, C]. BtlBw'nin sönmemesi için kazanç ≥ 1/(1 − p) gerekir: %10'da 1,11, %20'de 1,25, %30'da 1,43; bugünkü ProbeUp'ın 1,25(1 − p) değeri 1,125 / 1,000 / 0,875 [hesap; `sinirlar.py · BW-01`]. §3.5 kazanç kuralı yazmıyor.
- Ölçüm [ölçüldü; N = 30; taban ↔ aday]: `kilit_gönderen` r12k l30 78/120 ↔ 92/120, r1k l30 36/120 ↔ 56/120 (B)✱; arq kolu 11 hücrede 185/330 ↔ 249/330 (C)✱; hız çöküşü 12/225 ↔ 14/225, rinf hariç (B)✱. r12k_d300_l30_p65'te hat doluluğu %22,8 / %21,2: kilit darboğaz kuyruğu değil (RAPOR-B TS.3 no. 3).
- Sınırlar: (a) bugünkü sürdürmenin bir kısmı durum makinesindedir (`pending_lost` → RecoverArq, kazanç 1, ProbeUp iptali: `sender.rs:3993`, `:4076`, `:4252-4255`); §3.5 bu durumları almıyor, o parça aynen taşınmaz. (b) `kilitli` tanımı gecikmeden türer; kilitli/kilitsiz kıyası betimseldir. (c) FEC'li kollarda kilit seyrek: rs2 14/330 ↔ 25/330, rs4 32/330 ↔ 55/330 (C)✱; §3.8 güvenilir akış onarımının bu etkiyi ne kadar azaltacağı ölçülmedi.

**V-26 · [ölçülen itiraz] §3.5 / §3.10 uçuş tavanı "max(2 paket, 2 × bant tahmini × en küçük RTT)".**
- B'nin hesabı doğrulandı [hesap; `sinirlar.py · UT-01`]: 2 tam boy paket 2 × 1131 = 2262 B; RTT 2,10–2,37 s'de **1077–954 B/s** (L = 42: 1117–990; B'nin 2 × 1100 B ile değeri 1048 B/s). Gerekli: r12k_d1000_l30_p1000'de sunulan 1027 B / 285 ms = **3604 B/s** (KTP-2 telinde 3618), r12k_d1000_l0_p65'te 2556 B/s (2667). Taban, sunulanın %30'u / %42'si.
- Bugünkü taban 4 × 1200 = 4800 B (`rate.rs:18`, `:101`; cwnd = max(2 × BDP, min_cwnd), `:173`) → 2286–2025 B/s, sunulanın %63'ü / %89'u. Bu tabanla r12k_d1000_l0_p65'te 600 mesajın 298 / 286'sı sunumdan > 0,2 s sonra çıktı; r12k_d1000_l30_p1000'de ilk 5 s emisyon 1956 B/s (taban) [ölçüldü; RAPOR-B TS.3 no. 4, N = 30/kol]. KTP-2 tabanı bugünkünün 0,47 katı.
- Kapsam: taban yalnız ilk teslim örneğine kadar bağlar; sonra ikinci terim gelir, o da bugünkü 2 × BDP ile aynı biçimdedir. Rampa modeliyle (g = 2(1 − p)·rtprop/srtt; RAPOR-B T§19: l0 1,53–1,67, l30 1,04–1,18) taban farkı l0'da 1,5–1,8 RTT (3–4 s), l30'da 4,5–19 RTT (10–40 s) ek gecikme eder [çıkarım; `UT-01`]. Tavan backlog boyunca sürmez: pacer × srtt ≈ 4800 B yalnız backlog örneklerinin %3–7'sinde (RAPOR-B T§10). §3.10'da bant tahmini korunduğu için taban kesinti sonrası tek RTT bağlar [çıkarım].
- Varsayım: "paket" boyu taslakta tanımsız; değerler tam boy paket üst sınırıdır. Çerçeve birleştirmesiz 96 B'lik paketlerle 2 paket = 192 B → 91 B/s. İlk turda §3.5 taban hızı (31 B/s) daha dar bağlayabilir (V-22).

---

## 4. Doğrulananlar

Hesap [hesap]:
- **D-1** K-3: 96 B → 25 B/s'de 3,84 s; 3,125 B/s'de 30,72 s (`K3-01`).
- **D-2** K-3 notu: %80'de ortalama 5 iletim, %90 güven 11 iletim (ln 0,1 / ln 0,8 = 10,32); 5 × 30,72 s = 2,56 dk, 11 × 30,72 s = 5,63 dk — taslağın 96 B modelinde (`K3-03`, `K3-04`).
- **D-3** K-3 ölçütü: 3 × 3,84 = 11,52 ≈ 12 s (`K3-05`).
- **D-4** K-6 ses tel hızları: 925 / 666,67 / 408,33 / 305 B/s; başlık payı %83,8 (40 ms), %50,8 (200 ms) (`K6-01`).
- **D-5** K-6 "%20'de 200 ms ≈ 510 B/s ≈ 4 kbit/s": 508,3 (aynı boy) / 518,3 B/s (ONARIM 12 B) = 4,07 / 4,15 kbit/s (`K6-02`).
- **D-6** §3.8 tablo, beş satır: çarpan, çerçeve, blok hepsi tutuyor (%96,56/91,85 · %96,38/94,21 · %96,22/94,20 · %96,35/95,39 · %95,16/94,78); her satır ≥ %95 çerçeve (`FEC-01`).
- **D-7** §3.8 %50'de (4, 8): %94,34 / %92,70; (2, 4): %90,63 / %89,06 (`FEC-02`).
- **D-8** §3.8 8 + 4 RS: %10'da %99,57, %20'de %92,74 (`FEC-05`).
- **D-9** §3.8 yarı çoğaltma (8 kaynaktan 4'ü iki kez): %63,03 / %34,79 (`FEC-06`).
- **D-10** §3.8 R örnekleri: 8 × 0,25 × 1,25 = 5/2 → 3; 8 × 3/7 × 1,25 = 30/7 → 5 (`FEC-07`).
- **D-11** §3.8 d formülünün anlamı: P(d kopyanın hepsi kayıp) ≤ 0,1 olan en küçük d (`FEC-08`).
- **D-12** §3.9: (64 + 31)/2 = 47,5 B/s; 95/31 = 3,06 s (`S39-01`).
- **D-13** §5 hizmet payı aritmetiği: 700 × 4/5 = 560, 700 × 1/5 = 140 B/s (`S5-01`).
- **D-14** §5 teyitsiz veri örneği: 16 × 95 / 6,2 = 245,2 s (`S5-02`).
- **D-15** §5 israf: ideal ARQ oranı p/(1 − p), 1,5 katı 0,167 / 0,375 / 0,643 (`S5-04`).
- **D-16** §4 çerçeve başlıkları: AKIŞ 9, ONARIM 12, ACK 8 + 4n, KURTARILDI 9, KREDİ 6 + 6n, AKIŞ BİTİŞİ 7, AKIŞ İPTALİ 4, BLOK DURUMU 8, YOKLAMA 1; KTP-2 veri paketi 22 + 9 = 31 B (`Y4-01`, `Y41-01`).
- **D-17** §9: 1200 bit/s = 150 B/s; %95 tek yön kayıpta ortalama 20 iletim (`S9-01`).
- **D-18** K-2: 25 iletim, "her alınan kopyaya tek ACK, gönderen teyit alana dek" modelinde (`K2-01`; model notu V-5).
- **D-19** Ek A.4: 1200 B / 31 B/s = 38,7 s (`A4-01`).
- **D-20** §3.10 aritmetiği: 48 / 30 = 1,6 B/s (`PT-01`; içerik çelişkisi H-6). Hub zarfı 4 + 2 + 16 = 22 B (ccr §35.6 LAYER ACCOUNTING).

Kod [kod]:
- **D-K1** "Reorder oturum başına tek sıra (`transport.rs` :361-395)": doğru. `Reorder` yapısı ve `push` `transport.rs:361-412` (taslağın aralığı yapıyı ve `push`'un başını kapsar); harita `SessionHandle` anahtarlı (`:445`), çağrı `:1077-1080`; motor sırasız teslim eder (`receiver.rs:236`). Tutma tavanı `max_inflight_units` = 4096 (`transport.rs:1054`, `runtime.rs:259`); aşılınca en küçük boşluk atlanır (`transport.rs:398-402`).
- **D-K2** "veri paketi başlığı 22 + 11 = 33 B": doğru. Legacy zarf 22 (`legacy.rs:25`; ürün Legacy `kama-node/src/wiring.rs:47`) + DATA 11 (`wire.rs:17`).
- **D-K3** "31 B/s taban hız": doğru. `PRE_CONVERGE_FLOOR_BW = 31` (`sender.rs:1485`, kullanım `:4243`; adayda `:1672`). Ek: yakınsamadan sonra da `min_rate_bps = 31` kalıcı taban (`budget.rs:29`; `send_rate_final`, `sender.rs:4268`) → bugünkü motor K-3'ün 25 ve 3,125 B/s hatlarında 31 B/s'nin altına pace etmez [kod; etkisi ölçülmedi — Ajan B'nin konusu].
- **D-K4** "PTO … 30 saniyede durur": kodda doğru (`tail_recovery.rs:78` `max_backoff_pto_ns` 30 s; sözleşme `docs/transport-v1/PRODUCT-PROFILE.md:48`). Kaynak notu: `specs/transport-layer-tdd.md`'de ayrı bir "30 s PTO tavanı" satırı bulunamadı (grep); taslağın "taşıma spec'indeki normatif tavan" atfı transport-v1 sözleşmesine karşılık geliyor. Bugünkü motorun ürün yapılandırmasında (`validated_v1`) kurulum başına en çok 3 prob vardır (`max_tail_probes: 3`, `tail_recovery.rs:80`).

## 5. Bu denetimde doğrulanmayanlar

- §1'deki geçmiş ölçümler (R2A 59–66/100 ↔ 12–14/100; FS-1 868'e 0): yeniden hesaplanacak bir model değil, ölçüm kaydı; bu şeritte kaynağına gidilmedi.
- §8 süre tahminleri: hesap konusu değil.
