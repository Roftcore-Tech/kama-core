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

---

# CORRIGENDUM — 2026-09-04, adversaryal denetim sonrası

Bu rapordaki iki iddia YANLIŞTI ve geri çekiliyor. 36-ajanlı adversaryal denetim (16 iddia,
5'i çürütülemedi) ile ek netem koşuları bunları ayrı ayrı düşürdü. Sessizce düzeltilmiyor —
yanlış iddia ve gerekçesi kayda geçiyor.

## GERİ ÇEKİLDİ 1 — "bağımsız teyit" vakumdu

Yukarıda sonuç zincirinin 2. ve 3. maddeleri `ledger_loss_ppm_peak = 0` ve
`structure_gate_opened = 0` sayaçlarını "reponun KENDİ bağımsız sayacı da aynı şeyi söylüyor"
diye sundu. Bu **geçersizdir**: `sender.rs:3160` `return d;` satırı
`if self.fusion_policy.is_enabled()` bloğunun içindedir ve her iki sayaç (`:3168`, `:3177`) o
dönüşün ALTINDADIR. Raporun dayandığı üç pilot koşusunda politika AÇIKTI → iki sayaç da
yapısal olarak sıfırdı ve kayıp sinyali hakkında **hiçbir bilgi taşımıyordu**.

Bu, tam olarak bu reponun defterindeki *vakum-sayaç* tuzağıdır; raporu yazarken ona düştüm.

**Ayakta kalan kanıt:** `confirmed_total = 0` ve `ledger_int`'in `Confirmed = 0` alanı — bunlar
defterin kendisinden okunur, politika dalından etkilenmez.

## GERİ ÇEKİLDİ 2 — "hiçbir adaptif FEC yolu ateşleyemez" AŞIRI GENELDİ

Rapor şunu yazdı: *"bu repoda hiçbir ADAPTİF FEC yolu, hiçbir kayıp oranında ateşleyemez."*
**Yanlış.** Denetim doğru itiraz etti: ufuk (3.18 s) expire penceresinden (16.9 s) 5.3× KISA,
yani olay ufka ulaşmadan defterden düşmez — motor yeterince uzun tiklerse confirm ATEŞLER.

Tek-eksenli koşu (yalnız mesaj sayısı değişti, `k = 0` = SEVK DAVRANIŞI, aynı hücre):

| yük | `confirmed_total` | `ledger_peak` | `gate_opened` | parity (adaptif kol) |
|---|---|---|---|---|
| 96 msg (~1.9 s aktif) | 0 | 0 | 0 | **0** |
| 400 msg (~9.4 s aktif) | 339 | 132 000 | 0 | **0** |
| 900 msg (~18.2 s aktif) | 1023 | 271 327 | 9 270 | **792** |

Sevk edilen yapı, 900 mesajlık akışta adaptif parity BASIYOR. Doğru ve dar iddia şudur:

> **Kayıp sinyalinin her akışın başında ~16×RTT'lik bir ÖLÜ ZAMANI vardır** (kanaldan bağımsız
> 65 B / 31 B·s⁻¹ ≈ 2.097 s sabiti ×1.5 = ≥3.145 s, her kanalda). Bu süreden kısa akışlar
> kayıp sinyalini HİÇ görmez. Uzun akışlar görür.

Bu hâlâ ciddi bir kusurdur — taktik mesajlaşma tam olarak kısa akıştır — ama "asla ateşlemez"
değildir. Ayrıca reponun tüm FEC kampanyaları 96 mesajla koştu: o yükte sevk edilen adaptif kol
da, kanal-türevli kol da parity basmaz. "Adaptif kol hiçbir şey üretmedi" gözleminin sebebi
budur.

## AYAKTA KALAN ÇEKİRDEK

Mekanizma tespiti doğrulandı (denetimin bağımsız aritmetiğiyle de): ufuk her kanalda ≥3.145 s,
200 ms RTT'de 15.7×RTT. Kanal-türevli kol bu ölü zamanı ölçülü biçimde kısaltır — aynı 400
mesajlık yükte sevk kolu 0 parity basarken `k = 2000` **388 parity** basar (confirm 547,
`ledger_peak` 187 000).

## EK KISIT — tick penceresi kısmen HARNESS artefaktı

Rapordaki "ledger tick penceresi 2.3–5.2 s" ölçümü bench'in `drain_cap = Duration::ZERO`
ayarından etkilenir (`crates/bench/src/kama_product.rs`): bench'te kapanış-drenajı yoktur, ürün
default'u 3 s'dir. Yani ürün yolunda tick penceresi bench'tekinden ~3 s daha uzundur. Ölü-zaman
tespitini değiştirmez (ufuk yine ≥3.145 s), ama "pencere ufuktan kısa" ifadesi bench koşusuna
özeldir ve öyle okunmalıdır.

## DENETİMİN AYRICA BULDUĞU — bağımsız kusur, bu raporun konusu değil

**Monoton olmayan ölü bölge:** `loss_ppm ∈ [200 200, 212 599]` aralığında fuzzy 1..62‰ parity
planlar; `compute_requested_r` bunu K=8'de r=0'a yuvarlar, AMA sıfırdan farklı olduğu için
`structure_gate_fec` devre dışı kalır (`sender.rs:3201`: `if fuzzy_fec_permille != 0 → NotApplicable`)
→ **hiç parity yok**. Aynı akış 200 000 ppm'de r=4 alırdı. Yani kayıp ARTARKEN koruma DÜŞÜYOR.
Ayrı bir kart olarak ele alınmalı.
