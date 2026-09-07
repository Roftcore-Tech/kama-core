# ÖLÇÜM — "kanal-türevli yapılırsa gerçekten düzelir mi?"

**Tarih:** 2026-09-04 · **Dal:** `dev/product-transport-fusion` · **Tip:** TEŞHİS koşusu
(tek hücre, az rep). **PR-doğrulama kampanyası DEĞİLDİR.**

Soru soruldu, varsayılmadı. Tek eksen: `loss_confirm_k` (0 = sevk davranışı, >0 = `k × srtt`
per-mille). Kayıp TESPİTİ eşiği ve expire penceresi DEĞİŞMEDİ — yalnız `ledger_tick`'in confirm
argümanı. Hücre: rootless 3-namespace netem, 1 mbit, ölçülen RTT 200 ms, %20 ileri / %2 geri.

## Bulgu 1 — confirm yolu açılıyor (3 rep medyanı)

| kol | `confirmed_total` | confirm ufku | teslim | `tx_data` | spurious |
|---|---|---|---|---|---|
| k=0 (sevk) | **0** | 3247 ms | 96/96 | 42 816 B | 0 |
| k=1000 | 114 | 205 ms | 96/96 | 42 816 B | 0 |
| k=2000 | 133 | 409 ms | 96/96 | 42 816 B | 0 |
| k=4000 | 102 | 819 ms | 96/96 | 42 816 B | 0 |

## Bulgu 2 — çıkan değer DOĞRU

Politika kolu (tek fark `k`), aynı yükte:

| kol | `loss_ppm` (koşu tepesi) | yer-gerçeği (netem) | sapma |
|---|---|---|---|
| pol_k0 | **0** | 200 000 | −%100 |
| pol_k2000 | **183 333 – 195 833** | 200 000 | **−%2 … −%8** |

Sinyal yalnız sıfırdan çıkmıyor, **doğru büyüklüğe** oturuyor.

## Bulgu 3 — zincir uçtan uca kapanıyor, AMA yük penceresine bağlı

Politika kolu, `k = 2000`, aynı hücre; TEK EKSEN sunum penceresi:

| sunum penceresi | `fec_blocks` | parity gönderilen | latch (enqueued, emitted) |
|---|---|---|---|
| 96 msg (~1.9 s) | 0 | **0** | (0, 0) |
| 400 msg (~8 s) | 147 | **147** | (147, 147) |
| 900 msg (~18 s) | 339 | **435** | (435, 435) |

Politika histogramı `pol_k2000`: `ArqOnly = 2306`, **`FecAssist = 1616`** (k=0'da FecAssist = 0).

**Bu, bu repoda adaptif FEC'in ürün yolunda ilk kez tel üstüne parity koyduğu ölçümdür.**

### Ama 96 mesajlık yükte parity SIFIR — ve bu bir teşhis, kusur değil

Latch bastırma sayaçlarının hepsi 0 ve `suppression = None` → latch koştu, her blok kapanışında
oran 0'dı. Sebep zamanlama: 480 birim (60 blok) sunum penceresinde (~1.9 s) kapanıyor; kayıp
sinyali ise confirm ufku (~0.4 s) + birikme süresi kadar SONRA olgunlaşıyor. Karar geldiğinde
korunacak blok kalmamış oluyor.

**Bunun sonucu ağır:** reponun TÜM FEC kampanyaları 96 mesajlık yükle koştu. O yükte adaptif FEC
YAPISAL OLARAK ateşleyemez — ufuk düzeltilse bile. "Adaptif kol hiçbir şey üretmedi" gözleminin
ikinci sebebi budur ve kampanya tasarımına aittir, motora değil.

## Bulgu 4 — regresyon gözlenmedi

`teslim = 96/96` ve `tx_data = 42 816 B` DÖRT kolda da birebir aynı. Kayıp sinyali sıfırdan
çıkarken gönderim hızı düşmedi (INV-RATE-1 bu ölçümde korunuyor). `spurious = 0`.

## ÖLÇÜLMEYENLER — bu bir sevk kararı DEĞİLDİR

1. **Yanlış-confirm bedeli ÖLÇÜLMEDİ.** Bu hücrede netem yalnız kayıp + sabit gecikme uygular,
   yani REORDER YOK; `spurious = 0` bu yüzden beklenen sonuçtur, ufkun güvenli olduğunun kanıtı
   DEĞİLDİR. Kısa ufkun asıl bedeli reorder'lı hücrede ölçülür. **Sevkten önce zorunlu.**
2. **`k` kalibre değil.** 1000/2000/4000'in üçü de ateşledi; hangisinin doğru olduğu ölçülmedi.
3. **Tek hücre, 2–3 rep, tek RTT, tek bant, tek yük profili.**
4. **`loss_ppm` tüketicilerinin tam denetimi bitmedi** — bu sinyal bugüne dek hep 0'dı, dolayısıyla
   onu okuyan kod ürün yolunda hiç koşmamıştı. Hangi yolların ilk kez aktifleştiği ve INV-RATE-1'i
   ihlal eden bir yol olup olmadığı ayrıca denetleniyor.
5. **900 mesajta `loss_ppm = 289 190`** — netem'in %20'sinin üstünde. Muhtemelen sunum hızı
   kapasiteyi aştığı için eklenen tıkanıklık kaybı; DOĞRULANMADI.

## Kod durumu

`LossConfirmHorizon::{CadenceLegacy (default), ChannelDerived { k_permille }}` bir ÖLÇÜM KOLUDUR.
Default sevk davranışıdır ve byte-özdeştir (ölçüldü: dört kolda aynı `tx_data`). Sevk kararı
yukarıdaki 5 maddeye bağlıdır.
