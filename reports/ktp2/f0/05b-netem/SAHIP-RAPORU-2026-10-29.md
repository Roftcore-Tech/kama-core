# #140 son aday 49864510: netem ön-kayıtlı kural DUR (2026-10-29)

## Özet
- Tam yerel kapı **26/26 yeşil**: test_fast 1683/0/18 ign, integration 386/0, `--locked`, systemd-analyze, Cargo.lock aynı, ağaç temiz.
- netem-7 (aynı ön-kayıt, 6d667853…): **p2_loss80'de KAYMA-M1, yönü artış.**
  - taban 1b1425d2 M1 medyanı 0,510; aday 0,915.
  - p = 0,005282 < α = 0,00714.
  - Diğer altı profil kayma yok.
- Ön-kayıt iki yönlü → **DUR, sahibe rapor, merge yok.**
- §1.3'ün istediği cümle ("ön-kayıtlı kuralla kayma saptanmadı") **bu aday için yazılamaz.** Dolayısıyla "rev6 kapsam içinde"nin üç koşulundan biri karşılanmadı.

## Sorgu (salt-okunur; yeni koşu yok)
Sıra ön-kayıttaki gibi: enstrüman → türetim → kod. Ayrıntılar: kanit-paketi/README.md §5.

**Enstrüman.**
- Hesap üç bağımsız uygulamada birebir aynı.
- 280/280 koşuda rc=0 ve 100 olay.
- Hash'ler, qdisc, kol sırası ve eşzamanlılık temiz.
- Dışlanamayanlar:
  - (i) M1'in ölçüm noktası 05b'de kodda değişti: delivered_ns artık take_confirmed_units'ten geliyor. Bunun M1'e katkısı ölçülmedi.
  - (ii) Dosyaya yazmayan CPU yükü.
- Karşı denetim: sunucunun D'siz teslim sayısı da aynı yönde (57 / 94,5; p = 0,0032).

**Kırılganlık.** p/α ≈ 0,74. Tek bir taban koşusunda tek mesaj farkı hükmü çevirirdi. Hodges–Lehmann +0,265, %95 [0,02; 0,54].

**Oturum değişkenliği.** Aynı taban ikilisinin p2 M1 medyanı iki oturumda 0,150 ve 0,510 (p = 0,41).

**Olası mekanizma [çıkarım; ablasyon yok].**
- 05b madde 7, yani RTT kabul kuralı. Tasarım notunun B.2.0'daki "srtt 60,5 s" kusurunun düzeltmesi.
- Koşu sonu anlık görüntüsünde taban srtt'si 0,5–10 s, adayınki ≈ 19 ms.
- Sahiplik maddeleri de aynı yönde olabilir; sayaçlarla ayrılamıyor.
- DUR-45 (C) p2'de etkili olamaz [kod].

**Benim hatam.** Ön-kayıt §3'teki "05b yalnız token yetmezliğinde devreye girer" gerekçesi eksikti; her ACK'te çalışan değişiklikleri saymadı. Kural ve hüküm geçerli.

**Maliyet gözlemleri [betimsel].**
- p2'de istemci gönderim hacmi taban 84,3 KB, aday 155,1 KB. Kopya sayısı ≈ 14 kat.
- p1 ve p5'te M2 oranı 1,37 ve 1,58: bant dışında ama p ≥ α. Yön oturum 1'de de > 1. Bu veri ne gecikme artışını ne de yokluğunu gösteriyor.

## Senden karar: DUR-46 önerisi
- **(a) Önerim:** p2 KAYMA-M1'i beyanlı sapma olarak kabul et ve merge yolunu aç. Gerekçeler:
  - mekanizma adayları 05b'nin onaylı kapsamı içinde;
  - sunucu sayacı aynı yönü gösteriyor;
  - enstrüman denetimi temiz.

  Ardından INT-02'den önce ayrı ve ön-kayıtlı bir kart açılır. İçeriği: p2'de ablasyon (taban / taban + yalnız madde 7 / 49864510, N = 30), p1/p5 M2 ve tel hacmi.

  Merge adımları: kanıt paketi commit → yeni SHA'da tam kapı → senin satırın → merge.
- **(b)** Merge'den önce ablasyon. "Taban + yalnız madde 7" için yeni dal ve şerit işi gerekir, yaklaşık 1 gün.
- **(c)** Yeni ön-kayıtla yedi profil, N ≥ 30, yeniden koşu. Yaklaşık 1,7 saat. Güç kazandırır ama mekanizmayı ayırmaz.

## Değişmeyen açık kalemler
Önceki raporda listelendiği gibi:
- DUR-45 (C) / (2′) / (C′) / (1)
- DUR-42 (b-dar)
- PRE_CONVERGE tavansız katlama
- Koruma yalnız lib'de
- 05g (e8852a87): rebase + kapı #140'tan sonra
- 05f rev3.2 (8ecd09d2)
- K4G-06 rev5.2 (24bc6314)

## Yapılmayanlar
- Dala commit yok.
- PR #140 değişikliği yok.
- Yeni koşu yok.
- Kanıt paketi yerelde hazır: ~/kama-k4g-work/k4g05b/kanit-paketi/ (oturum-1, oturum-2, sorgu/).
