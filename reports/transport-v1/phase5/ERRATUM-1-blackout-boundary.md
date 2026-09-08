# ERRATUM-1 — Blackout metriğinde ufuk sınırı artefaktı

Durum: **DÜZELTİLDİ.** Etkilenen ölçümler **GEÇERSİZ** kılındı ve sıfırdan yeniden koşuluyor.
Tespit: Faz-5 Aşama B, T0 (kontrol hücresi) tamamlandıktan sonra, ilk sonuç okumasında.
Faz-4 metodoloji sözleşmesi §11'de tanımlanan blackout metriğini etkiler.

---

## Ne oldu

T0 kontrol hücresinde **üç kolun da completion(D) değeri 1.000** (IQR [1.000,1.000],
CI [1.000,1.000], N=30) iken sonuç sınıfları şöyleydi:

| kol | outcome | blackout insidansı | en uzun blackout |
|---|---|---|---|
| kama-v1-full | blackout=16 completed=14 | **16/30** CI[0.343,0.717] | 0.089 s |
| quinn-reliable-stream | blackout=1 completed=29 | 1/30 CI[0.001,0.172] | 0.042 s |
| tcp-default | completed=30 | 0/30 CI[0.000,0.116] | — |

**1 saniyelik pencere tanımıyla 0.089 s'lik bir blackout ÇIKAMAZ.** Bu, hükme varmadan önce
enstrümanı doğrulama kuralını tetikledi.

## Kök neden (gerçek koşu verisinden doğrulandı)

`blackout_scan` ufku `max(son_gönderim, son_teslim)` alıyordu. Ufuk son teslime **eşit**
olduğunda son pencere `[t, ufuk)` yarı-açıktır ve:

* teslimi **saymaz** — `d < end` yanlıştır (d == end),
* aynı mesajı **"bekleyen iş" sayar** — `delivered_ns >= end` doğrudur.

→ `teslim=0 ∧ bekleyen=var` → **sahte blackout**.

Gerçek kampanya verisinden vaka (T0/rep11, 100/100 teslim): son gönderim 4.952 s, son teslim
5.021 s, ufuk 5.021 s, son pencere `[5.000, 5.021)` → teslim sayısı 0, bekleyen var → blackout.

## Neden yalnız bir sayaç hatası değil — YANLI ölçüm

Artefakt yalnız akışın kuyruğu bir pencere sınırını **aştığında** ateşler. Kama'nın son teslimi
sistematik olarak 5.02 s civarındaydı (sınırın hemen ötesi), quinn'inki 4.99 s (hemen berisi).
Sonuç: aynı fiziksel davranış için **Kama 16/30, quinn 1/30, TCP 0/30**. Ölçüm, kuyruğu saniye
sınırına denk gelen kolu cezalandırıyordu. Bu bir taşıyıcı özelliği değil, **yönlü ölçüm
yanlılığıdır** ve blackout ∪ timeout ∪ connection-failure olarak tanımlı `stall` katmanlamasını
da kirletir.

`completion(D)` (BİRİNCİL metrik) etkilenmemiştir; etkilenenler: sonuç sınıfı dağılımı,
blackout insidansı ve süreleri, stall insidansı ve `stall-free` katmanı.

## Düzeltme (en küçük müdahale)

`crates/bench/src/outcome.rs`, `blackout_scan`:

```rust
// önce
let horizon = last_delivered.unwrap_or(last_sent).max(last_sent);
// sonra
let horizon = last_delivered.map_or(last_sent, |d| last_sent.max(d.saturating_add(1)));
```

Son teslim kendi penceresinin **içinde** kalır ve artık "bekleyen" sayılmaz. Gönderim-sonrası
sessizlik ufku **uzamaz**, dolayısıyla "teslim bir daha dönmedi" vakası aynen korunur.

Transport kodu DEĞİŞMEDİ: müdahale yalnız ölçüm katmanındadır (`crates/bench`).

## Doğrulama

1. **Regresyon testi** eklendi: `son_teslim_pencere_sinirini_asinca_sahte_blackout_yok` —
   vakayı gerçek kampanya verisinden (100 mesaj, 50 ms aralık, +70 ms teslim → son teslim
   5.020 s) birebir yeniden üretir.
2. Mevcut üç blackout testi **değişmeden geçiyor** → gerçek kesinti tespiti zayıflamadı
   (`ortadaki_kesinti_yakalanir_ve_kurtarma_olculur`, `kurtarma_yoksa_none`,
   `akis_sonu_sessizligi_blackout_degil`).
3. **Arşivlenmiş geçersiz veri üstünde yeniden hesap** (aynı ham koşular, düzeltilmiş kod):

   | kol | önce | sonra | completion |
   |---|---|---|---|
   | kama-v1-full | blackout 16/30 | **0/30** | 1.000 (değişmedi) |
   | quinn-reliable-stream | blackout 1/30 | **0/30** | 1.000 (değişmedi) |
   | tcp-default | blackout 0/30 | 0/30 | 1.000 (değişmedi) |

## Geçersiz kılınan ölçümler

Aşama B'nin ilk koşumunun tamamı: **T0 (90 koşu, tamamlanmış) + T1a (kısmi)**.
Silinmedi, kanıt zinciri için arşivlendi:
`/home/ryuzaki/kama-phase5-results/stageB-INVALIDATED-blackout-boundary/`

Düzeltme-öncesi ve düzeltme-sonrası tekrarlar **aynı N≥30 örnekleminde BİRLEŞTİRİLMEZ**.
Kampanya yeni SHA ile **sıfırdan** başlatılmıştır.

## Ön-kayıt üstündeki etkisi

`EXPECTATION.md` **değişmedi**. Hücre tanımları, geometri, deadline, N kuralı, birincil metrik
ve hüküm eşikleri aynen geçerlidir. Düzeltilen şey ön-kayıtlı bir seçim değil, o seçimi ölçen
kodun bir hatasıdır — sözleşmenin *"metodolojinin kendisinde gerçek bir hata varsa: DUR, düzelt,
ERRATUM yaz, etkilenenleri geçersiz kıl, o hücreleri baştan koş"* maddesi uyarınca işlem yapıldı.

Sonuçlara bakılarak yapılmış bir değişiklik DEĞİLDİR: hata, ilk hücrenin completion'ı üç kolda
da 1.000 iken, yani hiçbir kıyas hükmü kurulmadan önce yakalandı.
