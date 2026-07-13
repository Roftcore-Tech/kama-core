# srtt-clock-v1 — srtt-Türev-Ayrıştırması (TUR-3a.5 TASARIM NOTU, D0-tarzı)

> **Statü: r1 — Abkarada onayı bekliyor. Kod bu notun onayından SONRA** ("kod notun önüne geçemez").
> Otorite: transport-tdd > engine-spec (§16.2 ZM) > time-model-v1 (§0.13 KÖK-kaydı) > bu not > kod.
> **Üretim-şeffaflığı:** qd-merceği + mekanizma-kataloğu ultracode-workflow'undan (wf_f8792701);
> tepkisel/ölçek/SRC mercekleri haftalık subagent-limiti nedeniyle INLINE yapıldı (aynı kod-zemin
> disipliniyle; düşman-geçiş de inline — nihai doğrulayıcı bu onayın kendisi).

## 0. Kök (ölçülü, time-model-v1 §0.13)

srtt EWMA (α=1/8) **örnek-SAYISI saatinde** yakınsıyor — ZM-1'in estimator-içi ders-kitabı örneği.
Ölçüldü: kuyruk boşken (infl=267B≈2s) srtt=17.9s → qd sahte-yüksek → gain 0.5 mıhlı (temiz-1k
42-kilidi, yarı-1) + SRC Little-kolu cwnd/srtt sahte-düşük cap (yarı-2). 9 tüketici lag'ı miras alıyor.

## 1. MİMARİ-KARAR: tek-onarım mı, türev-ayrıştırması mı?

**CEVAP: SEKİZE TEK-ONARIM + qd'YE AYRIŞTIRMA (2 türev, 9 bespoke değil).**
- **`srtt_t`** — zaman-farkındalıklı EWMA (kuantum-döngü, §2.1): tüketiciler 1-8. Sekizinin ihtiyacı
  "merkezi-eğilim, kanal-zamanlı yakınsama" sınıfında birleşiyor; ayrı türevler kazanım getirmiyor
  (ölçek-grubu 8×-çarpan zırhlı; tepkisel-grup yönlü-güvenli — aşağıdaki tablo).
- **`qd_short_min`** — kısa-pencere-min (§2.2): YALNIZ tüketici 9 (qd). Kategori farkı ölçümle sabit:
  qd "ayakta-kuyruğun anlık değeri"ni ister; hiçbir α ortalamayı min'e çeviremez. Kuyruk-fiziği
  argümanı (workflow qd-merceği): RTT örneği kuyruğu YAŞAMIŞ paketten gelir, gerçek transitin altına
  inemez → gürültü TEK-TARAFLI-YUKARI → kısa-min hem robust hem anlamsal-doğru; rtprop=uzun-min ile
  simetrik (ZM-4 aynı-saat). rtt_last'ı DOMİNE eder (keep-last ile tek-örnek halde ona dejenere;
  ≥2 örnekte aykırı-maskeleme bedava); hızlı-EWMA kategori-hatası.
- Dokuz-ayrı-türev AŞIRIlığı da tek-skaler YETERSİZliği de reddedildi; patlama-yarıçapı 2 mekanizmaya iner.

## 2. Mekanizmalar (NORMATİF-adayı; golden'la kilitlenecek)

### 2.1 `srtt_t` — kuantum-döngü zaman-farkındalıklı EWMA [workflow mekanizma-kataloğu a.1]
```
n = clamp(Δt / T, 1, CAP=64);  Δt = now − last_sample_ns
mevcut RFC-6298 güncellemesi AYNI örnekle n kez uygulanır (adım-içi sıra: rttvar ÖNCE, srtt SONRA)
→ n=1'de bugünkü aritmetikle BİT-ÖZDEŞ (tv_rtt 4 vektörü VERBATIM geçer — davranış-nötr migrasyon)
→ rttvar EŞ-İŞLENİR (a.6: jitter=rttvar/srtt oranı iki saat-sınıfına bölünmez, ZM-4)
→ kalan-politikası: DÜŞER (Seçenek-1; muhafazakâr-yönlü, ≤1 kuantum/örnek; denetimi en kolay)
→ taşma: u128 mevcut desen; kesme <64ns kümülatif @CAP — golden-güvenli
```
**T (referans-aralık) = rtprop / K_CONV** — a.7 analizi: T=sabit-ms ZM-2 RED (MIN_RTT_FLOOR sınıf-
tekrarı); **T=ack_epoch ÖZ-REFERANS TUZAĞI** (şişkin srtt→şişkin T→n=1: ölçülü izde motive-vakada
ETKİSİZ — hesaplandı); T=EAI return-prior-SUSPECT mirası; **rtprop = 3a-sonrası tek dürüst saat**
ve srtt-lag'ını miras almayan tek estimator. Yalın T=rtprop izde n≈1-2 (marjinal) → yakınsama-bütçesi
için K_CONV böleni: `K_CONV [CAL matris {2,4,8}], başlangıç 4` → izde n=3-7 → srtt 17.9→3-5s
~10-20s duvar-zamanında (bugün 45-75s). **Floor-1 hızlı-decade güvenliği:** Δt≪T → n=1 → bugünkü
RFC-6298 derinliği aynen; hızlanma yalnız Δt>T bölgesinde. rtprop=None (PreConverge) → n=1 fallback.
Bilinçli kalibrasyon-kayması: seyrek-ACK'li hızlı kanallar örnek-başına daha tepkisel — ADR-18
4-decade kapısında doğrulanır; K_CONV ölçümsüz [CAL].

### 2.2 `qd_short_min` — ayakta-kuyruk ölçümü [workflow qd-merceği]
```
qd = min_{W_qd}(rtt_sample) − rtprop ;  W_qd = K_qd × ack_epoch  (K_qd [CAL {1,2,4}], başlangıç 2)
— RttEstimator'a İKİNCİ MinFilter (aynı observe beslemesi; keep-last aktif; W_qd ≤ W_RTPROP invariantı)
— [STD] 10s tabanı BİLİNÇLİ YOK (keep-last boşalmayı önler; taban hızlı-decade'de soft-drain'i köreltir)
— salıverme ANINDA (drain-sonrası ilk düşük örnek min'i çökertir — 42-kilidinin tam ilacı);
  dolma-gecikmesi ≤W_qd (dolma yönünün yapısal yedeği zaten var: hard-cap + 3b Drain [ŞART-3])
```

## 3. DOKUZ-TÜKETİCİ TABLOSU (karar + gerekçe + yanlış-seçim-bedeli + golden-etkisi)

| # | tüketici | ihtiyaç | türev | gerekçe (kısa) | yanlış-seçim bedeli | golden |
|---|---|---|---|---|---|---|
| 9 | qd → qd_ratio (fuzzy) | ayakta-kuyruğun ANLIK değeri; yukarı-gürültüye bağışık; salıverme-lag≈0 | **qd_short_min** | kuyruk-fiziği tek-taraflı-yukarı → min anlamsal-doğru; rtt_last'ı domine (keep-last dejenerasyonu); EWMA kategori-hatası | srtt'de kalırsa gain-mıhı sürer → ŞART-1 (≥45) sağlanamaz; rtt_last'ta aykırı-başına on-sn sahte-fren | **davranış-sınıfı** (yeni filtre) |
| 1 | loss-eşiği 9/8·srtt | üst-zarfımsı ölçek; tepkisellik faydalı, taban 1a/1b-korumalı | **srtt_t** | şişkin→geç-tespit (yönlü-güvenli); tazelik tespit-gecikmesini kısaltır; false-loss tabanı kanıt-şartı+cadence-taban'da | aşırı-taze türev (rtt_last) eşiği oynatır — ama cadence-taban mutlak-alt-sınır: risk sınırlı | kalibrasyon |
| 2 | ack_epoch→W_BTLBW+W_loss | kararlı ölçek; şişkin-yönlü zararsız | **srtt_t** | 8×-çarpan zırhı + keep-last; ayrı kararlı-türev kazanımsız (medyan-RED: örnek-sayısı saati + yeni golden yüzeyi) | — (tolerans yüksek) | kalibrasyon |
| 3 | W_RTPROP = K×srtt | aynı | **srtt_t** | aynı + 10s [STD] taban duruyor | pencere ani-daralması → 10s-taban + keep-last korur | kalibrasyon |
| 4 | tombstone-TTL | kararlı üst-sınırımsı | **srtt_t** | TTL kısalması yalnız geç-ACK FIRSATI azaltır (kayıp değil); taban cadence-türevi | — | kalibrasyon |
| 5 | jitter = rttvar/srtt | pay-payda AYNI saat (ZM-4) | **rttvar_t / srtt_t** | kuantum-döngü rttvar'ı eş-işler (a.6) — oran tek-saatli | payda onarılıp pay kalırsa oran saat-böler (ZM-4 ihlali) | kalibrasyon |
| 6 | SRC Little cwnd/srtt (**yarı-2**) | pencerenin FİİLEN döndüğü RTT | **srtt_t** (payda) | cwnd/rtprop≡2×btlbw → kol silinir (hard-ceiling kaybı, RED); rtt_last aykırı-çökertmesi RED; byte-cap'e dönüş faz-davranışı → 3c-notu; srtt_t: drain-sonrası cap>btlbw → kol yalnız GERÇEK kuyrukta bağlar (Little'ın kendisi) | şişkin kalırsa cap 48-89 sahte-düşük sürer (yarı-2 kapanmaz) | kalibrasyon |
| 7 | liveness-dup aralığı | tepkisel; iptal-edilebilir → agresiflik güvenli | **srtt_t** | şişkin→seyrek-kopya (p5-açlık sınıfı); taze→sık ama ACK-iptalli + bütçeli | tail-recovery yavaş kalır | kalibrasyon |
| 8 | ack_conf 3×srtt sessizlik-ölçeği | beklenen-ACK-ritmine göre sessizlik | **srtt_t** + ÖNERİ: `max(3×srtt_t, K_conf_eai×EAI)` [CAL, **karar-Abkarada**] | srtt_t hızlı düşerse 3×srtt < cadence-boşlukları → sahte-LOW-conf riski; EAI-tabanı ZM-4 uyumu | taban yoksa sub-kbps'te erken-RecoverFwd (dup-bütçe israfı; iptal-edilebilir → orta risk) | kalibrasyon (+taban kabulünde davranış-sınıfı) |
| — | (PTO) | engine-içi henüz yok | spec-notu: geldiğinde srtt_t+4×rttvar_t | — | — | — |

## 4. Ölçüm-planı (İKİ-YARI AYRI — Abkarada uyarısı gereği)

```
commit-α  qd_short_min YALNIZ (tüketici-9)      → ölçüm-α: temiz-1k Δ (beklenti: gain-mıhı kalkar;
          42 → 44-46 bandı; Little-kolu hâlâ bağlıyorsa 45-altı kalabilir — yarı-ayrıştırma AMAÇ)
commit-β  srtt_t (T=rtprop/K_CONV; tüketici 1-8) → ölçüm-β: temiz-1k Δ (Little-kolu açılır;
          α+β BİRLİKTE ≥45 DOĞRULAMA-ŞARTI [3a-ŞART-1 devri])
her ikisi: 0.25k süreklilik KORUNUR (düşüş-adımı 0) + p4/p5×6 aynı-era smoke (yüksek-varyans istisnası)
+ blackout-sınıfı birim-vektörler (gap-zıplama n=8, CAP-satürasyon, floor-1, rttvar-eş-sönüm, Δt=0)
```

## 5. Golden-planı (ADR-17; Abkarada golden-şartı)

- Migrasyon default'u nötr: setter bağlanmadan n≡1 → **tv_rtt 4 vektörü VERBATIM** (bit-özdeşlik kanıtı).
- **davranış-sınıfı** (yeni dal/kural — yeniden-üretilir + ayrı raporlanır): qd tanımı (ikinci min-filtre)
  + kuantum-döngü gap-dalları (yeni vektör aileleri §4) [+ tüketici-8 EAI-tabanı kabul edilirse].
- **kalibrasyon-değişimi** (değer kayar, dal aynı): sim/sender-golden'ları (srtt-değerine bağımlı
  olanlar), tv_fusion qd_ratio örnek-değerleri, tv_src türetimleri. Tümü yeniden-üretilir,
  commit-mesajında iki-sınıf ayrı listelenir.

## 5.5 Düşman-geçiş özeti (inline; satır-satır çürütme denendi)

- qd_short_min eksik-okuma riski (pencerede doluş-öncesi örnek → min düşük → erken-salıverme):
  SINIRLI — dolma-gecikmesi ≤W_qd zaten beyan; dolma-yönü yapısal yedeği (hard-cap + 3b Drain) var.
- qd tek-örnek dejenerasyonu (0.25k): hold'lu raw-fallback örneği bir aralık boyunca qd-spike →
  bir gap'lik 0.5× — sınırlı, tabloda kayıtlı.
- loss-eşiği tazeleşmesi jitter'da false-loss: İMKANSIZ-yakını — cadence-taban hızlı kanalda
  DOMİNE (100kbit'te ≈3.2s ≫ jitter-kuyruğu) + kanıt-şartı.
- SRC srtt_t salınımı (cap-aç→burst→kuyruk→cap-kapa): pacing=btlbw birincil sınır — cap yalnız
  btlbw-üstü patlamaları törpüler; salınım genliği yapısal sınırlı.
- T=rtprop erken-şişkinliği (ilk örnek hold'lu): min-filter ilk temiz örnekle aşağı düzelir;
  PreConverge n=1 fallback.
- Hiçbir tablo-satırı düşmedi; kalıntılar: qd-dolma-sınırı (beyanlı) + tüketici-8 tabanı (açık-nokta 3).

## 6. Açık noktalar (onayla birlikte karar)

1. K_CONV başlangıcı 4 (matris {2,4,8}) — uygun mu?
2. K_qd başlangıcı 2 (matris {1,2,4}) — uygun mu?
3. Tüketici-8 EAI-tabanı (`max(3×srtt_t, K_conf_eai×EAI)`) — bu turda mı, 3b'de mi?
4. Byte-cap'e dönüş (SRC kolunun rate-form yerine inflight<cwnd kapısı) — 3c/ProbeUp tasarım-notuna
   devir kaydı (bu turda DEĞİL).
