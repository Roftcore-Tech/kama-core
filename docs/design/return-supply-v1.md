# return-supply-v1 — Dönüş-Arz Sözleşmesi (bacak-1+2) + P5-GATE kabul-kriteri [TASLAK r3 — ONAY BEKLİYOR]

> **Statü:** TASARIM-NOTU (D0-tarzı). r2 → r3: Abkarada revize-talimatı (iki review-turu,
> CONDITIONAL GO) — PATCH-1 (K_EPISODE'lu robust-baseline dedektörü), PATCH-1b (K-4 üç-katman),
> PATCH-2 (K-1a/K-1b ayrımı), PATCH-3 (K-5 iki-R-kolu), PATCH-4 (dal-B terminolojisi) + üç
> açık-sorunun kararı işlendi. **Zorunlu-disiplin: iddia değil satır-satır aritmetik** (§1a.1-1a.2
> tüm vektörler × tüm CAL-kolları; hesapsız-fix tuzağının üçüncü tekrarına yapısal engel).
> Onaydan sonra bacak-1+2 kodu (ultracode); kapsam-sınırı MUTLAK: receiver.rs + RTRAJ-bench-kanalı
> + testler dışına taşma = İHLAL. Bacak-3 ayrılığı §5; dal-B'de paket GENİŞLETİLMEZ.
>
> r2-mirası (korunan): akış-türevli epizot-fikri, birleşik emisyon-saati, R=toplam-kredi,
> üç-katmanlı kabul, T_SPACING dürüst-etiketi, kapsam-kilidi. r1→r2 tarihi git-geçmişinde.
>
> **r3-İÇ-DOĞRULAMA (wf_1321a97d, 3 bağımsız verifier, 13 bulgu / 2 HIGH — hepsi işlendi):**
> HIGH: "tetikleyen-gap yazılmaz" kuralı kalıcı-rejim-geçişinde LIVELOCK üretiyordu ve kendi
> DV-3-satırımla çelişiyordu → **simetrik CLAMP'Lİ-YAZIM** kuralı (geometrik yakınsama, sınırlı
> geçiş-epizotu). MED'ler: kota-yanış şartı (since_last_ack>0 — ölü-kanalda kredi korunur,
> count=0-deliğinin pratik zararını sınırlar); çift-mikro-gap modu (V-5 + K=4-üstünlüğü);
> 12-koşu-tanımı pinlendi (seçilen-R'nin 6+6'sı); kesin-binom değerleri; tx_ratio pin;
> k=5 kompakt-tablolar; V-2/V-3 başlık-dizisi tamamlandı; yuvarlamalar.

## 0. Problem-cümlesi

Dönüş-yolu SACK-arzının tek şans-bağımsız tabanı statik-önsel cadence (8.39s); ilk-SACK-muafiyeti
tek-atımlık ve EMİSYONLA tükeniyor. %80 dönüş-kayıplı p5'te koşu-kaderi tek 65B SACK'in
piyangosuna iniyor (kafes-kanıtı §0.23); kopma-sonrası yeniden-başlamada (DV-2) aynı mekanizma
muafiyetsiz + ağırlaştırıcılı. Bu not arz-tabanını piyango-bağımsız hâle getirir.

## 1. Bacak-1a — SESSİZLİK-EPİZODU tespiti [PATCH-1: robust-baseline + K_EPISODE]

**r2-kusuru (iki-parçalı, Abkarada ikinci-review'u):** (i) tek-`last_gap` fazla hassas —
burst/batch-içi mikro-gap sonrası normal-gap sahte-epizot üretir; (ii) K_TAIL=2 ORANI kendisi
yetersiz — normal gap-varyansı (2-3×) ile gerçek-kopma ayrılamaz. K_TAIL yeniden-kullanımı
SEMANTİK-HATAYDI: tail-emisyon-ritmi ≠ epizot-tespiti; farklı işler farklı sabit.

**Sözleşme (r3):** Varış-anında, ritim-penceresi GÜNCELLENMEDEN ÖNCE değerlendirilir:
```
rhythm_baseline:
    sample_count = 0   → baseline yok (yalnız oturum-ilk özel-durumu aktif)
    sample_count < k   → baseline = max(gözlenen gap'ler)        # muhafazakâr başlangıç
    sample_count ≥ k   → baseline = median(son k gap)
new_episode = !have_recv
            ∨ (baseline > 0 ∧ current_gap > K_EPISODE × baseline)
```
- **Sınır-semantiği PİNLİ: kesin `>`** (tam-sınırda epizot AÇILMAZ — muhafazakâr taraf:
  sahte-kota riski minimize; K_EPISODE=3 kolunu vektör-1 tam-sınırda geçirir → kol ölçülmeden
  elenmez, kırılganlığı beyanlıdır: gerçek jitter'da 60ms+ε epizot açardı → K=4 default-öneri).
- **K_EPISODE = YENİ ayrı [CAL {3, 4}]** — boyutsuz kanal-türevli çarpan (ZM'nin yasakladığı
  sınıf mutlak-ZAMAN sabitleridir; boyutsuz oran izinli — beyan). K_TAIL YENİDEN-KULLANILMAZ.
- **k (pencere-boyu) [CAL {3, 5}], öneri k=3** — küçük ring-buffer (k gap); tek straggler/
  mikro-gap median'ı bozamaz (robust). k=5 kolu daha muhafazakâr (max-fazı uzar) — ölçümle.
- **episode_abs_floor EKLENMEZ** (talimat): K_EPISODE tüm vektör+rejimleri sağlıyor (aşağıda);
  ölçüm küçük-gap sızıntısı gösterirse floor T_SPACING'ten türetilir, yeni-süre-sabiti olmaz.
- **Pencere-güncelleme kuralı [r3-doğrulama fix'i — SİMETRİK CLAMP'Lİ-YAZIM]:** her gap
  (dup dahil, tetikleyen dahil) pencereye `clamp(gap, baseline/K_EPISODE, K_EPISODE×baseline)`
  olarak yazılır (baseline mevcutken; count=0'da ham). Tek kuralla üç özellik: (i) zehirlenme-
  koruması — izole kopma-gap'i en çok K×baseline olarak girer, median onu da yutar; (ii)
  LIVELOCK yapısal ölür — kalıcı rejim-geçişinde baseline her epizotta ×K büyür → yakınsama
  ~⌈log_K(yeni/eski)⌉ epizotta (verifier-HIGH: eski "tetikleyen-yazılmaz" kuralı pencereyi
  donduruyordu → epizot-per-varış + kalıcı-kota); (iii) mikro-gap tabanı — batch-gap'leri
  baseline/K'dan küçük giremez (çift-mikro-gap sahte-epizot modu K=4'te tamamen ölür,
  K=3'te ≤3-epizotla sınırlı-geometrik — V-5). Epizot-sonrası pencere KORUNUR (sıfırlanmaz).
- **Beyanlı-sınırlar:** (a) rejim-geçişi: ~⌈log_K(oran)⌉+1 meşru geçiş-epizodu (DV-3 400k→3k,
  K=4: ~4; p5-ani-throttle 25ms→8.6s: ~5-6) — zararsız: kota RESET (birikim yok), T_SPACING
  aralık-korur, tx_ratio-KAPISI sınırlar; episode_count→RAPOR kararının (PATCH-1b) ikinci
  gerekçesi. (b) count=0-deliği: varış-2 gap'i sınıflandırılamaz (baseline yok) ve HAM yazılır —
  "ilk-paket→kopma→dönüş" deseninde kopma-süresi max-fazı baseline'ını şişirir → dedektör
  ≤k-varış kör + dönüşe epizot verilmez; pratik zarar KOTA-YANIŞ-ŞARTIYLA sınırlanır (§2:
  ölü-kanal sessizliğinde kredi yanmaz → oturum-kotası dönüşte sağ) — beyanlı, birim-test
  senaryosu §9'da. (c) **sürüklenme-analizi [Abkarada-doğrulaması, GO-öncesi]:** izole kopmada
  yazım Kg tek-örnek → med=g DEĞİŞMEZ → sonraki kopma eşiği K×g, tetikler ✓; sürüklenme-adımı
  ancak pencereye ⌈k/2⌉ kopma-yazımı sığarsa (kopmalar-arası ≤1 normal varış): [Kg,g,Kg] med=Kg
  → üçüncü-kopma eşiği K²g (0.8s·16=12.8s>8s kaçırır); araya ≥2 normal varış → [Kg,g,g] med=g
  ÖZ-ONARIM ✓. Kayıtlı DV-2 profillerinde YOK (p6 2s-kesinti/8s-temiz: yüzlerce normal-varış;
  8s-kopuk/2s-pencere: 2-3 normal + 1 kopma-yazımı → med=g). Tek uç: KOPMA-YOĞUN desen
  (pencere-başına ≤1 varış, %90-kopuk sınıfı) — orada gap'ler ritmin kendisi; "ritim-öğrenme"
  kota-per-varış spam'ine karşı DOĞRU davranış (r1-hatasının anti-tezi); clamp-üst-sınırı
  değişmez (tırmanışı geometrik sınırlayan o). DV-2 AÇIK-RİSK listesine (§9) girer.
- Oturum-ilk (!have_recv) → epizot; varış-2'de sample_count=0 → baseline-yok → kontrol atlanır.

### 1a.1 Adversarial vektörler — satır-satır (k=3; her CAL-kolu; semantik `>`; kota YENİLEMEMELİ)

Gösterim: her varışta `current_gap → baseline(kaynak) → K=3-eşiği / K=4-eşiği → hüküm`.
Pencere değerlendirme-SONRASI güncellenir.

**V-1 `20,20,60ms`** (burst-sonu normal-gap — r2'yi kıran vektör):
| varış | current | baseline | 3×b | 4×b | K=3 | K=4 |
|---|---|---|---|---|---|---|
| 2 | 20 | yok (count=0) | — | — | yok | yok |
| 3 | 20 | max[20]=20 | 60 | 80 | 20>60 F | 20>80 F |
| 4 | 60 | max[20,20]=20 | 60 | 80 | **60>60 F (tam-sınır)** | 60>80 F |
→ epizot YOK / kota yenilenmedi ✓✓ (K=3 tam-sınır beyanı yukarıda).

**V-2 `25,1,25,25ms`** (socket-batching mikro-gap'i; clamp'li-yazım: 1 → 25/K olarak girer):
| varış | current | baseline | 3×b | 4×b | K=3 | K=4 | yazılan |
|---|---|---|---|---|---|---|---|
| 2 | 25 | yok | — | — | yok | yok | 25 (ham) |
| 3 | 1 | max[25]=25 | 75 | 100 | F | F | clamp→8.3 (K=3) / 6.25 (K=4) |
| 4 | 25 | max[25,8.3]=25 | 75 | 100 | F | F | 25 |
| 5 | 25 | med[25,8.3,25]=25 | 75 | 100 | F | F | 25 |
→ YOK ✓✓ (baseline-değerleri clamp'ten etkilenmez — hükümler r3-ilk-tablosuyla özdeş).

**V-5 `25,25,1,1,25,25,25ms`** (çift-mikro-gap batch — r3-doğrulama vektörü; K-kolu ayrımı):
Mikro-gap'ler clamp-tabanına (baseline/K) yazılır → med ≥ taban → sonraki normal-gap eşiği
K×(baseline/K) = baseline = TAM-SINIR. Hüküm tamsayı-ns bölme-kalıntısına iner: **K=4** →
25ms/4 = 6.25ms TAM → eşik 25.000000 → 25 > 25 F → **sahte-epizot SIFIR ✓**. **K=3** →
25/3 = 8.333ms kesmeli → eşik 24.999999ms → 25 > eşik T → **~2 sınırlı sahte-epizot**
(yazım ceil'e kırpılır → geometrik-yakınsar; kota-tavanlı, T_SPACING-aralıklı) → beyanlı-sınır,
K=4-default-önerisinin ikinci gerekçesi. Birim-test iki kolu da tamsayı-ns semantiğiyle pinler.

**V-3 `25,50,25,50,25ms`** (bimodal jitter):
| varış | current | baseline | 3×b | 4×b | K=3 | K=4 |
|---|---|---|---|---|---|---|
| 2 | 25 | yok | — | — | yok | yok |
| 3 | 50 | max[25]=25 | 75 | 100 | F | F |
| 4 | 25 | max[25,50]=50 | 150 | 200 | F | F |
| 5 | 50 | med[25,50,25]=25 | 75 | 100 | F | F |
| 6 | 25 | med[50,25,50]=50 | 150 | 200 | F | F |
→ YOK ✓✓ (kararlı-durum medyanı iki modu da tolere eder).

**V-4 `800ms, 1.7s, 800ms`** (yavaş-kanal doğal-varyansı — ilk-talimat-fix'ini kıran vektör):
| varış | current | baseline | 3×b | 4×b | K=3 | K=4 |
|---|---|---|---|---|---|---|
| 2 | 800 | yok | — | — | yok | yok |
| 3 | 1700 | max[800]=800 | 2400 | 3200 | 1700>2400 F | F |
| 4 | 800 | max[800,1700]=1700 | 5100 | 6800 | F | F |
→ YOK ✓✓. (K_TAIL=2 oranı burada 1700 ≥ 1600 ile epizot açardı — K_EPISODE'un varlık-sebebi.)

**k=5 kolu (satır-satır — talimat: her CAL-kolu):** dört vektörde count<5 → tamamen max-fazı;
k=3'ten farklılaşan TEK hücreler: V-2 varış-5 baseline max[25,8.3,25]=25 (aynı-değer, F aynen);
V-3 varış-5 baseline max[25,50,25]=50 → eşik 150/200, 50 F ✓; varış-6 baseline max[25,50,25,50]=50
→ 25 F ✓. V-1/V-4 zaten max-fazındaydı → tablolar BİREBİR aynı. Tüm hükümler F ✓ — k-kolu
vektör-güvenli, ölçüm-ayrımı kararlı-durum medyanında.

### 1a.2 r2-rejim-doğrulamaları K_EPISODE ile (dört aritmetik KORUNDU + DV-3 eki)

| rejim | gap/baseline | K=3-eşiği | K=4-eşiği | hüküm |
|---|---|---|---|---|
| temiz-0.25k (sabit-gap 9.9-12.7s) | ≈9.9-12.7s | 29.7-38.1s | 39.6-50.8s | current≈baseline → epizot YOK ✓ |
| p5-açlık damlaması (8.6s) | 8.6s | 25.8s | 34.4s | 8.6 F ✓; tek-forward-kayıp atlaması 17.2 F ✓; çift-kayıp 25.8: K=3 **tam-sınır F** ✓, K=4 F ✓ (forward %5 → çift-kayıp p=0.0025, nadir) |
| DV-2 p6 2s-kesinti (pencere-gap ≈25ms @100kbit) | 25ms | 75ms | 100ms | 2000ms > eşik ✓✓ TETİKLER |
| DV-2 8s-kopma (3kbps pencere-gap ≈0.8s) | 0.8s | 2.4s | 3.2s | 8000ms > eşik ✓✓ TETİKLER |
| E-FULL-sonrası doygun-simetrik kanal | gap/eşik = 1/K_EPISODE ≤ 1/3 | <1 | <1 | bant-BAĞIMSIZ epizot-per-varış YAPISAL ölü ✓ |
| DV-3 rejim-geçişi (400k→3k: 25ms-pencere → 0.8s-gap) | eski-baseline 25ms | 75ms | 100ms | clamp'li-yazımla geometrik yakınsama: K=4'te yazımlar 100→100→400 (med 25→100→400) → ~4 geçiş-epizodu, sonra 800 ≤ 4×400 F ✓ oturur (MEŞRU-sınıf, beyanlı; kota-reset+T_SPACING+tx-KAPI sınırlar) |

**Muafiyet (r2'den aynen):** epizodun İLK emisyonu gecikmesiz; "ilk emisyon" HERHANGİ yoldan
(freq dahil — birleşik emisyon-saati); bitişik ikinci-kopya atılmaz.

## 2. Bacak-1b — Epizot-başı SACK-TEKRAR kotası (R) [r2'den; PATCH-1b bağları güncel]

Yeni-epizot `kota = R` kurar (**R = epizot-başı TOPLAM emisyon-kredisi, muaf-ilk-emisyon
DAHİL**). Kota > 0 iken timer-yolu aralığı `T_SPACING`; HER SACK-emisyonu (freq/timer/tail)
kotadan 1 düşer VE paylaşık emisyon-saatini (`last_emit_any`) ilerletir; emisyonlar-arası ≥
T_SPACING kota-modunda da korunur. Kota 0 → normal rejime dönüş (bacak-2). Epizot-yenilenmesi
kotayı R'ye RESET'ler (birikim YOK → geçiş-epizotları kredi-şişiremez, §1a beyanlı-sınır).
**KOTA-YANIŞ ŞARTI [r3-doğrulama]: kredi yalnız `since_last_ack > 0` iken yanar** — ack'lenmemiş
yeni-bilgi yokken (ölü-kanal sessizliği) timer'ın cadence-tekrarları kotadan DÜŞMEZ → kopma
boyunca kredi KORUNUR, dönüşte sağ (count=0-deliğinin pratik zararını sınırlar, §1a-b beyanı;
bütçe-tutarlı: ölü-havaya kota harcanmaz).

- **T_SPACING = SACK_WIRE / RETURN_BW_PRIOR ≈ 65/31 ≈ 2.1s — STATİK-ÖNSEL-TÜREVİ (dürüst
  etiket);** E-FULL'de ölçülen return_bw'ye bağlanır (§3.7 statik-prior borcuna yeni-tüketici).
- **Bütçe:** kota-modunda anlık dönüş-tüketimi = önselin %100'ü (bütçe-payının 4×'ü) — TASARIM
  GEREĞİ ve SINIRLI: epizot-başına ≤(R−1)×65B; epizotlar oturum-başı + gerçek-kopma-dönüşleri
  (+beyanlı geçiş-epizotları); kredi-aşımı K-4 invariantıyla YAPISAL imkânsız.
- **R'nin fiili anlamı:** açlıkta epizot yenilenmez → R tam bağlar: R=4 → denemeler
  {0.05, 2.15, 4.25, 6.35}s, P(≥1 sağ)≈1−0.8⁴≈0.59, sonrası 8.39s-kafes; R=8 → ~14.7s'e dek
  8 deneme, P≈1−0.8⁸≈0.83. **R [CAL {4, 8}] — ONAYLI** (karar-1). **En-küçük-yeterli-R seçimi
  ÇOK-KRİTERLİ (karar-1 şartı): YALNIZ IQR değil — K-1a ∧ K-2 ∧ K-4 birlikte; dar-IQR verip
  kota-yükünü veya kuyruk-başarısızlığını artıran kol SEÇİLMEZ.**

## 3. Bacak-2 — §3.3-E MİNİMAL-TABAN: varış-oranı hızlanması [r2'den aynen]

**Spec-donuk cümle (engine-spec §3.3-E):** "DATA arrival-rate'iyle hızlanır. İLK SACK
cadence-exempt. FULL adaptif decimation ADIM-6 ana gövdesi — burada yalnız minimal taban."
```
cadence_eff = if since_last_ack > 0 ∧ last_gap_ns > 0:
                  clamp(K_ARR × last_gap_ns,  alt = T_SPACING,  üst = ACK_CADENCE_FLOOR_NS)
              else: ACK_CADENCE_FLOOR_NS
referans-saat = last_emit_any   (tail-emisyonları DA sayılır; tail'in kendi-ritmi DEĞİŞMEZ)
```
- **K_ARR [CAL {2, 4}]** (kanal-türevli; {1} kolu elendi — yapısal paket-başı-ACK).
- Bacak-2 hızlanma-girdisi `last_gap` (anlık akış-ritmi) KALIR — epizot-dedektörünün robust-
  baseline'ı AYRI iş (tespit vs ritim-takibi; clamp alt-sınırı T_SPACING mikro-gap selini keser).
- Etki-bölgesi: açlıkta etkisiz (taban korur); orta-rejimde (gap 0.5-3s → cadence 1-6s) Faz-3
  döngüsünü gevşetir. 0.25k'da K_ARR×gap > FLOOR her kolda → dokunmaz.

## 4. P5-GATE kabul-kriteri [PATCH-2 + PATCH-1b + PATCH-4 işli]

Ölçüm: p5×6 × R∈{4,8} (=12 koşu; **piyango-ekseni p5'te — DV-2-profilinde DEĞİL**) +
p4/temiz-1k/0.25k smoke. IQR-yöntemi: tip-7.

**Tohum-çürümesi zemini (r2'den):** kurtuluş m. denemede → tohum ≈ 267/(m×2.1) B/s:
m=0: ~10k, m=1: 127, m=2: 64, m=3: 42, m≥4: ≤32. Orta-tohum sınıfının kaderi ProbeUp-köprüsüne
kalır (erken-kurtuluş → ~110s koşu-payı → EAI(127)≈6.9s → periyot ~28s → 1-2 prob → 3c-kanıtlı
7.6×-sınıfı rampa).

| katman | tip | eşik | anlam |
|---|---|---|---|
| **K-1a kafes-ölümü** | **KAPI** | medyan-ilk-teslim ≤ 8.4s (mevcut ~25.5s) | piyango-ekseni öldü (mekanik; R'den bağımsız-sağlanabilir) |
| **K-1b kuyruk** | **RAPOR** (6-koşuda) | >16.8s oranı raporlanır + teorik binomla karşılaştırılır: p(R=4)=0.8⁵≈0.328 (kota-4 + kafes@14.74s = 5 deneme ≤16.8s), p(R=8)=0.8⁸≈0.168; n=6 beklenen ≈2.0 / ≈1.0; kol-başı n=6 %95-üstleri (bilgi): R=4 → ≤4/6, R=8 → ≤3/6 | n=6'da kuyruk-kapısı piyango-bağımlı (P(≥2 aşım|R=4,mekanizma-doğru)≈%63.8 — kapı olamaz, PATCH-2 aritmetiği). **12-koşu-birleşiminde KAPIYA döner** — **12-koşu = SEÇİLEN-R'nin seçim-serisi-6 + doğrulama-serisi-6 (AYNI-p havuzu; karışık 6×R4+6×R8 havuzu DEĞİL** — karışıkta eşikler anlamsızlaşır, verifier-bulgusu): gözlenen >16.8s sayısı ≤ KESİN-binom-%95-üst: R=4 → ≤7/12 (μ=3.93, σ=1.63; P(X≥8)=%1.68), R=8 → ≤4/12 (μ=2.01, σ=1.29; P(X≥5)=%3.73) |
| **K-2 taban** | KAPI | 6-koşu teslim min ≥ 80 | açlık+orta-tohum ProbeUp-köprüsüyle kapandı |
| **K-3 dar-IQR** | KAPI (tam-mühür) | tip-7 IQR ≤ 20 ∧ min ≥ 80, İKİ bağımsız 6'lık seride | P5-GATE |
| **K-4 yük** [PATCH-1b] | üç-katman | `episode_count` = DIAGNOSTIC (rapor; p5'te varış-incelmesi + geçiş-epizotları meşru-değişken) · `quota_emits ≤ R × episode_count` = **SERT YAPISAL INVARIANT** (kredi-aşımı imkânsız; birim-test + RTRAJ-denetimi) · `tx_ratio ≤ 1.25×` = **KAPI** — **pin [r3-doğrulama]:** tx_ratio = client `udp_tx_bytes / (msg_count × unit_wire)` (mevcut kampanya-metriği; p5-mevcut ≤1.18×) → İLERİ-yön koruması; DÖNÜŞ-yön yükü ayrıca RAPOR: RTRAJ `emit_total×65B/süre` (kapı değil — kredi-invariantı korur) | doğru-çalışan dedektör sahte-kırmızı veremez; spam yapısal imkânsız |
| **K-5 smoke** [PATCH-3] | KAPI | p4 1.00× ∧ temiz-1k ≥45 ∧ 0.25k 20/20. **Beyanlı-istisna (iki-R-kolu):** oturum-ilk kota → +(R−1) SACK: **R=4 → +195B, pencere ~6.3s; R=8 → +455B, pencere ~14.7s** → 0.25k BİT-ÖZDEŞ DEĞİL (davranış-sınıfı, ayrı raporlanır; pencere-sonrası özdeşlik beklenir) | regresyon-kapıları |

**İKİ-DALLI dürüst tahmin [PATCH-4 terminolojisi — "K-1-mühürü" İFADESİ YASAK]:**
- **dal-A:** K-1a ✓ + ProbeUp-köprüsü çalışır → K-2/K-3 ✓ → **tam-mühür** + en-küçük-yeterli-R
  (çok-kriterli, karar-1).
- **dal-B:** K-1a ✓ ama orta-tohum sınıfı 30-79 bandında kalır (trajektori: tohum 42-127 +
  teslim <80 + prob geç/yok) → raporlama AYNEN şöyle:
  ```
  Bacak-1 alt-kapısı: ACCEPTED        (kafes-ölümü bağımsız-kazanım olarak kaydedilir)
  P5-GATE: DEFERRED — bacak-3 kanıtı gerekli
  Tam mühür: yalnız K-2 ∧ K-3 sonrası
  ```
  P5-GATE mühürlenMEZ; paket GENİŞLETİLMEZ; bacak-3 ayrı teşhis+tur (karar-2: iki-aşamalı ONAY).

## 5. Bacak-3 AYRILMA-SINIRI (bu pakete GİRMEZ) [r2'den aynen]

Sender-estimator tohum-hijyeni: (i) gap-sonrası ilk bw-örneği dairesel-ölçüm geçerlilik-kapısı,
(ii) plato-yanılgısı, (iii) supersede-sansürü — `sender.rs`/`bw.rs`, srtt-KÖK sınıfı → KENDİ
tasarım-notu + izolasyonu. **SINIR-TESTİ:** diff `receiver.rs` + RTRAJ-bench-kanalı + testler
DIŞINA taşarsa kapsam İHLAL → DUR.

## 6. Ölçüm-gözlemlenebilirliği (pakete girer, davranışsız)

- Receiver diag: `diag_episode_count`, `diag_quota_emits`, `diag_accel_emits`.
- Bench: serve-loop'a periyodik `RTRAJ ep= quota= accel= emit_total=` stderr-satırı (K-4
  invariant + diagnostic buradan; client `udp_rx_bytes` yalnız hayatta-kalan-bayt vekili).
- K-4 invariantı birim-testte de: kota-kredisi aşımı = test-kırmızısı.

## 7. §16.2 satır-taslakları (ZM-3 — onayla spec'e girecek)

| mekanizma | sınıf | satır-özü |
|---|---|---|
| epizot-dedektörü | kanal-türevli ✓ | robust-baseline (max→median(k), k [CAL {3,5}]) + `current_gap > K_EPISODE×baseline` (kesin >, pinli); K_EPISODE [CAL {3,4}] BOYUTSUZ kanal-türevli çarpan (ZM-yasağı mutlak-ZAMAN sabitleri — boyutsuz oran izinli, beyan); tetikleyen-gap pencereye yazılmaz; oturum-ilk açık özel-durum |
| T_SPACING | **statik-önsel-türevi** | SACK_WIRE/RETURN_BW_PRIOR ≈ 2.1s; E-FULL'de ölçülene bağlanır — §3.7 borcuna yeni-tüketici |
| cadence_eff | kanal-türevli ✓ | clamp(K_ARR×last_gap, T_SPACING, FLOOR); yalnız since_last_ack>0 ∧ last_gap>0; referans = last_emit_any; K_ARR [CAL {2,4}] |
| R kota | sayaç (saat değil) | epizot-başı toplam emisyon-kredisi (muaf dahil); [CAL {4,8}]; `quota_emits ≤ R×episode_count` SERT-INVARIANT |

## 8. Kararlar (Abkarada — r2 §8'in kapanışı)

1. **R-matrisi {4,8} ONAY**; en-küçük-yeterli-R seçimi ÇOK-KRİTERLİ (K-1a ∧ K-2 ∧ K-4 ∧ IQR).
2. **Dal-B iki-aşamalı ONAY** — PATCH-4 terminolojisiyle (alt-kapı-ACCEPTED / P5-GATE-DEFERRED /
   tam-mühür-sonra).
3. **Oturum-ilk kota KALIR** — p5 kader-penceresi tam oturum-başı; maliyet tek-seferlik ve
   iki-R-kolu için beyanlı (K-5); bit-özdeşlik-bozulması davranış-sınıfı olarak ayrı-raporlanır.

## 9. Beklenti-kaydına girecekler (kod-GO sonrası, ölçümden ÖNCE yazılacak)

- K-1a kafes-kaybolması: medyan-ilk-teslim ~25.5s → ≤8.4s.
- R=4-vs-8 AYRIK karşılaştırma; seçim çok-kriterli.
- İki-dallı dürüst tahmin (dal-B'de paket genişletilmez; "bacak-3-kanıtı-doğdu" kaydı + ayrı teşhis).
- Adversarial vektörler (V-1..V-5) birim-teste, CAL-kolu-başına (K_EPISODE×k çapraz), kota-
  yenilememe assert'iyle; V-5 tamsayı-ns tam-sınır semantiği iki K-kolunda pinlenir.
- count=0-deliği senaryosu ("ilk-paket→kopma→dönüş") birim-teste: dönüşte epizot YOK (beyanlı)
  ama kota-yanış-şartı sayesinde oturum-kredisi sağ → dönüş-SACK'leri kredili assert'i.
- Rejim-geçişi yakınsaması birim-teste: 25ms→800ms dizisinde epizot-sayısı ≤ ⌈log_K(oran)⌉+1.
- **DV-2 beklenti-kaydına taşınacak AÇIK-RİSKLER (Abkarada-kararı):** (i) count=0-deliği
  ("ilk-paket→kopma→dönüş": dönüşe epizot yok — DV-2'de bu desen ÖZELLİKLE ölçülür; zarar
  görülürse ayrı-fix); (ii) kopma-yoğun-desen sürüklenmesi (%90-kopuk profili: ritim-öğrenme
  davranışı gözlemlenir — beyanlı-tercih doğrulanır ya da yeniden-açılır).
- Smoke: p4 1.00× ∧ temiz-1k ≥45 ∧ 0.25k 20/20 + beyanlı-istisna (K-5, iki-R-kolu).
- P5-GATE kanıtı p5-profilinde (DV-2-profilinde değil).
