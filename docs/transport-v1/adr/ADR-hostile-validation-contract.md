# ADR — Hostile doğrulama sözleşmesi (gelecek performans iddiaları için asgari metodoloji)

**Status:** ACCEPTED (Faz-6, 2026-09-11). Bağlayıcı: her gelecekteki "Kama v1 … kazanır/kaybeder"
iddiası bu sözleşmeyle üretilmek ZORUNDADIR; aksi hâlde iddia geçersizdir.

## Context

Faz-4 öncesi kıyaslar (a) FEC'siz stub kolla koşmuş, (b) ürün profilini ölçmemiş, (c) n=6
medyanlarını nihai iddia yapmıştı. Faz-5'te bu iddialardan biri (T2b "burst'te Kama yeniyor")
N=30'da yeniden üretilemedi ve geri çekildi. Metodoloji zayıflaması sessizce geri gelmemeli.

## Decision — dondurulmuş sözleşme

1. **Metodoloji belgesi:** `reports/transport-v1/BENCHMARK-METHODOLOGY.md` (Faz-4, 20 bölüm)
   otoritedir; sonuçlara bakılarak DEĞİŞTİRİLEMEZ. Hata bulunursa: DUR → ERRATUM → etkilenen
   koşular geçersiz → sıfırdan (ERRATUM-1 örneği).
2. **Profil kimliği:** yalnız `profile_delta = []` olan koşu `kama-v1-full`
   (`client_core::runtime::TRANSPORT_V1_PROFILE_ID`) etiketi taşır; sapma → `kama-custom`, ürün
   sonucu olarak kullanılamaz (INV-008/009).
3. **Ön-kayıt:** hücreler, geometri, deadline, metrik, N, çürütme ölçütleri ölçümden ÖNCE
   commit'lenir (`EXPECTATION.md` deseni). Sonuç görüldükten sonra hücre eklenmez/çıkarılmaz/
   düzenlenmez; kazanılan hücre seçilmez, kaybeden hücre uzatılmaz.
4. **N kuralı:** nihai iddia N ≥ 30 + güven aralığı; n < 30 = `smoke`, performans iddiası YASAK.
   İstatistik: medyan + IQR + sıra-istatistiği CI; oran için Clopper–Pearson; bootstrap/RNG yok.
5. **Sonuç sınıfları ve stall:** completed / deadline-miss / blackout / timeout /
   connection-failure / harness-failure / infra-failure; harness/infra N'e SAYILMAZ ama
   raporlanır; all-runs / stall-free / insidans birlikte.
6. **Adillik düzeyleri:** BLOCKING (koşul farkı → sayı üretilemez) · NON-NORMALIZED (anlam farkı →
   damga zorunlu) · NORMALIZED. Anlam eşitlenmez (`bench fairness`).
7. **T0–T7 rolü:** T0 regresyon kapısı; T1–T7 ilan edilmiş zarfın hücreleri
   (`EXPECTATION.md` §3, her biri repoda ZATEN var olan profilden). Yeni bozulma modeli icat edilmez.
8. **Rakip kimliği:** `quinn-reliable-stream`, `tcp-default` birincil; `quinn-datagram-app-retry`
   ayrı tablo. "Quinn"/"Kama" tek başına etiket olamaz.
9. **Gözlem penceresi simetrisi:** D ≤ 60 s (TCP kolunun `min(D,60 s)` tavanı; EXPECTATION §4).
10. **Hüküm sınıfları:** her hücre `PHASE5` sözleşmesindeki 15 sınıftan TAM BİRİNE düşer;
    "ilginç sonuç" hüküm değildir.

## Consequences

* Metodolojiyi zayıflatan bir kıyas otomatik olarak "Kama v1 iddiası" sayılmaz.
* Bir davranış değişikliği (EVOLUTION §9.5) etkilenen T-hücrelerini bu sözleşmeyle yeniden koşmadan
  `kama-v1-full` adını koruyamaz.

## Evidence

`BENCHMARK-METHODOLOGY.md`, `phase5/EXPECTATION.md`, `phase5/ERRATUM-1-blackout-boundary.md`,
`PHASE5-T0-T7-REPORT.md` §3–§6, §17.

## Revisit conditions

Yalnız metodolojide kanıtlanmış bir hata (ERRATUM yolu) ya da yeni bir hedef rejim eklenmesi
(yeni hücre = yeni ön-kayıt; mevcut hücreler değişmez).
