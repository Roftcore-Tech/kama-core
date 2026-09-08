# FAZ 5 — HÜCRE SONUÇLARI (biriken kayıt)

Taban: `e3c1e8bb740bbc25acd2c9fc5a7fc478a3f15adc` (ERRATUM-1 sonrası).
Ön-kayıt: [`EXPECTATION.md`](EXPECTATION.md) · Erratum: [`ERRATUM-1-blackout-boundary.md`](ERRATUM-1-blackout-boundary.md)
Kıyas düzeyi tüm hücrelerde **NON-NORMALIZED** (Kama `reliable-messages`/sırasız ↔ rakipler
`reliable-ordered-stream`/tam sıralı). Deney koşulları eşleşiyor (BLOCKING yok).

---

## T0 — TEMİZ KONTROL (`h6_rtt20`: delay 10ms loss 1% rate 10mbit, çift yön) · N=30 · D=20s

| kol | n | claim | completion(20s) medyan | IQR | CI | outcome | blackout | stall |
|---|---|---|---|---|---|---|---|---|
| kama-v1-full | 30 | final | **1.000** | [1.000,1.000] | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |
| quinn-reliable-stream | 30 | final | **1.000** | [1.000,1.000] | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |
| tcp-default | 30 | final | **1.000** | [1.000,1.000] | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |

completion(5s) ve completion(10s) de üç kolda 1.000.

**Kama aktivasyonu (medyan):** parity=0 · denied=0 · retx=33 · dup=0 · tail_probe=33 ·
srtt=20.9 ms (enjekte 2×10 ms ile tutarlı) · btlbw=13.9 kB/s · fwd_loss=7‰ (enjekte %1).

**Tel maliyeti (medyan):** kama data=28 300 B + retx=9 069 B (+parity=0) ≈ **37.4 kB** ·
quinn **35.8 kB** · tcp 26.8 kB.
*TCP sayısı UYGULAMA katmanıdır; çekirdek retransmit/ACK/başlık trafiği harness'a görünmez ve
Kama/quinn'in tam tel muhasebesiyle DOĞRUDAN kıyaslanamaz (metodoloji §10).*

**Ön-kayıtlı beklentiye karşı:**
- "Kama normal çalışmalı" → ✔ 1.000
- "parity ≈ 0 kalmalı" → ✔ parity=0, yapı-gate temiz yolda hiç açılmadı
- "patolojik blackout olmamalı" → ✔ 0/30
- "patolojik retransmit olmamalı" → retx/data **%32** (33 retx / 100 mesaj), tamamı tail
  probe (retx == probes). Ancak TOPLAM tel Kama 37.4 kB ↔ quinn 35.8 kB, yani **%4 fark**.
  Yani fazladan iletim gecikme karşılığında ödenmiş bir tasarım takası; toplam maliyet QUIC ile
  paritede. **Patolojik DEĞİL.** Bağlam: 50 ms'lik paced geometri her mesajdan sonra doğal bir
  "kuyruk" yaratır, bu yüzden tail probe sık ateşler — yük biçiminin sonucu, kolun kusuru değil.

### T0 HÜKMÜ: **COMPARABLE / PARITY**
Birincil metrikte üç kol da tavanda ve güven aralıkları özdeş. Kontrol hücresi amacını
gerçekleştirdi: **Kama'nın hostile uzmanlaşması sağlıklı yolda regresyon üretmiyor.**

---

## T1a — RANDOM LOSS (`p1_loss50`: loss 50% rate 100kbit, çift yön) · N=30 · D=20s

| kol | n | claim | completion(20s) medyan | IQR | CI | outcome | blackout | stall |
|---|---|---|---|---|---|---|---|---|
| kama-v1-full | 30 | final | **1.000** | [1.000,1.000] | [1.000,1.000] | blackout=18 completed=10 deadline-miss=2 | 18/30 en uzun 15.0s | 18/30 CI[0.406,0.773] |
| quinn-reliable-stream | 30 | final | 0.900 | [0.470,1.000] | [0.530,1.000] | blackout=25 completed=2 conn-fail=1 timeout=2 | 27/30 en uzun 18.0s | 28/30 CI[0.779,0.992] |
| tcp-default | 30 | final | 0.120 | [0.050,0.550] | [0.060,0.510] | blackout=26 conn-fail=1 timeout=3 | 29/30 en uzun 26.0s | 30/30 CI[0.884,1.000] |

Deadline eğrisi (medyan): **D=5s** kama 0.940 / quinn 0.220 / tcp 0.040 —
**D=10s** kama 0.990 / quinn 0.510 / tcp 0.060 — **D=20s** kama 1.000 / quinn 0.900 / tcp 0.120.

### Varsayımsız dağılım ifadeleri (sıra-tabanlı, Gaussian varsayımı YOK)

| | kama-v1-full | quinn | tcp |
|---|---|---|---|
| min | **0.970** | 0.000 | 0.000 |
| medyan | **1.000** | 0.920 | 0.125 |
| ≥0.99 olan koşu | **28/30** | 11/30 | 2/30 |
| <0.50 olan koşu (felaket) | **0/30** | 8/30 | 20/30 |

* Kama'nın **en kötü koşusu (0.970), quinn'in medyanının (0.920) ÜSTÜNDE.**
* `P(Kama koşusu > quinn koşusu) = 0.789` · `P(Kama koşusu > TCP koşusu) = 0.958` (0.5 = fark yok).
* Kama hiçbir koşuda %50'nin altına düşmedi; quinn 8, TCP 20 koşuda düştü.

### Aktivasyon (medyan) — ÖN-KAYITLI BEKLENTİ YANLIŞLANDI

```
parity_emitted = 0   parity_token_denied = 0   ← yapı-gate HİÇ açılmadı
retx = 95   dup = 9   tail_probe = 11   recover_arq = 8   state_transitions = 18
srtt = 99.2 ms   btlbw = 8.1 kB/s   fwd_loss = 496‰ (enjekte %50)   ret_ack_loss = 658‰
tel: data = 28 300 B   retx = 26 427 B   parity = 0 B
```

Ön-kayıt §9'da T1a için *"yapı-gate açılır → parity; spekülatif L1/L2"* yazmıştım.
**Ölçüm bunu yanlışladı:** %50 bağımsız kayıpta parity ne ÜRETİLDİ ne de ÖNERİLİP REDDEDİLDİ
(`denied = 0`). Kama bu hücreyi **saf ARQ ile, sıfır FEC ile** kazandı. Faz 1–4 boyunca
kablolanan kodlama alt-sistemi bu hücreye hiçbir katkı yapmadı. Tahmin geriye dönük
düzeltilmiyor; sapma olduğu gibi kaydediliyor.

Bağlam (Aşama A gözlemiyle tutarlı): iid kayıp hücrelerinde yapı-gate kapalı kalıyor
(T1a %50 → parity 0, T1b %80 → parity 0), buna karşılık korelasyonlu/gemodel hücrede açılıyor
(T2a → parity 22, denied 1943). Bu, `Q18Iid` yapı politikasının bir IID-yapı dedektörü olmasıyla
tutarlıdır. **Faz-5'te ayarlama YAPILMAZ**; T2 sonucuyla birlikte sınıflandırılacaktır.

### Tel maliyeti
kama data 28.3 kB + retx 26.4 kB ≈ **54.7 kB** · quinn **56.5 kB** · tcp 26.8 kB (uygulama
katmanı; çekirdek trafiği görünmez, DOĞRUDAN kıyaslanamaz — metodoloji §10).
Kama, quinn'den **%3 daha az** tel harcayarak completion'ı 0.900 → 1.000'e çıkardı.

### Anlam farkı (NON-NORMALIZED — zorunlu beyan)
quinn ve TCP **tam sıralı akış** garantisi verir; Kama **mesaj granülünde sırasız** teslim eder.
Kama'nın üstünlüğünün bir kısmı doğrudan bu **daha zayıf garantiden** gelir: sıralı akışta tek
bir kayıp ardındaki tüm teslimleri bloklar (head-of-line), mesaj granülünde bloklamaz. Bu fark
normalize EDİLMEMİŞTİR ve edilmemelidir; sonuç bu beyanla birlikte okunmalıdır.

### T1a HÜKMÜ: **KAMA DOMINATES**
Birincil metrikte üç deadline'da da her iki rakibi geçti; en kötü koşusu quinn'in medyanının
üstünde, felaket koşusu yok (0/30 vs 8/30 ve 20/30). Üstünlük **kodlamadan değil, ARQ +
mesaj-granülü tesliminden** geliyor — bu, hücrenin en önemli bulgusudur.
