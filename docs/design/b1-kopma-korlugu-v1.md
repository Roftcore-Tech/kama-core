# b1-kopma-korlugu-v1 — Kopma-Körlüğü: ACK-Sessizlik-Kapısı + Dönüş-Kanıtı-Patlaması [TASLAK r1 — ONAY BEKLİYOR]

> **Statü:** D0-TASARIM-NOTU (B1; B2-DUR sonrası öne-alındı — Abkarada sıra-kararı). ONAYDAN SONRA kod.
> **Kapsam-sınırı:** sender-taraf pacing/dup-yolu (`sender.rs` schedule_transmit + plan_forward_
> duplication + drain_due_duplicates + pacer-etkileşimi) + testler + TRAJ; **başka her şey = İHLAL, DUR.**
> **AYRIM (Abkarada, korunur):** B1 = **incidence-reduction** (kopma-örneği DOĞUM-sıklığını +
> boşa-gönderim-israfını düşürür); B2/M4/M5 = **estimator-discrimination** (örneği ayırt-eder).
> B1 B2-meta-kökünü ÇÖZMEZ; kirlilik-doğumunu ve %74-israfı AZALTIR. Zemin: DV-2 p6b/c90 teşhisi
> (time-model §0.29) + kod-okuması.

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
