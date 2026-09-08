# FAZ 5 — AŞAMA A (SMOKE) HARNESS DOĞRULAMA

> **Bu belge hiçbir performans hükmü İÇERMEZ.** Aşama A'nın tek işlevi araç doğrulamadır
> (harness, manifest, aktivasyon, rakip yürütme, hücre tanımı). Hüküm sınıfı: `smoke`.
> Ön-kayıt: [`EXPECTATION.md`](EXPECTATION.md) · commit `b1049e3956de6987b6f43adcaab291607ecc359e`

Koşum: 11 kıyas hücresi × 3 kol × 4 tekrar, D = 10 s, 4 eşzamanlı shard.
Süre: 8 dk 16 sn (2026-09-08T13:43:17Z → 13:51:33Z). 11/11 shard `DONE`.

---

## A1 — Artefakt bütünlüğü ve profil kimliği

11 hücrenin tamamı, üç kolun tamamı için 4/4 `out.jsonl` **ve** 4/4 `manifest.json` üretti.
Kama kolunun kimliği her hücrede `kama-v1-full`, sapmalı koşu sayısı **0**.
→ Faz-5 çıkış ölçütü 4 (her sonuç kanonik profilden) Aşama A'da sağlanıyor.

## A2 — Bozulma gerçekten uygulandı mı

Her hücrede iki yön için `tc qdisc show` çıktısı arşivlendi (`realized_fwd.txt`,
`realized_rev.txt`) ve shard başlamadan önce barrier ile doğrulandı (netem yoksa shard
`BARRIER-FAIL` ile durur, ölçüm üretmez). Örnek: T6 `fwd: loss 5% rate 100Kbit` /
`rev: loss 80% rate 100Kbit` — iki yön AYRI ve ön-kayıtla birebir.

## A3 — YÖN AYRIMI ÇÜRÜTME TESTİ (EXPECTATION §11) — **GEÇTİ**

Ön-kayıt: *"T6'da `fwd_loss_permille` yükselirse (ACK kaybı ileri kayıp gibi okunuyorsa) →
INV-RETURN-1 ihlali → WIRING/IMPLEMENTATION BLOCKER."*

| hücre | enjekte ileri | ölçülen `fwd_loss` | enjekte dönüş | ölçülen `ret_ack_loss` |
|---|---|---|---|---|
| T6 | %5 | **68‰ (≈%6.8)** | %80 | **824‰ (≈%82)** |
| T5 | %0 (yalnız bant asimetrisi) | 19‰ | %0 | 0‰ |
| T7 | %20 | 242‰ | %2 | 0‰ |
| T0 | %1 | 7‰ | %1 | 0‰ |
| T1a | %50 | 510‰ | %50 | 680‰ |

T6'da dönüş yolunun %82'si kaybolurken ileri kayıp tahmini %6.8'de kaldı: **bozuk geri besleme
ileri DATA kaybı gibi okunmuyor.** Çürütme gerçekleşmedi → yön ayrımı için wiring blocker YOK.

**Kaydedilen sapma (hüküm değil):** T1b'de enjekte %80 iken ölçülen `fwd_loss` 399‰. Tahminci
bir-ACK-gecikmeli kümülatif olduğu için yapısal olarak MUHAFAZAKÂRDIR; bu bir eksik-tahmin
yönünde sapmadır ve Aşama B'de n=30 ile birlikte raporlanacaktır.

## A4 — Rakip kolların yürütülmesi

Üç kolun tamamı 11 hücrede rc=0 ile koştu; **hiç harness arızası yok**. Ağır hücrelerde
rakiplerde `connection-failure` sonuç sınıfı görüldü (ör. T6'da quinn 4/4, T1b'de quinn 3/4).
Metodoloji §12 uyarınca bu **geçerli bir ölçüm sonucudur**, harness hatası değildir ve
agregaya girer. Aşama B'de insidansı güven aralığıyla raporlanacaktır.

## A5 — Kıyas adaleti

11 hücrenin tamamı **NON-NORMALIZED**; hiçbirinde **BLOCKING** yok.
Sebep sabit ve belgeli: Kama `reliable-messages` / sırasız, quinn ve TCP
`reliable-ordered-stream` / tam sıralı. Deney koşulları (yük, mesaj sayısı, deadline, sunum
aralığı, ileri ve dönüş bozulması, kurulum sınıfı) **eşleşiyor** — bu yüzden sayı üretilebilir,
ancak her tabloda anlam farkı beyan edilecektir.

## A6 — Aktivasyon kanıtı (mekanizmalar fiilen katıldı mı)

Ön-kayıtlı beklenti "T0'da ~0 parity" **doğrulandı**: T0 `parity=0`.
Kayıplı hücrelerde ARQ ve kodlama katıldı (T2a `parity=22`, T6 `parity=7`, T7 `parity=8`).

**Aşama B'de incelenecek iki gözlem (Aşama A'da hüküm verilmez):**
1. `parity_token_denied ≫ parity_emitted` üç hücrede belirgin: T1a 3514 vs 4, T7 2551 vs 8,
   T2a 1943 vs 22. Bu, Faz-3'te kaydedilen ve Faz-5 T2'nin cevaplamakla yükümlü olduğu sorudur.
2. **Monotonluk kaydı:** T1a (%50 kayıp) `parity=4` iken T1b (%80 kayıp) `parity=0` **ve**
   `denied=0`. Yani daha ağır kayıpta hiç parity ÖNERİLMEMİŞ. Aşama B bunu n=30 ile ölçecek;
   doğrulanırsa "kayıp artarken koruma düşüyor" sınıfı bir bulgudur ve taksonomi 1/2/5
   (implementasyon / wiring / yanlış default) altında sınıflandırılacaktır — tuning YAPILMAZ.

## A7 — Paralellik kararı: **SERİ KOŞULACAK**

Ön-kayıt (§13) 4 eşzamanlı shard'a izin veriyordu, *"completion medyanı ve srtt materyal olarak
ayrışırsa"* seriye dönme şartıyla. Ölçülen (T1a, n=4, D=10 s):

| kol | seri completion medyan | 4-paralel completion medyan | seri srtt | 4-paralel srtt |
|---|---|---|---|---|
| kama-v1-full | 0.860 (IQR 0.010–1.000) | 1.000 (IQR 0.990–1.000) | 40.4 ms | 59.8 ms |

`srtt` medyanı %48 kaydı. **Ancak bu kıyas KARIŞTIRILMIŞTIR (confounded):** her netns kurulumu
netem'e taze bir rastgele tohum verir (`tc qdisc show` çıktısında `seed …` alanı), dolayısıyla
n=4'te paralelliğin etkisi tohum varyasyonundan AYRILAMAZ. Ayrıca completion IQR'si (0.010–1.000)
tek başına devasa.

Karar: ayrım kanıtlanamadığı için **muhafazakâr dal seçildi — Aşama B tamamen SERİ koşar**
(`maxpar=1`). Bu aynı zamanda projenin kendi yerleşik disiplinidir
(`kapi_calrun.sh`: *"hücreler arası paralellik YOK — CPU/timing izolasyonu"*).
Bedeli: Aşama B ≈ 7.2 saat (ön-kayıtlı 10 saat bütçesi içinde).

---

## Aşama A hükmü

**AŞAMA A GEÇTİ — araç doğrulandı; Aşama B (N=30) koşulabilir.**

Sözleşmenin durdurma sebeplerinden hiçbiri gerçekleşmedi: harness arızası yok, profil uyumsuzluğu
yok, yanlış bozulma yok, eksik aktivasyon yok, rakip semantik hatası yok, kurulum muhasebesi
hatası yok. Aşama A'nın hiçbir sayısı performans kanıtı olarak KULLANILMAYACAKTIR.
