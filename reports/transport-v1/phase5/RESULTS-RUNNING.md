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
