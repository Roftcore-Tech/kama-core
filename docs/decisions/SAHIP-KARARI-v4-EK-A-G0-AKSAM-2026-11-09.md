# SAHİP KARARI v4 — EK-A: G0 akşamı kararları (2026-11-09, 21:30)

> **Depo sürümü** (2026-11-10, sahip kararı madde 1): kişi adları rol adıyla değiştirildi (mobil sahibi ×4, DevOps sahibi ×1), iki makine yolu çıkarıldı (satır 11'deki okuma yolu deponun `reports/yol3/2026-11-09/g2/OKUMA.md` kopyasına çevrildi); başka satır değişmedi. Bayt-özdeş özgün depo dışında saklanır: sha256 `aeb73b6170e61717ee233cdfbdff5e3939320d8f62fdbc034d54c3c760a48e53`.

Bu ek `SAHIP-KARARI-v4-CC-IMZA-ve-YENIDEN-TAKVIM-2026-11-09.md` (v4) üstüne gelir; v4'ün hiçbir bölümünü iptal etmez, aşağıda adı geçen satırları değiştirir. Orkestratöre olduğu gibi verilir; sohbet görülmez, her karar gerekçesiyle burada. **Yeni iş emri açılmaz;** bugün başlatılan dört yerel şerit (M1, M2, T1, Ö) ve bulut iş emirleri (HA, HB1, HB2) metinleri değişmeden sürer; bu ekteki maddeler ilgili şeridin **mevcut** iş emrine KARAR-KAYDI olarak eklenir. Onay lafzidir.

---

## §1 Bugün ne oldu (origin'den doğrulanmış)

- `main` = 5de1ce3f → dbea1ef3 (#149). Bugün merge: #144 (hub belgeleri), #148 (karar kaydı), #145 (CLAUDE.md), #146 (corrigendum), #147 (yerel kapı), #149 (Ö-a düzenek).
- Dört yerel şerit origin'de: `yol3/m1-e2a-goster` (PR #150 açık), `yol3/m2-e1-bayrak`, `yol3/t1-seritler`, `yol3/o-goster`; teşhis ağacı `yol2/o1`.
- **G2 teşhisi planlanandan iki gün önce koştu ve bitti** (160 koşu, rc = 0; 5 hücre kesinti sonrası aynı ikiliyle yeniden). Okuma: `reports/yol3/2026-11-09/g2/OKUMA.md`, manifest sha256 37bcabe3….
- Bulut oturumları henüz açılmadı (sahip bu akşam açar). D-28 mesajları sahip gönderir (bağlamlı sürüm sahipte).

## §2 Teşhis sonucu — kabul ve düzeltmeler

**Kabul.** Seçim kuralı ön kayıtlıydı ve bağlayıcıydı; sonucu aynen kabul ediyorum:
**Gösterim seti = K1 = (c) + (d) + DrainRtprop + (e-dar); retx_first KAPALI.**
Manşet (R1, raporlanan): gönderim/gereken K0 0,55 → K2 1,02. Hız kilidini (c) açıyor. Model beklentisi (K2−c) iki yönde tutmadı: (c) PTT'yi iyileştirdi (K1 8,63 s ↔ K2−c 10,22 s), retx_first bu hücrede yararsız/zararlı (K0r 16,37 s; 10 koşunun 3'ü eksik). Bu bir ölçüm bulgusudur; model yanılmış, kural işlemiştir.

**v4'te değişen satırlar:**
- v4 Bölüm II **K4** satırı ("retx_first M2 bayrağı … ilan→retx bekleyişi retx_first ile kalkar") → **retx_first gösterim düğümünde kapalı.** İlan→retx bekleyişi sorunu açık kalır; Aralık'ta E2b/CC-2 kapsamında, ölçümle.
- v4 Bölüm III.2 "Gösterim seti (G2 seçimi; model beklentisi K2−c …)" → "Gösterim seti = K1 (2026-11-09 teşhisi)".
- v4 Bölüm IV G1–G2 satırları: teşhis bitti; **M2 PR'ı Çarşamba 11 Kasım'a çekilebilir** (Perşembe yerine). mini-Ö yine Cuma 13.

**Eksikler — M2 iş emrine KARAR-KAYDI olarak girer, mini-Ö'den önce biter:**
1. **R3 koruması K1 üzerinde koşmadı** (K0–K2 arasında koştu). Aynı iki koruma hücresi (`d1000 l0` p95; `r25` teslim@60 ve gönderim) **K1 ile** koşar, N = 10. Bayrağı kapatmak otomatik olarak daha güvenli değildir; K2 sonucu K1 kanıtı sayılmaz. Koruma K1'de kalırsa seçim kuralı yeniden işletilir (sıradaki uygun kol), DUR açılmaz.
2. **emisyon/sunulan = 4,90 açıklanır:** pay nelerden oluşuyor (yeni veri / yeniden iletim / prob / parite yok). Yüksek yeniden iletim payı varsa "hız kilidi açıldı" cümlesinin yanına **faydalı teslim edilen bayt / gönderilen bayt** oranı yazılır; sonuç cümlesi ona göre.
3. **rc = 0 ≠ aktarım tamamlandı.** Her hücre için koşu başına tamamlanma (dosya bitti mi, PTT akışı kapandı mı) ve eksik koşu sayısı ayrı sütun. K0r'deki 3 eksik koşunun nedeni bir satırla.
4. Manşette K2'nin `sw95` 9. sıra değeri 34,89 → 63,75 s yükseldi: "tuzaklı" diye dışlanan koşular (K2 3/10, K2−c 2/10) ayrıca incelenir — gerçek kullanıcı bunları da yaşar; dışlama yalnız R1 medyanı içindir, rapor ikisini de gösterir.
5. Uyarı korunur: r25'te (d) hiç devreye girmiyor; manşette `probe_up` 0. İkisi CC-1 girdisi olarak kaydedilir.

**Etiket değişmez:** "gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel". K1 deneysel gösterim profilidir; ürün çözümü ya da quinn'e karşı üstünlük iddiası değildir ve hiçbir raporda öyle yazılmaz.

## §3 T1 — oynatma sözleşmesi düzeltilmeden mobil sahibine gitmez

`docs/design/T1-SERITLER.md` Ek A (v1) ile mobil sahibine gidecek metin arasında çelişki var:
- Madde 4 "boşluk, atlama ya da yinelenme yok" ile madde 9 "`Channel(256)` + `trySend`; dolarsa say ve logla" **çelişiyor:** `trySend` reddedilirse demet düşer; `PttClosed` düşerse Kotlin akışın bittiğini öğrenemez. Taşımada kapattığımız kaybı Rust–Kotlin sınırında yeniden üretmek kabul değil.

**Karar (T1 iş emrine KARAR-KAYDI):**
1. Callback bloklamaz (madde 9'un bu kısmı kalır). Reddedilen olayın sahibi belli kalır: **sınırlı bellekle yeniden sunum** (Rust tarafında akış başına kuyruk; dolunca en eski demet düşer ve `PttGap`/sayaç üretilir) **ya da açık akış sonlandırma** (`PttClosed{reason: overflow}`); hangisi seçildiyse sözleşme ona göre yazılır. "Logla geç" yok. `PttOpened`/`PttClosed` yaşam döngüsü olayları hiçbir koşulda düşmez.
2. Aynı süreçte kuyruk taşıran küçük test **şimdi** yazılır (T1 testleri arasına): taşmada davranış 1'dekiyle birebir.
3. Madde 4'ün "kayıp yok" iddiası gerçek davranışla uyumlu yazılır.
4. Madde 2/6: **"ilk demet varışı ≤ 3 s" bağlayıcı gösterim ölçütüdür; "ilk duyulabilir ses" (varış + T) ayrıca ölçülür ve raporlanır.** Tampon T kabul hedefini sessizce değiştirmez; dinleyicinin duyduğu gecikme ≤ 3 s + T olarak **etiketlenir**.
5. Madde 5 (geç demette bekleme, gecikme birikimi) açık gösterim tercihi olarak yazılır; "kesintisiz canlı PTT çözüldü" denmez.
6. Oturum değişiminde eski oynatma durumunun kapatılması ve geç olayların elenmesi, `seq` beklenmeden mevcut (peerId, streamId) yaşam döngüsünde tanımlanır; kullanıcıya eski oturum demeti yansımaz.
7. Madde 3: 20 ms tick tek başına gecikme garantisi değildir; callback + tur süresi + Kotlin tüketimi mini-Ö'de ölçülür, sözleşmeye "ölçülen" olarak girer.
Düzeltilmiş v2 orkestratör raporuyla gelir; sahip onaylayınca mobil sahibine gider.

## §4 Derleme kuralı

Paylaşılan `CARGO_TARGET_DIR` worktree'ler arasında **kullanılmaz** (önceki kural geçerli: worktree başına ayrı target). Ortak cargo kilidi (`scripts/ci/cargo-lock.sh`) kalır. `local-gate.sh --ref` kipinin paylaşımlı önbelleği bu kuralın tek istisnasıdır (yalnız kapı koşusu, sonunda silinen worktree).

## §5 Sahibin cevapları (bekleyen kararlar)

- **MERGE #150:** yerel kapı ya da hosted CI yeşil ve PR yalnız E2a gösterim alt kümesi + PRODUCT-PROFILE satırı ise → **MERGE #150**. Raporda kapı durumu yoksa önce tek satır kapı durumu, sonra merge.
- **MERGE #151:** yalnız `reports/`; sahip inceledi (§9) → **MERGE #151**.
- **A yolu 2. şartı ("iki telefon arasında PTT bir kez koşmuş"):** tek telefonda iki düğümle PTT koşması **bileşen testidir, şartı karşılamaz**. Şart şöyle okunur: **iki ayrı süreçte** (aynı makine ya da iki telefon) PTT bir kez koşmuş. İki telefon hub olmadan oturum kuramadığı için iki telefon şartı A yolu kararına kadar fiilen hub bağına bağlıdır; bu bir döngü değil, A yolunun ön şartıdır. Karar G5 (Pzt 16) 12:00'de aynen: üç şart birden.
- **Hub mimarisi:** Hub'da hop-ARQ (özgün tasarım; `docs/seams/KM-SEAM-TC-001.md:78`, `specs/routing-rendezvous-architecture.md:933-940`; PoC-KAPSAM-001:90 ile M4'e ertelenmiş) **18 Kasım'a kadar açılmaz.** 19 Kasım'dan sonra sahibin kendi şeridi olarak açılır: dar deney (iki zincirli `kama-node` rölesi, iki hopta netem; E2E-ARQ ↔ hop-ARQ: teslim p95, teslim başına bayt) + ADR (hop ACK çerçevesi sıfır-bayt kuralıyla, transport kabulü / custody / makbuz ayrımı, üç durum ekseni, H4/H5/H7/H8). Karar kapısı 28 Kasım. **Bu ekten önce önerilen "§8 A3 sinyal anlamları" karar kaydı şimdi girmez;** HB1/HB2 ilk raporlarını verdikten sonra ayrı PR. Dondurmaya kadar motorun ACK yoluna dokunan hiçbir hub işi yok.
- **DTLS→Noise, sinyalleşme:** 19 Kasım'dan önce açılmaz; ilk adım ölçüm (DevOps sahibi, 1 gün). Gösterimle ilgisi yok.

## §6 Sonraki ortak çıktı (v4 III.2'ye ek; mini-Ö'nün tanımı)

mini-Ö (v4 takvimindeki yeri; tarih orkestratörün takvimine göre), **tek entegrasyon SHA'sında (`yol2/goster`), iki ayrı süreçte** koşar: dosya (150 KB) sürerken TEXT + PTT. Ölçülen ve raporlanan:
1. basıştan ilk demet varışına süre (bağlayıcı ≤ 3 s) ve ilk duyulabilir sese süre (raporlanan);
2. oynatma duraklamaları ve biriken gecikme;
3. dosya sürerken TEXT teslim süresi (≤ 10 s);
4. dosya bitişi (≤ 30 s);
5. olay/demet kaybı ve kuyrukların en yüksek doluluğu (T1 sayaçları).
Aynı süreçte iki düğüm bileşen testidir, kabul değildir. Rapor yalnız gerçekten engel olan maddeleri "engel" diye yazar; rutin uygulama tercihleri için onay turu açılmaz.

## §7 Ne yapılmaz

- Yeni plan/takvim belgesi yok; v4 takvimi geçerli (M2 PR bir gün öne alınabilir).
- K1 "transport düzeldi" ya da "quinn'den iyi" diye sunulmaz; quinn kıyası Aralık G-T'de (FEC + CC-2 ile).
- retx_first gösterime geri alınmaz; ilan→retx bekleyişi Aralık'ta ölçümle.
- Mobil sahibine düzeltilmemiş sözleşme gitmez.
- Hop-ARQ, mailbox, DTLS, sinyalleşme: 18 Kasım'dan önce kod yok.

## §9 PR #151 incelemesi ve quinn A paketi (sahip, 23:40)

Sahip #151'i inceledi; K1/K0/K1a ayrıştırma sayıları ham veriden birebir yeniden üretildi (`ayristirma_util.py`, üç hücre). Kararlar:

1. **B2(b) cevabı** (ham veriden, K1 10 koşu): dosyanın ilk 15 kB'si medyan 5,2 s (K0 6,4; K1a 5,9); Startup pace'i ısınma TEXT'lerinden öğrenilen uygulamayla-sınırlı bw ≈ 325 B/s ile ≈ 937 B/s'den başlıyor; 5–6 s'de 25 kB/s taşma, netem 1000 paketlik kuyruğu, srtt 0,9–1,4 s; RecoverArq'ta cwnd/şişmiş-srtt ile 8,5–9 kB/s boşaltma; 13 s sonra ProbeUp 15 kB/s. Fazlar içinde tel ≥ pacer hedefi. BULGULAR §7'ye bu bulgu [ölçüldü] olarak yazılır; "tel hedefin altında" cümlesi medyan yanılsaması olarak düzeltilir.
2. **A-6 eklenir** (`yol3/q2-quinn-pacer-prob` dalı, bayrak `a6_iw_floor`, varsayılan kapalı): Startup pace tabanı = 10·MTU / srtt; Startup'ta pace düşmez; ısınmadan öğrenilen uygulamayla-sınırlı bw Startup'ı tohumlamaz. Kaynak: quinn `connection/pacing.rs:92-95` + cwnd = ilk pencere (`congestion/bbr/mod.rs:84,95`). Rejim kapısı A-2 ile aynı (ölçülen bw < 1 kB/s ise devre dışı). Yerler: `sender.rs:2351-2361` startup floor / `grounded_startup_rate`.
3. **7 kollu ölçüm** kolları: K1 · K1+A (A-1…A-6) · K1+A−6 · K1+şeritler · K1+A+şeritler · Q-bbr-rfc · Q-cubic; gösterim analoğu hücresi + `a_r12k_d300_l30_dosyali` + `b_r25_d300_l20_p65`. Ölçü: dosya bitiş, PTT p95 (ilk demet varışı), faydalı debi, retx payı, t(%10).
4. **netem kuyruğu:** ölçüm hücrelerine ve gösterim `tc`'sine `limit` ≈ 2×BDP (hesap: rate × RTT / MTU × 2, hücre başına yazılır); mevcut 1000 paket ayrı etiketle. Derin kuyruk Startup taşmasını Kama'da (Drain fazı yok) quinn'den fazla cezalandırıyor.
5. **B5 kabul:** quinn pacer'ı pencere/srtt ile; BBR `pacing_rate` yalnız metrik. Sahibin karşılaştırma belgesi buna göre düzeltildi ve `docs/design/QUINN-FARKI-KOD-KARSILASTIRMA-2026-11-09.md` olarak #151'e ya da ayrı docs PR'ına girer (sahip dosyayı verir).
6. Sahibin dünkü "kova kuantizasyonu dosya debisini düşürüyor" tahmini ham veriyle desteklenmedi, geri çekildi; A-3 parite/retx için kalır.

## §10 Bağımsız inceleme sonrası düzeltmeler (sahip, 2026-11-10 01:50) — §9'un üstüne gelir

1. §9.1 cümlesi: "dosyanın ilk 15 kB'si 5,2 s" → "göndericinin `txb` sayacı ilk 15 kB için medyan 5,2 s; başlangıç gönderimi yavaş (ilk 3 s'de ≈ 2 kB; ilk büyük gönderim ≈ 1,3 s); Startup pace ≈ 930 B/s (ısınma bw ≈ 325 B/s × 2,885)". Alıcı tarafı ölçülmedi (aynada alıcı dosya ilerlemesi yok); A-6 aç/kapa kolu katkıyı ayıracak.
2. §9.2 A-6 kapısı değişti: "ölçülen bw < 1 kB/s ise devre dışı" **kalkar** (kendi vakasını dışlıyordu). Yeni kapı: IW/srtt tabanı yalnız bant tahmini henüz uygulamayla-sınırlı-olmayan bir tam tur görmemişken uygulanır (`bw.rs` app-limited / ilk tam tur bayrağı); gerçek düşük kapasite ilk tam turda ölçülünce taban kalkar. "Startup'ta pace düşmez" yalnız bw terimine uygulanır; cwnd/uçuş tavanı (`rate.rs:143-150`) aynen bağlar. A-6 deneydir, varsayılan kapalı; ürün için doğrulanmış sayılmaz. Deney kaydı zorunlu: ısınma sırasında tam turun zaten tamamlanıp tamamlanmadığı, TEXT→dosya geçişinde tabanın gerçekten etkinleştiği an ve hangi olayla kalktığı aynaya yazılır; etkinleşmeyen bayrak ölçülmüş sayılmaz.
3. §9.4 netem: mevcut 1000 paket profili **korunur**; küçük kuyruk ayrı kol (`q_limit` etiketli); gösterim profili sonuç alınmadan değişmez. `limit` hücre başına paket cinsinden yazılır; bir paketin altına düşen hücrede kol koşulmaz.
4. §9.3 ölçüler ayrılır: ilk demet varışı · bütün demetler p95 · ilk duyulabilir ses. "PTT p95 (ilk demet varışı)" ifadesi kalkar.
5. "Orkestratör kodladı → onaylı" anlamına gelen her cümle kalkar. A-1…A-6 bayrak arkasında deneydir; onay = ölçümü yapmak değil, KAPI-MANİFESTİ'nde önceden yazılmış kabul koşullarını sağlamak.
6. **MERGE #150**: hosted CI 11/11 yeşil (run 42642482245); D-32 (i) ile orkestratör birleştirir, diff incelemesi yine yapılır. **MERGE #151** (yalnız reports/).

**İş emri yamaları (orkestratör isemri-v3 iş emri dosyalarını günceller, sahip güncel dosyaları bulut oturumlarına verir):**
- Üç iş emrinde satır 18: "(özel; public yapılmaz)" → "(HERKESE AÇIK — PR, rapor ve belgelere kişi adı, makine yolu, bakiye, müşteri adı yazılmaz)". Depo 25 Ekim'den beri açık.
- **HB1 KK-H36:** T3 yalnız o denemeye/oturum nesline ait **doğrulanmış RESPONSE kabulüyle** durur: `peer_ready(peer, session_handle)` kama-node `note_up`'tan; eski oturumun Ready'si yeni denemeyi kapatmaz. "İlk giden oturum datagramı T3'ü durdurur" yedeği **kalkar** (StoreResp OK ≠ doğrulanmış yanıt). `on_session_traffic` yalnız sessizlik ölçümü için kalır.
- **HB2 KK-H52:** "aç → önbelleğe koy → ham baytı yerel UDP'den yeniden geçir" bu hâliyle **NO-GO**. Ölçüt: açılmış paketin sahipliği tüketici kabulüne kadar korunur; önbellek taşması, zaman aşımı ve yerel aktarım kaybı açıkça ele alınır ("kabul edildi ama teslim edilmedi" yeniden üretilemez). **Çar 11 öğlene dek ≤ 1 sayfa tasarım notu**, iki seçenek: (a) tek-açma önbelleği kalır ama yerel UDP yerine süreç-içi kanal + kaybı sayan `hub_stash_miss` sayacı, dumanda 0 zorunlu; (b) `runtime.rs`'e dar, eklemeli hub alım yolu (hub kipinde görev datagramı kanaldan alır) — motor dosyası değişikliği olduğundan **Paz 15 20:00'den önce** main'de, sahip MERGE. İki seçenekte de değişmez şart: **açılmış kayıt tüketici kabulüne kadar sahipliğini korur; kanal dolması, tüketici durması ve oturum kapanması yollarında sessizce kaybolmaz.** (a)'daki `hub_stash_miss = 0` tek dumanda görülmesi bu şartı kanıtlamaz; taşma, duran tüketici ve kapanan oturum vakaları ayrı birim testleriyle gösterilir. Bu şartı sağlayan seçenek seçilir; sahip Çarşamba onaylar. "Yalnız + satır" şartı kayıplı ara katmanı haklı çıkarmaz. Yapılandırma, hub soketi, bind/`BindSigner` wiring, duman düzeneği ve A5-min bu karardan bağımsız, **hemen başlar**.
- **HB2 KK-H54:** oversize hata üretilmeden yakalanır: hub kipinde seam limitleri (`ProtectionLimits.max_plaintext_bytes`) 1280 − sabit yük olarak verilir; endpoint denetimi yedek + sayaç.
- **T1 (oynatma sözleşmesi, §3'e ek):** UniFFI yüzeyi değişmez → `PttGap` / `PttClosed{reason}` eklenmez. Taşma Kotlin'de çözülür (kuyruğun sahibi Kotlin): Demet kuyruğu zaman tabanlı sınırlı (256 × bundleMs ≈ 51 s). **Politika:** taşmada yerel akış sonlandırılır (tüketici geçici yavaşlamış olabilir; bu bir ölüm teşhisi değil, açık politika): Kotlin akışı kendi ürettiği kapanışla bitirir, sayar; sonlandırılmış akışa ait geç `PttBundle` olayları (peerId, streamId) ile elenir. **Kapanış durumunun korunması:** `PttOpened`/`PttClosed` demet kuyruğuna girmez, akış başına ayrı durum alanında tutulur; kuyruk dolu olsa da kapanış kaydedilir ve tüketici yeniden çalışınca görünür. Sessiz kayıp yok. Rust tarafı değişmez. Test: Kotlin'de kuyruk taşırma + geç demet eleme; Rust'ta demetlerin planlanandan **erken verilmediği** (geç teslim, madde 5 gereği, mümkün kalır).

## §8 Bu ekin uygulanması

Orkestratör: §2 maddelerini M2, §3'ü T1, §4'ü bütün şeritlerin iş emrine, §9'u (§10 düzeltmeleriyle) q2/Ö iş emirlerine KARAR-KAYDI olarak ekler; §10'daki iş emri yamalarını isemri-v3 dosyalarına işleyip sahibe 'isemri-v3 güncellendi: HA/HB1/HB2 <sha256>' satırı verir; §5'teki MERGE #150 koşulunu kontrol edip raporlar; §6'yı Ö iş emrine mini-Ö tanımı olarak işler. Hiçbir ajan durdurulmaz, yeniden başlatılmaz. Bu ek için ayrı rapor istenmez; G1 raporunda tek satır: "EK-A işlendi: M2 (5), T1 (7), Ö (1), derleme kuralı".
