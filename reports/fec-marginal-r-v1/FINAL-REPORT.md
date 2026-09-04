# FEC-MARGINAL-R-V1 — SONUÇ

**Bu, bu repodaki İLK ön-kayıtlı, ürün-katmanı, FEC kollu kampanyadır.** Ön-kayıt ve ön-uçuş
çıktısı ilk rep'ten önce, kendi commit'inde (`9a39af89`) donduruldu. Aşağıdaki her sayı o
sözleşmenin altında üretildi; hüküm kuralı koşudan önce yazılmıştı ve mekanik olarak uygulandı.

## Koşu bütünlüğü

- 240/240 rep `COMPLETED`, **0 geçersiz** (kuruluş, reassembly, chunk-drop kapılarının hepsi temiz)
- Kol farklılaşması doğrulandı: r0 `repairs_tx = 0` · r2 `4800` · r4 `9600`; `repair_missing = 0`
- FEC gerçekten kurtardı: alıcıda `fec_recovered_units` 737–782 (kol/hücreye göre)
- Süre: 877 s (~15 dk)

## Hüküm — birincil uç nokta (`d_msg = 4×RTT = 800 ms`, alıcı-taraflı teslim)

| hücre | kol | hüküm | Δ koşullu-completion | byte/teslim | kurtarılan |
|---|---|---|---|---|---|
| iid20 | r2 | **MARGINAL-R-FAYDALI** | +1.5 pp | +23% | 767 |
| iid20 | r4 | MARGINAL-R-ETKİSİZ | +0.9 pp | +48% | 737 |
| cor20 | r2 | MARGINAL-R-ETKİSİZ | +1.0 pp | +23% | 782 |
| cor20 | r4 | **MARGINAL-R-FAYDALI** | +1.2 pp | +47% | 781 |

Çöküş oranları hiçbir kolda CI-ayrık ARTMADI (Clopper-Pearson aralıkları örtüşüyor), yani
proaktif redundancy bu rejimde kuruluşa zarar VERMİYOR.

**Okuma:** FEC ölçülebilir biçimde çalışıyor, ama birincil uç noktada kazanç **1–1.5 puan**,
maliyet **%23–48**. Bu endpoint'te takas kötüdür. Fayda ilan edilen iki hücrede bile etki,
ölçüm çözünürlüğümüzün (MDE 0.5–1.1 pp) hemen üstündedir.

## Bulgu 1 — r=4, r=2'nin üstüne hiçbir şey koymuyor

| hücre | kol | parity gönderilen | kurtarılan birim | verim |
|---|---|---|---|---|
| iid20 | r2 | 4800 | 767 | **0.160** |
| iid20 | r4 | 9600 | 737 | 0.077 |
| cor20 | r2 | 4800 | 782 | **0.163** |
| cor20 | r4 | 9600 | 781 | 0.081 |

İki katı parity, **aynı ya da daha az** kurtarma; parity başına verim yarıya düşüyor.
%20 kayıpta **r = 2 verimli noktadır**; r = 4 saf wire maliyetidir. Bu, MDS beklentisiyle de
tutarlı: %20'de r=2 zaten blokların %67.8'ini kurtarılabilir kılıyor, r=4'ün eklediği %24.9'luk
blok payı pratikte ARQ'nun zaten kapattığı artıktır.

## Bulgu 2 — etki SIKI deadline'da yaşıyor (TANIMLAYICI, hüküm DEĞİL)

Ön-kayıt §5 uyarınca `d_msg = 4×RTT` birincil, diğerleri **tanımlayıcı** ilan edilmişti.
Post-hoc eğri (tüm geçerli repler):

| `d_msg` | cor20 r0 | cor20 r4 | iid20 r0 | iid20 r2 |
|---|---|---|---|---|
| 2×RTT | 0.555 | **0.776** (+22.1 pp) | 0.530 | **0.667** (+13.7 pp) |
| 4×RTT | 0.905 | 0.966 (+6.1) | 0.857 | 0.845 (−1.2) |
| 8×RTT | 0.925 | 0.975 | 0.875 | 0.850 |
| 16×RTT | 0.953 | 0.989 | 0.941 | 0.920 |

Sıkı deadline'da etki **bir mertebe** büyük; gevşek deadline'da kayboluyor, hatta iid20/r2'de
tersine dönüyor. Bu, "FEC ARQ'nun tur bütçesinin YETMEDİĞİ yerde kazanır" hipotezini destekler.

**BU BİR HÜKÜM DEĞİLDİR ve öyle alıntılanamaz.** Birincil uç nokta 4×RTT'de donmuştu; etkinin
2×RTT'de yaşadığını görüp hükmü oraya taşımak, bu reponun defalarca bedelini ödediği post-hoc
endpoint kaymasıdır. Ayrıca bu eğri TÜM geçerli repler üzerinden, birincil uç nokta ise
ÇÖKMEYENLER üzerinden hesaplanır — farklı popülasyonlar, doğrudan kıyaslanamaz.

Doğru kapanış: `d_msg = 2×RTT` birincil alınan İKİNCİ bir ön-kayıt (~15 dk).

## Bulgu 3 — eşlenmiş marjinal oranda yapı, FEC'i ARQ'ya bırakmayı GEREKTİRMİYOR

Hücreler marjinal kayıp oranı eşit (%20), yalnız yapı farklı. Korelasyonlu hücrede FAYDALI hüküm
r=4'te çıktı ve o hücrede çöküş oranı EN DÜŞÜKTÜ (1/40). Yani "burst yapı → FEC'i ARQ'ya bırak"
kuralı bu ölçümde **desteklenmiyor**.

Bu önemlidir çünkü sevk edilen yapı-gate tam o kuralı uygular ve gerekçesi Q17 holdout'una
dayanır — o holdout formel olarak GEÇERSİZDİR (zararlı-FP hücresinde hiç FEC kolu yoktu).
Şimdi elimizde, o kuralın aksini gösteren geçerli bir ölçüm var. Kuralın kendisi ayrı bir
ön-kayıtla ele alınmalıdır; bu kampanya politikayı BYPASS eder (kollar sabit-r basar).

## Bu kampanyanın CEVAPLAMADIĞI şeyler

- Sevk edilen FEC **politikası** hakkında hiçbir şey (kollar yapı-gate'i ve fuzzy'yi bypass eder)
- quinn'e karşı üstünlük (quinn kolu yok)
- %20 dışındaki kayıp, 200 ms dışındaki RTT, 1 mbit dışındaki bant, 256 B dışındaki yük
- Blackout/kesinti rejimi
- Sıkı-deadline etkisinin HÜKMÜ (yalnız tanımlayıcı)

## Provenans

- ön-kayıt + ön-uçuş + koşucu + analiz: commit `9a39af89` (koşu verisinden ÖNCE)
- ön-uçuş `sha256 = 98d36169571365376d3cbc1421887b32f97306c0ab6d945e6ea941484ae34bae`
- ham veri `runs/`, analiz çıktısı `ANALYSIS.txt`
- analiz deterministik: bootstrap seed = `SHA256(TAG‖cell‖arm)`, RNG kütüphanesi yok
