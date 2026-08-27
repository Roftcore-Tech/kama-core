# S2-REGION — Üstünlük-Bölgesi Haritası (SCREENING; v1+v1b birleşik)

**2026-08-27.** Harness `scripts/netem/s2_region.sh` (iki-netns netem; aynı link+mesaj-planı).
Kollar: kama = ÜRÜN runtime (`p1_endpoint`; completion = **acked**/units — client-görüşü, teyit-yolu
dahil), quinn = bench (completion = delivered/msgs — alıcı-görüşü). SCREENING: farklı binary'ler;
formal banka kapi-eval-v1'dir. Ham satırlar: `v1/rows.txt`, `v1b/rows.txt`. Sorular koşu-öncesi
yazıldı (`PRE-REG-LITE.md` + v1b-eki).

## A — %80 iki-yön kayıp @200kbit (p2-eşdeğeri; n=5 birleşik, 80 msg/1s/D=120s)

| kol | rep'ler | ortalama |
|---|---|---|
| quinn | 0, 0 | **0.0%** — kesin ölü (bankayla tutarlı) |
| kama-OFF (ARQ+tail) | 44,80,69,42,76 | 62.2/80 = **%78** (varyans büyük) |
| kama-FEC (static-r4) | 71,62,50,75,70 | 65.6/80 = **%82** |

**Hüküm:** Bölge-iddiası NET (quinn 0 vs Kama ~%80). **%90-barı bu harness'ta AÇIK kaldı** (%82;
OFF-FEC farkı n=5'te gürültü-içi). Metrik-notu: kama-sayımı ACKED — reverse-yol da %80 kayıplı,
teyit-zinciri (ACK/RECOVERED dönüşleri) boğuluyor; FEC'in forward-teslim katkısı client-görüşünde
sistematik EKSİK görünür. %90-sorusu delivered-metriğiyle (server-görüşü) yeniden değerlendirilmeli.

## B — Bölge-sınırı: quinn'in STOKASTİK-ÖLÜMÜ (loss × RTT @200kbit; 40msg/250ms/D=45s)

| hücre | kama-FEC (rep'ler) | quinn (rep'ler, v1+v1b) | quinn tam-sıfır |
|---|---|---|---|
| %40, 0ms | 40,39 (~%99) | 40,**0**,40,40,40 | **1/5** |
| %60, 0ms | 40,30 (~%88) | 9,40,24,38,40 | 0/5 (derin-değişken) |
| %60, 150ms | 37,36 (~%91) | **0**,40,**0**,37,**0** | **3/5** |
| %90, 0/150ms | 3-1/40 | 0-1/40 | ikisi de ölü (bu kısa-planın fiziksel tavanı) |

**Hüküm (white-paper ana bulgusu):** %40-60 kayıpta quinn'in kırılganlığı **bimodal** — ya tamamlar
ya HİÇ başlayamaz; RTT eklenince (%60+150ms) **5 koşunun 3'ü tam-sıfır**. Kama aynı hücrelerde HER
koşuda ≥%88 ve tutarlı. Ortalama-karşılaştırmasından güçlü iddia: **deterministik-hayatta vs
stokastik-ölüm** (taktik sahada "bazen hiç kurulamayan" bağ = kullanılamaz bağ).

## C — Kullanıcı-rejimi: yüksek-bant + yüksek-kayıp + yüksek-RTT (5Mbit, 300ms; İLK ölçüm)

| hücre | kama-FEC | quinn | not |
|---|---|---|---|
| %40, D=45s (gevşek) | 38.5/40 | 40/40 | bant bolsa quinn çok-tur ARQ ile KURTULUYOR |
| %60, D=45s (gevşek) | 39/40 | 35/40 | " |
| %40, **D=10s (sıkı/gerçek-zamanlı)** | 39.7/40 | 39.7/40 | %40 henüz ayırt etmiyor |
| %60, **D=10s** | 38,39,**0** → 25.4/40 | **0**,20,**0** → **6.7/40** | **ayrışma burada** |

**Hüküm:** Hipotez ("bant var, kayıp+gecikme yüksek → quinn işlevsiz") **deadline-bağımlı** çıktı:
gevşek deadline'da bol bant quinn'i kurtarır; **gerçek-zamanlı sınıfta (D=10s) %60+300ms'te quinn
pratikte işlevsiz (ort. %17; 3 koşuda 2 tam-sıfır), Kama 3'te 2 koşuda %95+**. Kama'nın 1/3
tam-sıfırı dürüst kayıt: sıkı-D + iki-yön-%60 + 600ms-RTT'de teyit-zinciri kurulamama riski bizde de
var (cold-start/teyit-yolu — bilinen DR-CS hattı). n=3; yönlü-bulgu, kesin-oran değil.

## Sınırlar
Screening (ön-kayıtlı-eş değil); n=2-5; tek makine; kama=acked vs quinn=delivered metrik-asimetrisi
(A-notu); l90 hücreleri kısa-planda taban. White-paper'da bu tablo "screening" etiketiyle ve
kapi-eval-v1 formal sonuçlarının YANINDA kullanılmalı.
