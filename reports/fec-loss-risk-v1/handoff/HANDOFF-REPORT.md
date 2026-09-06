# HANDOFF AİLESİ — spekülatif → kanonik devir teslim

**Soru (owner):** ilk `Confirmed` geldiğinde spekülatif kolun otoritesi değişince uzun akışta
recovery "cliff"i oluşuyor mu? **İnvaryant:** kanonik sinyalin gelmesi FEC kurtarmasını
kötüleştiremez.
**Kod:** `d5b81514` (c809069b + salt-gözlem zaman-serisi enstrümanı). **Beklenti kaydı:**
`EXPECTATION.md` — koşudan ÖNCE yazıldı. **Konfig:** RTT 200 ms · 1 mbit · reorder yok ·
msgs {96, 400, 1000} × kayıp {5, 10, 20} % × {OFF, ON} × n=3 · 50 ms örnekleme.

## Referans — MDS K=8 blok çözülebilirliği (hesap, ölçüm değil)

| kayıp | r=1 | r=2 | r=4 | Δ(r2→r1) |
|---|---|---|---|---|
| 5 % | 0.929 | 0.988 | 1.000 | −0.060 |
| 10 % | 0.775 | 0.930 | 0.996 | −0.155 |
| 20 % | 0.436 | 0.678 | 0.927 | −0.242 |

Klampın öngörülen bedeli: 250→125 ‰ (r=2→r=1) demotion, %10'da −15.5 pp, %20'de −24 pp blok
çözülebilirliği.

_(Sonuç bölümleri koşu bitince doldurulur.)_

## Düzeltme tasarımı (veri gelmeden yazıldı; uygulama ölçüm SONRASI)

**Kusurun kökü:** `speculative_propose` içinde `if loss_windowed_ppm() > 0 { cap = min(cap, bootstrap_cap) }`
— olgunluk = "pencerede en az bir Confirmed var". Bu, kanıtın *değişmesine* değil, bir onayın *var olmasına*
tepki verir. Kanonik kol bu bant rejiminde hiçbir şey ÖNERMEZKEN (fuzzy 0, gate ≥100k'a ulaşmıyor)
spekülatif kol yine de düşürülür → devir teslim alıcısı olmayan bir devir.

**Kural (owner semantiği, doğru uygulanmış):** *otorite ancak devralan biri VARSA devredilir.*
Spekülatif tavan, kanonik lane fiilen bir öneri ürettiğinde (`confirmed_prop > 0 ∨ structure_prop > 0`)
`bootstrap_cap`'e iner; aksi hâlde kanıt (persistent risk) neyi destekliyorsa o kalır. `max()` zaten
kanonik öneri daha büyükse onu seçer; klamp yalnız "kanonik küçük ama var" durumunda spekülatifi
frenler — "kanonik hiç yok" durumunda değil.

**Alternatif (daha basit):** klampı tamamen kaldır; `max()` tek başına devir teslimi tanımlar.
Ölçüm iki alternatifi de ayırt etmez (her ikisi de bu ailede aynı davranır); tercih tasarım sorusudur.

**Enstrüman notu:** `reason_idx = SpecPersist` klamp SONRASINDA da görünür — sebep seviyeyi söyler,
miktarı değil. Devir-teslim analizinde `spec_prop` değerine bakılmalı, sebebe değil.
