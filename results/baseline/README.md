# CI-07 netem baseline

`compare.py --gate` bu dizindeki `kama-baseline.json`'a karşı koşar. Dosya YOKSA kapı
**düşer** — "ölçüm yoksa yeşil" davranışı BUL-01/BUL-02'nin ürettiği yanlış güvenlik hissidir
ve bu depoda yasaktır.

## Baseline nasıl üretilir

Docker daemon + `NET_ADMIN` yetkili bir makinede:

```bash
docker compose -f harness/docker-compose.yml build
REPEATS=5 bash harness/run-kama-matrix.sh
python3 harness/compare.py --emit-baseline results/baseline/kama-baseline.json
```

`--emit-baseline`, herhangi bir profilde `MIN_RUNS` (=3) repeat'ten az veri varsa **reddeder**;
eksik veriyle dondurulmuş bir baseline kapı değildir.

## Ne zaman tazelenir

`--gate` bir iyileşme raporladığında. Tazeleme **gerekçesiyle birlikte** commit edilir —
baseline'ı sessizce yükseltmek regresyon penceresini kaydırır.

## Statü

`[BEKLİYOR — Yaaz]` Baseline henüz üretilmedi: değerlendirme makinesinde çalışan Docker daemon
yoktu (SR-STR-CORE-001 §1.4 ortam kısıtı). `netem-gate` job'ı `[self-hosted, linux, netem]`
etiketli bir runner ister; runner atanana kadar nightly'de bu job **skip değil, fail** olur —
kasıtlıdır (ADR-18 "ZORUNLU KALİTE KAPISI").