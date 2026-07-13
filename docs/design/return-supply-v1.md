# return-supply-v1 — Dönüş-Arz Sözleşmesi (bacak-1+2) + P5-GATE kabul-kriteri [TASLAK r2 — ONAY BEKLİYOR]

> **Statü:** TASARIM-NOTU (D0-tarzı). Abkarada'ya döner; ONAYDAN SONRA bacak-1+2 kodu.
> **Zemin:** ayrıştırma-kaydı time-model-v1 §0.23. **Kapsam-kilidi:** bacak-1+2 = YALNIZ
> receiver-taraf (+bench ölçüm-kanalı); sender'a SIFIR dokunuş. Bacak-3 ayrılma-sınırı §5.
>
> **r1→r2 (workflow wf_ddecb38d: 3 lens, 20 bulgu / 6 HIGH — hepsi işlendi):**
> r1'in üç çekirdek-aritmetiği kırıktı: (1) epizot-eşiği=FLOOR(8.39s) açlık/0.25k varış-aralığının
> (8.6-9.1s) ALTINDA → epizot-per-varış → 0.25k dönüş-kanalı KALICI taşma (32.8 > 31.25 B/s) →
> **eşik akış-türevliye çevrildi (K_TAIL×last_gap)**; (2) K-1 medyanı kendi piyango-aritmetiğiyle
> imkânsız + R'den bağımsızdı → kriter mekanizmadan yeniden türetildi; (3) tohum-ÇÜRÜMESİ
> (m. denemede ≈127/m B/s) orta-tohum sınıfını iki dal arasında bırakıyordu → üç-katmanlı
> kabul + ProbeUp-kurtarış köprü-hipotezi; (4) DV-2 çift-görev iddiası kayıtlı profillerde
> tetiklenmiyordu → akış-türevli eşikle 2s VE 8s kopmaları kapsanır (aritmetik §1a).
> Ayrıca: emisyon-saati birleştirildi (çift-emisyon/öncelik belirsizlikleri), T_SPACING dürüst
> etiketlendi (statik-önsel-türevi), K_ARR {1} kolu elendi, K-4 ölçüm-kanalı eklendi, §16.2
> satır-taslakları eklendi (ZM-3).

## 0. Problem-cümlesi

Dönüş-yolu SACK-arzının tek şans-bağımsız tabanı statik-önsel cadence (8.39s); ilk-SACK-muafiyeti
tek-atımlık ve EMİSYONLA tükeniyor. %80 dönüş-kayıplı p5'te koşu-kaderi tek 65B SACK'in
piyangosuna iniyor (kafes-kanıtı §0.23); kopma-sonrası yeniden-başlamada (DV-2) aynı mekanizma
muafiyetsiz + ağırlaştırıcılı. Bu not arz-tabanını piyango-bağımsız hâle getirir.

## 1. Bacak-1a — SESSİZLİK-EPİZODU tespiti ve muafiyetin yeniden-kurulması

**Epizot-tanımı (akış-türevli — r2):** Varış-anında, `last_gap`/`last_arrival` GÜNCELLENMEDEN
ÖNCE değerlendirilir:
```
yeni_epizot = !have_recv                                  # oturum-ilk (açık özel-durum)
            ∨ (last_gap_ns > 0
               ∧ now − last_arrival_ns ≥ K_TAIL × last_gap_ns)   # akış-ritmi kırıldı
```
- **Saat-sınıfı:** tam kanal-türevli (ZM-2) — eşik mevcut `K_TAIL=2`'nin YENİDEN-kullanımı
  (tail-mantığıyla aynı anlam: "bu boşluk mevcut akış-ritmiyle açıklanamaz"); YENİ SABİT YOK,
  statik-önsele bağımlılık YOK (r1'in FLOOR-eşiği çapraz-saat ZM-4 ihlaliydi — lens-2).
- **Aritmetik-doğrulama (r1-HIGH'ların kapanışı):** temiz-0.25k: gap≈9.9-12.7s, eşik≈20-25s →
  epizot OLUŞMAZ ✓; p5-açlık ritmik damlama: gap≈8.6s, eşik 17.2s → epizot oluşmaz (kota-spam
  yok) ✓; DV-2 p6 2s-kesinti: pencere-içi gap(100kbit)≈25ms, eşik 50ms → 2s ≥ 50ms ✓ tetikler;
  8s-kopma (3kbps pencere gap≈0.8s, eşik 1.6s) ✓ tetikler; E-FULL-sonrası doygun-simetrik kanal:
  gap/eşik = 1/K_TAIL = 0.5 < 1, bant-BAĞIMSIZ → epizot-per-varış sınıfı YAPISAL ölür ✓.
- **Değerlendirme-sırası:** kopma-sonrası ilk varışta eşik ESKİ (kopma-öncesi) gap'le hesaplanır →
  last_gap-zehirlenmesi (§0.23 ağırlaştırıcı-c) epizot-tespitini etkileyemez.
- **Kabul-edilen sınırlar (beyanlı):** (i) kopma-İÇİ tek başıboş-datagram/ağ-dup'u last_arrival'ı
  ilerletir → dönüşteki epizot o straggler'a göre ölçülür (gecikmiş ama gap-oranlı — sınırlı);
  ağ-dup'unun force-SACK'i cadence-saatini ilerletebilir (nadir; kota bunu telafi eder). (ii)
  Oturum-ilk epizot HER profilde kota verir → 0.25k'da ilk ~6.3s'te +(R−1) SACK (bkz. K-5).

**Muafiyet:** epizodun İLK emisyonu gecikmesiz (cadence-gate'siz). "İlk emisyon" HERHANGİ yoldan
olabilir (freq dahil — aşağıdaki birleşik emisyon-saati): epizotu açan varış freq-SACK
tetiklediyse O muafiyettir; timer ayrıca bitişik ikinci-kopya ATMAZ (r1-belirsizliği kapandı:
bitişik-emisyon = tek piyango-bileti, dekorelasyon-gerekçesiyle tutarlı).

## 2. Bacak-1b — Epizot-başı SACK-TEKRAR kotası (R)

**Sözleşme (r2 — birleşik emisyon-saati):** Yeni-epizot `kota = R` kurar (**R = epizot-başı
TOPLAM emisyon-kredisi, muaf-ilk-emisyon DAHİL** — r1'in R+1 off-by-one'ı kapandı). Kota > 0
iken timer-yolu aralığı `T_SPACING`'tir (cadence yerine); HER SACK-emisyonu (freq/timer/tail
fark etmez) kotadan 1 düşer VE paylaşık emisyon-saatini (`last_emit_any = max(last_ack_ns,
last_tail_ack_ns)`) ilerletir; emisyonlar-arası ≥ T_SPACING kota-modunda da korunur. Kota 0 →
normal rejime dönüş (bacak-2 cadence'i). Epizot-yenilenmesi kotayı R'ye RESET'ler (birikim yok).

- **T_SPACING = SACK_WIRE / RETURN_BW_PRIOR ≈ 65/31 ≈ 2.1s.** **Dürüst saat-sınıfı etiketi
  (r2): STATİK-ÖNSEL-TÜREVİ** — bugün ölçüm içermez; E-FULL'de ölçülen return_bw'ye bağlanır
  (§3.7 statik-prior borç-satırına YENİ TÜKETİCİ olarak işlenecek). Gerekçeler aynı: dar-dönüşte
  doğal seyrekleşme, burst-kayıp dekorelasyonu, kota-penceresi ≈ 4×2.1 = 8.4s ≈ tam eski-slot.
- **Bütçe-dürüstlüğü (r2):** kota-modunda ANLIK dönüş-tüketimi = 65B/2.1s = 31 B/s = önselin
  %100'ü (bütçe-payının 4×'ü) — TASARIM GEREĞİ ve SINIRLI: epizot-başına ≤(R−1)×65B ek.
  Akış-türevli eşik sayesinde epizotlar yalnız oturum-başı + gerçek-kopma-dönüşleri (temiz
  akışta ve açlık-damlamasında YOK) → "sürekli kota-rejimi" sınıfı yapısal imkânsız.
- **R'nin fiili anlamı (r2 — akış-türevli eşikle):** açlıkta epizot yenilenMEZ → kota tek-seferlik
  ve R tam bağlar: R=4 → denemeler {0.05, 2.15, 4.25, 6.35}s (P(≥1 sağ)≈0.59, sonra 8.39s-kafes
  devam); R=8 → ~14.7s'e dek 8 deneme (P≈0.83). r1'deki "R=4≡R=8 bit-eşdeğer" bulgusu eski-eşiğin
  artefaktıydı; yeni-eşikle R-matrisi ayırt-edici ✓. **R [CAL {4, 8}]** — dar-IQR veren EN KÜÇÜK R.

## 3. Bacak-2 — §3.3-E MİNİMAL-TABAN: varış-oranı hızlanması

**Spec-donuk cümle (engine-spec §3.3-E, aynen):** "DATA arrival-rate'iyle hızlanır. İLK SACK
cadence-exempt. FULL adaptif decimation ADIM-6 ana gövdesi — burada yalnız minimal taban."

**Sözleşme (r2 — clamp + 0-guard + birleşik-saat):** kota-dışı timer-yolu aralığı:
```
cadence_eff = if since_last_ack > 0 ∧ last_gap_ns > 0:
                  clamp(K_ARR × last_gap_ns,  alt = T_SPACING,  üst = ACK_CADENCE_FLOOR_NS)
              else: ACK_CADENCE_FLOOR_NS
referans-saat = last_emit_any  (tail-emisyonları DA sayılır — r1'in tail×accel 20ms-çift-emisyonu
                                yapısal ölür; tail'in kendi-ritmi/cadence-bağımsızlığı DEĞİŞMEZ)
```
- **K_ARR [CAL {2, 4}]** — r1'in {1} kolu ELENDİ (yapısal paket-başı-ACK, lens-2). K_TAIL
  değer-sınıfı (varış-aralığı çarpanı); saat-sınıfı kanal-türevli ✓.
- **Alt-sınır T_SPACING:** burst/batch varışlarda (last_gap→µs) timer-selini keser (bütçe-tutarlı);
  0-guard tail'deki mevcut guard'ın simetriği.
- **Etki-bölgesi (dürüst):** açlık-havzasında K_ARR×8.6s ≥ FLOOR → etkisiz (taban korur); asıl
  etki orta-rejim (gap 0.5-3s → cadence 1-6s): Faz-3 döngüsünün timer-bacağını gevşetir,
  entegre-hayatta-kalmayı (r4-dersi) büyütür. Kader-penceresini bacak-1 çözer; TAMAMLAYICI.
- 0.25k'da K_ARR×gap > FLOOR her CAL-kolunda → bu kol 0.25k'a DOKUNMAZ (r1-istisna-metni yanlış
  kolu işaret ediyordu — düzeltildi; 0.25k'ın gerçek etkisi §4 K-5'te).

## 4. P5-GATE kabul-kriteri (r2 — mekanizmadan türetilmiş; onaya sunulan eşikler)

Ölçüm: p5×6 × R∈{4,8} (=12 koşu; piyango-ekseni p5'te — DV-2-profilinde DEĞİL, Abkarada-ayrımı)
+ p4/temiz-1k/0.25k smoke. IQR-yöntemi SABİTLENDİ: tip-7 (lineer interpolasyon) — n=6'da
yöntem-bağımlılık r1-bulgusuydu.

**Tohum-çürümesi gerçeği (r2, kriterlerin zemini):** kurtuluş m. denemede gelirse tohum ≈
267B/(m×2.1s) → m=0: ~10k, m=1: 127, m=2: 63, m=3: 42, m≥4: ≤32 B/s. Yani bacak-1 KAFESİ
öldürür ama orta-tohum (42-127) sınıfını yaratır; bu sınıfın kaderi **ProbeUp-kurtarışına**
kalır (köprü-hipotezi: erken-kurtuluş → ~110s koşu-payı → EAI(127 B/s)≈6.9s → periyot ~28s →
1-2 prob → 3c-kanıtlı 7.6×-sınıfı rampa). Kriterler bu yüzden ÜÇ-KATMANLI:

| katman | eşik | anlam |
|---|---|---|
| **K-1 kafes-ölümü** (mekanik) | medyan-ilk-teslim ≤ 8.4s (bir eski-slot; mevcut ~25.5s) ∧ ilk-teslim > 16.8s olan koşu ≤ 1/6 | piyango-ekseni öldü; eski 8.39s-ızgara kümelenmesi RAPORLANIR (bilgi — yeni 2.1s-örgü eski ızgaranın 4×-incelmesi olduğundan ızgara-testi kapı OLAMAZ, r1-bulgusu) |
| **K-2 taban** | 6-koşu teslim min ≥ 80 (R-seçim serisinde; mühür-serisiyle teyit) | açlık+orta-tohum sınıfları ProbeUp-köprüsüyle kapandı |
| **K-3 dar-IQR (MÜHÜR)** | tip-7 IQR ≤ 20 ∧ min ≥ 80, İKİ bağımsız 6'lık seride (R-seçim + doğrulama) | P5-GATE |
| K-4 yük | epizot/kota diag-sayaçları: kota-SACK ≤ R×epizot ∧ epizot-sayısı {p5: ≤2, temiz-akış: 1, 0.25k: 1} ∧ tx-oranı ≤ 1.25× | tekrar spam'e dönüşmedi (ölçüm-kanalı §6) |
| K-5 smoke | p4 1.00× ∧ temiz-1k ≥45 ∧ 0.25k 20/20 + **beyanlı-istisna:** oturum-ilk kota → ilk ~6.3s'te +(R−1) SACK → 0.25k BİT-ÖZDEŞ DEĞİL (davranış-sınıfı, ayrı raporlanır; sonrası-özdeşlik beklenir) | regresyon-kapıları |

**İKİ-DALLI dürüst tahmin (r2 — beklenti-kaydına girecek):**
- **dal-A:** K-1 ✓ + ProbeUp-köprüsü çalışır → K-2/K-3 ✓ → mühür + en-küçük-yeterli-R.
- **dal-B (workflow-sonrası olasılığı YÜKSELDİ):** K-1 ✓ (kafes ölür — bacak-1'in kendi işi) AMA
  orta-tohum sınıfı ProbeUp'a rağmen 30-79 bandında kalır (trajektori-kanıtı: tohum 42-127 +
  teslim <80 + prob-girişi geç/yok) → **Faz-2 dairesel-ölçüm bağlayıcı = bacak-3 kanıtı doğdu** →
  ayrı teşhis + bacak-3 turu; bu paket GENİŞLETİLMEZ (Abkarada-kuralı). K-1-başarısı yine de
  kalıcı kazanım (kafes + DV-2-yolu).

## 5. Bacak-3 AYRILMA-SINIRI (bu pakete GİRMEZ)

Sender-estimator tohum-hijyeni: (i) gap-sonrası ilk bw-örneği dairesel-ölçüm geçerlilik-kapısı,
(ii) plato-yanılgısı, (iii) supersede-sansürü — ÜÇÜ DE `sender.rs`/`bw.rs`, srtt-KÖK sınıfı →
KENDİ tasarım-notu + izolasyonu. **SINIR-TESTİ:** bu paketin diff'i `receiver.rs` + bench-ölçüm-
kanalı + testler DIŞINA taşarsa kapsam İHLAL.

## 6. Ölçüm-gözlemlenebilirliği (pakete girer, davranışsız)

- Receiver diag: `diag_episode_count`, `diag_quota_emits`, `diag_accel_emits`.
- **Bench ölçüm-kanalı (r2 — K-4 mevcut altyapıda ÖLÇÜLEMEZDİ, lens-3):** SACK'ler server-taraf
  Receiver'da doğar; serve-loop'un özet-çıkışı YOK → serve-loop'a periyodik stderr diag-satırı
  (`RTRAJ ep= quota= accel= emit_total=`) eklenir. Client-taraf `udp_rx_bytes` hayatta-kalan-bayt
  vekilidir (emit-sayısı değil) — K-4 server-satırından okunur.

## 7. §16.2 satır-taslakları (ZM-3 — onayla spec'e girecek)

| mekanizma | sınıf | satır-özü |
|---|---|---|
| epizot-eşiği | kanal-türevli ✓ | K_TAIL×last_gap (yeniden-kullanım; yeni sabit yok); last_gap-güncellemesinden ÖNCE değerlendirilir; oturum-ilk açık özel-durum |
| T_SPACING | **statik-önsel-türevi** (dürüst-etiket) | SACK_WIRE/RETURN_BW_PRIOR ≈ 2.1s; E-FULL'de ölçülene bağlanır — §3.7 statik-prior borcuna yeni-tüketici satırı |
| cadence_eff | kanal-türevli ✓ | clamp(K_ARR×last_gap, T_SPACING, FLOOR); yalnız since_last_ack>0 ∧ last_gap>0; referans = last_emit_any |
| R kota | sayaç (saat değil) | epizot-başı toplam emisyon-kredisi (muaf dahil); [CAL {4,8}] placeholder-golden'a girmez |

## 8. Açık sorular (onayla birlikte)

1. R-matrisi {4,8} (öneri) — {2} kolu zayıf (P=0.36), {8}-üstü bütçe-ağır.
2. K-2/K-3'ün dal-B'ye düşmesi hâlinde mühür-tanımı: "K-1-mühürü + bacak-3-sonrası tam-mühür"
   iki-aşamalı kabul edilir mi? (Öneri: evet — kafes-ölümü bağımsız-değerli.)
3. Oturum-ilk kota istisnası (K-5, 0.25k +3 SACK) kabul mü, yoksa oturum-ilk epizot kota-SIZ
   (yalnız muafiyet) mi olsun? (Öneri: kota KALSIN — p5'in kader-penceresi tam oturum-başı;
   0.25k maliyeti tek-seferlik 195B.)
