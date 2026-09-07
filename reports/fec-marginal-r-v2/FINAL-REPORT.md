# FEC-MARGINAL-R-V2 — SONUÇ: sıkı deadline'da FEC ödüyor

Ön-kayıt commit `73e21a5` (ilk rep'ten önce). 240/240 rep `COMPLETED`, **0 geçersiz**.
Kol farklılaşması: r0 `repairs_tx = 0`, r2 `7200`, `repair_missing = 0`. Süre ~15 dk.

## HÜKÜM (birincil `d_msg = 2×RTT = 400 ms`, çöküş çıpası 16×RTT)

| hücre | completion r0 → r2 | Δ | CI-ayrık | çöküş | byte/teslim | hüküm |
|---|---|---|---|---|---|---|
| iid20 | 0.569 → 0.673 | **+10.4 pp** | EVET | 1/60 → 3/60 (artmadı) | +23% | **FAYDALI** |
| cor20 | 0.555 → 0.747 | **+19.2 pp** | EVET | 3/60 → 4/60 (artmadı) | +24% | **FAYDALI** |

Bootstrap %95 aralıkları tamamen ayrık (iid20: [0.534,0.596] → [0.601,0.736];
cor20: [0.505,0.596] → [0.702,0.780]). Çöküş oranı hiçbir kolda CI-ayrık artmadı.

**Bu, v1'in TANIMLAYICI bulgusunun BAĞIMSIZ veriyle replikasyonudur.** v1 aynı deseni post-hoc
görmüştü (cor20 +22.1 pp, iid20 +13.7 pp); v2 onu ön-kayıtlı birincil uç noktada, yeni replerle,
aynı yönde ve benzer büyüklükte doğruladı.

## Asıl sonuç: FEC'in değeri DEADLINE'a bağlı, kayıp oranına değil

Aynı kanal, aynı kol, aynı byte — yalnız skorlama eşiği farklı:

| `d_msg` | Δcompletion (r0 → r2) | takas |
|---|---|---|
| **2×RTT** | **+10 … +19 pp** | +24% byte → **iyi** |
| 4×RTT (v1 birincili) | +1.0 … +1.5 pp | +23% byte → **kötü** |
| 16×RTT | ≈0 veya negatif | saf israf |

ARQ'nun her kurtarma turu ≥1 RTT'dir. Deadline birkaç RTT'lik bütçe tanıyorsa ARQ zaten yetişir
ve parity israftır; bütçe ~2 RTT'ye inince ARQ'nun turu SIĞMAZ ve kodlama tek çalışan mekanizma
olur. Bu, "FEC yüksek kayıpta açılır" sezgisinin YERİNE geçen kuraldır: **eksen kayıp oranı değil,
deadline/RTT oranıdır.**

## r seviyesi: v1'den taşınan karar

v1 ölçtü: r=4, r=2'nin üstüne kurtarma EKLEMEZ (781 vs 782) ama iki katı parity gönderir
(verim 0.16 → 0.08). v2 bu yüzden r=4'ü ön-kayıtlı olarak düşürdü. **%20 kayıpta r=2 verimli nokta.**

## Yapı ekseni

Marjinal oran eşitken (%20) korelasyonlu hücrede etki DAHA BÜYÜK (+19.2 vs +10.4 pp) ve çöküş
artmadı. "Burst yapı → FEC'i ARQ'ya bırak" kuralı bu ölçümde desteklenmiyor — v1 ile aynı yönde.

## AKTİVASYON İÇİN SONUÇ — ve ortaya çıkan tasarım boşluğu

Ölçülen aktivasyon bölgesi: **d_msg ≲ 2×RTT · kayıp ~%20 · r = 2**. Bunun dışında FEC israf.

**Ama motor uygulamanın deadline'ını BİLMİYOR.** `RuntimeConfig`'de teslim-aciliyeti/deadline
ifade eden bir alan yok; `FusionObservation` da onu taşımıyor. Yani ölçülen kural bugün
KABLOLANAMAZ: kayıp oranından FEC açmak, kanıtın söylediği şey DEĞİL.

Bu kampanyanın ürüne dönüşmesi için gereken minimum kablolama:
1. `RuntimeConfig` (veya gönderim yolu) **mesaj-başı teslim bütçesi** taşısın.
2. `FusionObservation`'a `deadline_over_srtt` türetilmiş alanı eklensin (prefix-kausal: srtt zaten
   ölçülü, deadline app'ten gelir).
3. `FusionPolicy` kalibrasyonu bu eksende tanımlansın — kablo ve bağımsız güvenlik doğrulayıcısı
   HAZIR; eksik olan yalnız girdi.

## CEVAPLANMAYANLAR
%20 dışı kayıp · 200 ms dışı RTT · 1 mbit dışı bant · 256 B dışı yük · blackout · sevk edilen
politika (kollar bypass eder) · quinn kıyası.
