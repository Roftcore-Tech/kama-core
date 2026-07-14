# b1-kopma-korlugu-v1 — Kopma-Körlüğü: ACK-Sessizlik-Kapısı + Dönüş-Kanıtı-Patlaması [r2 — DUR-GERİ-BİLDİRİMİ: B1 da meta-kök-ailesinde; sıra-gerekçesi kısmen-çürüdü; karar Abkarada]

> **Statü [Abkarada-KARAR]:** ★ DUR / NO-IMPLEMENTATION · B1 M4/M5-aralıklı-sensing-katmanına BAĞLANDI
> (aynı meta-kök; p5-mühür-riski kabul-edilemez). Korunan: 3-HIGH + meta-kök-indirgeme + kod-satırlı-
> çürütmeler. 1a-dup-oran ŞARTLI-PARK (M4/M5-alt-kümesi); 1b-ÇIKAR (HIGH-2 seq-bias). KOD YAZILMAZ.
> **Kapsam-sınırı:** sender-taraf pacing/dup-yolu (`sender.rs` schedule_transmit + plan_forward_
> duplication + drain_due_duplicates + pacer-etkileşimi) + testler + TRAJ; **başka her şey = İHLAL, DUR.**
> **AYRIM (Abkarada, korunur):** B1 = **incidence-reduction** (kopma-örneği DOĞUM-sıklığını +
> boşa-gönderim-israfını düşürür); B2/M4/M5 = **estimator-discrimination** (örneği ayırt-eder).
> B1 B2-meta-kökünü ÇÖZMEZ; kirlilik-doğumunu ve %74-israfı AZALTIR. Zemin: DV-2 p6b/c90 teşhisi
> (time-model §0.29) + kod-okuması.

## ★ r1→r2 DUR-GERİ-BİLDİRİMİ (doğrulama wf_c7e19441 — 3 HIGH; ikisi SIRA-KARARINI vurdu)

**ÜST-BULGU: DV-2'nin İKİ bulgusu (B1 kopma-körlüğü + B2 RTT-kirliliği) TEK META-KÖKE indirgeniyor.**
B1'in değerli-çekirdeği (kör-akışı-kes) "ne-zaman-kopma?" tespitini gerektirir → bu tam da B2'nin
yerel-ayırt-edilemezliği (kopma-vs-yavaş, TEK-gözlem). B1 forward-tarafta (ne-zaman-göndermeyi-dur),
B2 estimator-tarafta (örneği-nasıl-yorumla) — AMA ikisinin de doğru-cevabı M4/M5 (kanal-durumu-sensing).
B1 de YEREL-EŞİK-AİLESİNDE.

### HIGH-1 [SIRA-GEREKÇESİNİ ÇÜRÜTTÜ]: B1 B2-kirli-örnek-DOĞUMUNU azaltmaz (dup ≠ srtt-kaynağı)
Abkarada (c)-sıra-kararının bir dayanağı ["B1 kör-kopyaları-kesince straggler-ack-doğumu ↓ → B2
hafifler"] MEKANİK-YANLIŞ: srtt-örneği YALNIZ origin==Data'dan gelir (sender.rs:526,542); dup'lar
origin=Duplicate → ASLA srtt-örneği olamaz (B2'nin kendi bit-exact-zinciriyle tutarlı: zehir =
original-DATA-straggler, retx=0). B1 yalnız dup-yolunu keser; original-straggler-zehir (~20 birim
p6b) B1'in kapsamı-DIŞI. **Net B2-doğum-azalması ≈ 0.** Sıra-dayanağı [%74-WIRE-israf] DURUYOR;
[B2-hafifletme] dayanağı ÖLDÜ. → B1-önce artık YALNIZ wire-israf + gerçek-dünya-değeriyle savunulur.

### MED-b [KRİTİK — B1 YENİ P5-GATE-MÜHRÜNÜ TEHDİT EDER]: parasitic-recovery riski
TARİH (time-model §0.29 / dv-plan §0.7): kör-dup-storm'u yok-etmek p5'i 100→24'e DÜŞÜRMÜŞTÜ
("parasitik-recovery"). Kör-forward-akışın İKİNCİL-rolü: sessizlikte forward-path'i emit-eder-tutar
→ receiver SACK-üretmeye-devam → %80-reverse-loss'ta her-hayatta-kalan-SACK ARQ'yu açar. B1
forward-akışı nabza-indirirse bu SACK-fırsat-üretimi düşer → **p5-teslim RİSKİ (mühür-bozabilir).**
1a pulse-down SELEKTİF-DEĞİL (hangi-kopyanın-teslim-edeceğini bilemez = discrimination=B2-işi) →
teslim-eden-kopyaları da keser. Benim "p5-nötr, ProbeUp/ramp-taşır" iddiam YANLIŞ-ATIF: ölçüm
(tur5) p5-teslimi ARQ-retransmit'ten (RecoverArq) geliyor, ramp-değil.

### HIGH-2: 1b "yalnız-teslim-edilmemişlere-patlama" kopma-dönüşünde GÜVENİLMEZ
SACK MAX_ACK_RANGES=16 + en-yüksek-seq-biased (ack.rs:83,159); kopma-dönüşü rx-set çok-parçalı
(98-seq) → >16-range → ilk-SACK yalnız top-16 → alttaki TESLİM-EDİLMİŞ birimler unacked-sanılır →
1b onları TEKRAR gönderir. Kendini-besler (dup-fan-out seq-şişmesi). "Yanlış-tetik zararsız" YANLIŞ.
→ 1b-ÇIKAR-önerisi (SACK-truncation + karmaşıklık; wire-israf 1a-tek-başına da azalır).

### HIGH-3: eşik-referansı (B inflight-yaş) srtt-BAĞIMSIZ-DEĞİL (temiz-referans YOK)
Ölçülen-nicelik srtt-free ama GATE-eşiği için: mutlak=ZM-yasak · srtt-katı=B2-akraba ·
deadline=engine'de-YOK (InflightPacket'te field yok, §16.2) · rtprop=kirletilebilir · EAI=srtt-bağlı.
Ve meta-kök doğrudan inflight-yaşa uygulanır (gerçek-20s-kanal oldest-unacked-age=20-40s ≈ kopma).
"(B) yapısal-uzak" iddiam KENDİ-kör-nokta-(i)-beyanımla ÇELİŞİYOR. (B) (A)'dan yalnız HASAR-profilinde
farklı (dup-oran vs estimator-zehir), meta-kök-akrabalığında DEĞİL.

### KARAR-UZAYI (Abkarada — B1 de dar-turla-tam-çözülmez):
- **(a) DAR-WIRE-HAFİFLETME + p5-mühür-nöbeti:** yalnız 1a-dup-oran-azaltma (INV-RATE-1-güvenli,
  dup-scope; LOW-c teyit), 1b-ÇIKAR, hedef-iddiasız (dal-B kabul); **P5-GATE regresyon-kapısı
  ZORUNLU** (p5×6 {100×6} korunmalı — bozulursa B1-DUR). Kör-akışı-TAM-çözmez (kopma-tespiti M4/M5);
  yalnız en-kaba-israfı kırpar. Değer: kısmi-wire, düşük-risk-EĞER-p5-korunursa.
- **(b) B2-İLE-BİRLEŞTİR (M4/M5):** B1+B2 aynı-meta-kök → tek kanal-durumu-sensing-turunda
  (DV-3-sonrası) birlikte çözülür. En-temiz-mimari; ikisi de kampanya-öncesi-borç.
- **(c) ERTELE:** DV-3'ü mevcut-motorla (B1+B2 ikisi-de-artefakt-etiketli); M4/M5-sonra.

**Önerim: (b)+(c) birleşik** — B1'i de M4/M5-ailesine bağla (dar-turla-tam-çözülmez + p5-mühür-riski
çok-yüksek); DV-3'ü ertele-etme, iki-artefakt-etiketli-koş; latency-smoke B1'den-BAĞIMSIZ koşulabilir
(upfront, cwnd-kararı). Ama sıra-gerekçesi-çürümesi + p5-mühür-riski senin kararın.

## 0. HÜKÜM — kopma-körlüğü mekanizması (DV-2 p6b, ölçülü)

- **%74-israf (bit-ölçülü):** p6b'de 98 gönderimin ~73'ü KOPUK-HAVAYA (dup %79.6-pay; meta.tx
  retx=0, data=20, dup=78). Kök: pacer bağlantısızlığı BİLMEZ — `drain_due_duplicates`
  (sender.rs:1252) blok-boyu sabit-rate'te (516→258→193 B/s) kopya emit eder; `plan_forward_
  duplication` (sender.rs:1207) her RecoverFwd-girişinde tüm-inflight-DATA'ya geometrik-kopya
  (interval=srtt, ×2-backoff) planlar → kopma-boyunca "kör-akış".
- **İki hasar:** (i) dönüş-bütçesi + ileri-bütçe israfı (oran 4.90 = kör-kopyalar); (ii) DOLAYLI
  B2-besleme: kör-kopyalar kopma-boyunca gönderim sürdürür → kopma-sonrası straggler-ack'i
  ÜRETECEK paket-miktarı ↑ → B2-kirli-örnek doğum-sıklığı ↑ (B1 bunu azaltır — incidence-reduction).
- **ep-kaçağı = gönderici-kör-ritmi:** DV-2'de epizot-açmayan pencereler (p6b 4/12, c90 2/4)
  dedektör-suçu DEĞİL, o pencerelerde SIFIR-ileri-varış (gönderici kör-akışta "yanlış-anda"
  gönderiyor — dönüş-pencerelerine denk-getirmiyor).

## 1. FIX — İKİ-PARÇA (DV-2 teşhis-önerisinin r1-tasarımı)

### 1a. ACK-SESSİZLİK-KAPISI (kör-akışı nabza-indir)
`RecoverFwd`/tail-liveness dup-akışı, ACK-sessizliği belirli-eşiği aşınca DÜŞÜK-NABZA iner
(kesmez — liveness-probe'u korur; yalnız kör-israfı keser). Sessizlikte tam-rate yerine
tek-probe/backoff-periyodu.
- **Eşik-KLAS SORUNU (talimat adım-3 — B2-meta-kökün AKRABASI, açık-beyan):** "sessizlik > eşik"
  eşiği srtt-türevliyse (mevcut plan_forward_duplication interval=srtt) B2-kirli-srtt eşiği kaydırır
  → B1 geç-durur. **B1-risk-profili B2'den DÜŞÜK:** yanlış-tetik B1'de yalnız dup-ORANINI oynatır
  (teslim-korumalı — kör-akış zaten çöp; erken-nabız teslimi düşürmez, geç-nabız yalnız israfı
  sürdürür); B2'de yanlış-tetik estimator-ZEHRİ (kalıcı, 9-tüketici). Yine de tasarım-tercihi:
  eşik-referansı seçenekleri (onayda S1) —
  (A) srtt-türevli (basit; B2-kirliliğine duyarlı ama düşük-risk);
  (B) inflight-yaş-türevli (en-eski-unacked-paketin bekleme-süresi — srtt-BAĞIMSIZ; kör-akış
      = "hep-aynı-paketleri ack'siz-gönderiyorum" doğrudan-sinyali);
  (C) ACK-cadence-türevli (EAI — ama EAI→rate→srtt, kapalı-döngü riski B2-akrabası).
  **Öneri: (B) inflight-yaş** — srtt-bağımsız, kör-akışın doğal-imzası, B2-meta-kökünden yapısal-uzak.

### 1b. DÖNÜŞ-KANITI-PATLAMASI (biriken-arzı doğru-ana harca)
Sessizlik-boyunca dup-nabza-inince biriken forward-bütçe, İLK dönüş-kanıtında (ilk-ACK/SACK)
yalnız **TESLİM-EDİLMEMİŞ** birimlere patlama olarak salınır (teslim-edilmişlerin kör-kopyaları
DEĞİL). Mekanik: acked_units-farkı (sender.rs:626 dup-purge zaten var) → kalan-unacked'e
öncelikli-burst (pacer-token-payı, β2-K_ARQ_SHARE emsali sınırlı).
- **Kör-nokta (talimat):** "ilk dönüş-kanıtı" da sensing — gerçek-yavaş-kanalın geç-ACK'i vs
  kopma-dönüşü ayırt-edilemez (B2-akrabası). Ama B1-patlaması TESLİM-EDİLMEMİŞLERE (gerçek-ihtiyaç)
  → yanlış-tetik zararsız (zaten gönderilecek birimler erken-gider, çöp-değil).

## 2. KÖR-NOKTALAR (talimat adım-3; latency-smoke ölçecek)

B1 kendi-başına "yavaş-kanal-mı-kopma-mı?" sınırına yaklaşır (B2-meta-kökün akrabası — kabul):
- (i) gerçek-yüksek-RTT'de erken-durma (kör-akış sanıp nabza-iner → yavaş-kanalda dup-liveness zayıflar);
- (ii) cwnd/rate gereksiz-donma (sessizlik-kapısı pacing'i de etkilerse);
- (iii) sparse-kanalda yanlış-blackout-teşhisi (seyrek-ack'i kopma-sanma).
Üçü de **latency-smoke** (adım-3, upfront-backlog) + DV-3'te ölçülür; B1-notu bunları ÇÖZMEZ,
BEYAN eder ve düşük-risk-profilini (teslim-korumalı) gerekçelendirir.

## 3. Ölçüm-planı + beklenti (kod-GO sonrası, ölçümden önce)

- Birim: kör-akış-kesim vektörü (sessizlikte dup-oran-düşüşü); dönüş-patlama vektörü (ilk-ACK →
  yalnız-unacked-burst); eşik-referansı(B) inflight-yaş sınır-vektörü.
- DV-2-seti yeniden (ana p6b): oran 4.90 → ≤~1.5 hedefi (kör-israf kesildi); teslim ≥18/20
  KORUNUR (dal-A iyileşme: doğru-ana-patlama → 20/20; dal-B teslim-aynı-oran-düştü → dürüst-kayıt).
  h4/c90/count0 sınıf-aynı.
- Smoke: p5×2 + temiz-1k(=45) + 0.25k — B1 sürekli-kanalda tetiklenmez-tahmini (sessizlik yok);
  p5-açlık DİKKAT: p5-kör-akış (RecoverFwd-dup'ları) B1'le nabza-inerse p5-teslim etkilenebilir
  → özel-izlenir, dal-yapılı (B1 p5'te de dup-israfı-keser AMA p5-teslimi kör-kopyalara-bağımlı
  DEĞİL — ProbeUp/ramp taşıyor → nötr-beklenti).
- TRAJ: dup_blind=/burst_on_return= sayaçları.

## 4. Açık sorular (onayla)

- **S1: eşik-referansı** (A-srtt / B-inflight-yaş / C-EAI) — öneri B (srtt-bağımsız; B2-uzak).
- **S2: dönüş-patlama-payı** — pacer-token-tam mı, sınırlı-pay mı (β2-emsali; öneri sınırlı-pay,
  oran-patlamasını önler).
- **S3: latency-smoke B1-öncesi mi sonrası mı** — talimat sonrası (adım-3); ama B1'in kör-nokta-(i)
  riski smoke'ta görünür → smoke B1-KODU-sonrası zorunlu (talimat-uyumlu).
