# BULGU — kayıp-defteri `Suspected → Confirmed` geçişi ürün yolunda HİÇ ateşlemiyor

**Tarih:** 2026-09-04 · **Dal:** `dev/product-transport-fusion` · **Statü:** kök-neden ölçüldü, DÜZELTME YAPILMADI (yetki dışı)

Bu bulgu, deadline-ekseni işini doğrulamak için koşulan ön-uçuş pilotunda çıktı. Kampanya
kolunu ölçmeden önce "kol gerçekten farklılaşıyor mu" kapısını koşmak zorunluydu (v1'de bu
kapıyı atlamanın bedeli ödenmişti); kapı, kampanyadan çok daha büyük bir şeyi yakaladı.

## Ölçülen olgu

`kama-product` ürün yolu, rootless üç-namespace netem harness'ı, 1 mbit · ölçülen RTT 200 ms ·
%20 ileri kayıp, 96 mesaj × 256 B. Üç koşu (iid20 ×2, cor20 ×1):

| ölçüm | iid20 | cor20 | iid20-b |
|---|---|---|---|
| ilan edilen kayıp (`lost_units`) | 100 | 48 | 47 |
| defterdeki olay | 131 | 57 | 54 |
| `Suspected` birim | 100 | 48 | 47 |
| **`Confirmed` birim** | **0** | **0** | **0** |
| **`confirmed_total` (koşu boyu)** | **0** | **0** | **0** |
| `ledger_tick` çağrısı | 5 992 | 3 058 | 3 883 |
| en yaşlı gözlenen `Suspected` | 2.10 s | 1.88 s | 2.69 s |
| confirm ufku (`confirm_age`) | 3.18 s | 3.23 s | 3.23 s |
| expire ufku (`w_loss`) | 16.9 s | 17.2 s | 17.2 s |
| ledger tick penceresi | 5.24 s | 2.31 s | 3.12 s |

Kayıp DOĞRU tespit ediliyor (100 birim ilan). Hiçbiri `Confirmed`'a geçmiyor.

## Neden önemli — sonuç zinciri (her halkası ölçülü)

`loss_windowed_ppm()` YALNIZ `Confirmed` birimleri sayar:

    ratio_ppm(confirmed_units, sent_window_units)

`Confirmed` hep 0 → **`loss_ppm` ürün yolunda yapısal olarak 0**. Ölçülen sonuçlar:

1. `loss_range = (0, 0)` — politikanın gördüğü kayıp, koşu boyunca sabit sıfır
2. `ledger_loss_ppm_peak = 0` — reponun KENDİ bağımsız sayacı da aynı şeyi söylüyor
3. `gate_opened = 0` — Q18 yapı-gate'i hiçbir an açmıyor
4. Kalibre füzyon politikası `ArqOnly` veriyor (kova 0), çünkü kayıp ekseni LOW okuyor

**Yani bu repodaki hiçbir adaptif FEC yolu, hiçbir kayıp oranında ürün yolunda ateşleyemez.**
Bu, "FEC faydası yalnız STATİK-r kollarında gösterilebildi, adaptif kol hiçbir şey üretmedi"
gözleminin açıklamasıdır.

Bu, Q18 teşhisini de DÜZELTİR. Kayıtlı teşhis "controller %20 kayıpta sıfır parity planlıyor"
idi ve sebep fuzzy diz-yerleşimine atfediliyordu. Ölçüm başka söylüyor: **girdi sıfır.** Dizi
nereye koyarsanız koyun, sıfır girdi sıfır parity verir.

## Mekanizma

    confirm_age = 1.5 × EAI
    EAI         = 2·unit/rate + EAI_SACK_WIRE_BYTES / EAI_RETURN_BW_PRIOR
                = 2·unit/rate + 65 B / 31 B·s⁻¹
                = 2·unit/rate + 2.097 s

İkinci terim **kanaldan bağımsız bir sabittir** (31 B/s'lik ters-yol bant prior'ı). Ölçülen
RTT 200 ms olan bir yolda confirm ufkunu ~3.2 s'ye, yani **16 × RTT**'ye çiviler. Ölçülen
ledger tick penceresi ise 2.3–5.2 s — ufkun kendisiyle aynı mertebede. Bir olayın confirm
eşiğine ulaşması için gereken süre, motorun o olayı hâlâ işlediği süreden uzun.

Bu, bu repoda zaten belgeli olan **saat-sınıfı** kusur ailesindendir: eşik ölçülen kanaldan
değil sabit bir bayt/bant prior'ından türetilmiş.

İki aday yol ayrımı ölçülmedi ve iddia edilmiyor: yaşlanan olaylar (a) tick penceresi bitene
dek eşiğe ulaşamıyor olabilir, (b) geç-ACK ile `Spurious`'a geçip Suspected havuzundan
düşüyor olabilir. Her iki okumada da ölçülen sonuç aynı: `confirmed_total = 0`.

## Neden burada DURUYORUM

`confirm_age` semantiğini değiştirmek ARQ motorunun kayıp-onay davranışını repo genelinde
değiştirir (INV-RATE-1 komşuluğu) ve bu oturumda verilen yetki deadline eksenidir. CLAUDE.md
§6: "Surgical edit: byte-layout / imza değişikliği talimatsız YAPMA."

Ayrıca: bu düzeltilmeden **politika kolu kampanyası koşulamaz.** Şimdi koşulsa null sonuç
üretir ve o null, bu reponun tarihsel hatasıyla birebir aynı biçimde "FEC işe yaramıyor" diye
okunur. Ölçüm aracı doğrulanmadan ölçüm yapılmaz.

## Kanıt

- Ham koşu çıktıları: `KP-CLIENT ... ledger_int=(...) confirm_path=(...) tick_span=(...)`
- Teşhis yüzeyleri (bu dalda eklendi): `ThresholdLossDetector::diag_ledger_internals`,
  `diag_confirm_path`, `diag_tick_span`; `Sender` ve `ClientStats.fate` üzerinden ürün yolunda
  okunabilir.
