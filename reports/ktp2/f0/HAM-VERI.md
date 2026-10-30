# F0 ham veri ve komutlar

**Sapma beyanı:** iş emri ham veriyi raporlarla aynı dizinde istiyor. Ham koşu dizinleri ≈ 1,5 GB, sıkıştırılmış 182 MB olduğu için git'e girmedi. Git'te koşu başına türetilmiş CSV'ler, bütün betikler ve ham arşivin sha256 listesi (`ham-veri.sha256`) var. Ham veri bu makinede; depoya ya da başka bir yere taşınması sahip kararına bağlı.

## Yerler

| ne | yer |
|---|---|
| Ham koşular | `~/kama-k4g-work/ktp2-f0/runs/tam/{a,b,c,d}/out/<hücre>/<kol>/rep<N>/` |
| Hücre başına arşiv | `~/kama-k4g-work/ktp2-f0/arsiv/{a,b,c,d}/<hücre>.tar.zst` |
| Arşiv sha256 listesi | `reports/ktp2/f0/ham-veri.sha256` (git) |
| Kuyruk kaydı | `~/kama-k4g-work/ktp2-f0/runs/tam/ortak/{grid.log,liste.txt,NOT.md,env.txt}` |
| İkililer | `~/kama-k4g-work/ktp2-f0/bin/<kol>-<commit>/` (`SHA256SUMS`, `KAYNAK`) |
| Koşu başına CSV | `reports/ktp2/f0/{a,b,c,d}/tam/` (git) |

Her hücrenin `cell.env` dosyası hücre, koşucu ve harness özetini, ikili yollarını ve sha256'larını taşır.

## İkililer (rustc 1.96.0, cargo 1.96.0)

| ikili | commit (origin) | sha256 |
|---|---|---|
| bench, taban | `2cd9f68d` (`ktp2/f0-taban`) | `3311d8e0c6d0026a17f48eeeeaf09566741a4de5d60e7ce6a97d1c999ea0ecbe` |
| bench, aday | `bf18dc35` (`ktp2/f0-kol-05b`) | `c893e84047757007b9e7bdd4d330683de1a01cf4f671d1dfaa3f50accd4a2860` |
| f0_flows, taban | `1f6c3433` (`ktp2/f0-taban`) | `d330b82ee1fcee279cc4f5c9b6f5b49393521f99aad136c9c29094c14da1dabf` |
| f0_flows, aday | `0a251074` (`ktp2/f0-kol-05b`) | `0234015664f2029996a974ea467fd90bca44afa3d2dcf19d5b6aa65fb91eb4a0` |

Kollar: taban = main `a858f3fc` + ölçüm kodu; aday = `49864510` + aynı ölçüm kodu. Kabul denetimi: iki kol yalnız 05b'nin kendi farkıyla ayrılıyor (dosya kümesi ve içerik satırları).

## Koşu komutları

```
# Ortam: runs/tam/ortak/env.txt
bash scripts/ktp2/f0/f0_grid.sh -j 11 <kök> <KÖK::HÜCRE.cell …>     # liste: runs/tam/ortak/liste.txt
bash scripts/ktp2/f0/f0_grid.sh -j 1  <kök> scripts/ktp2/f0/cells/d/d_tcp_gso1_r12k_d300_l30.cell
```

Hücre dosyaları: `scripts/ktp2/f0/cells/{a,b,c,d}/`. Profiller ve kurallar: `scripts/ktp2/f0/KATALOG.md`. Kuyruk düzeltmesi (ayrı şerit kuyrukları → tek ortak kuyruk): `runs/tam/ortak/NOT.md`.

## Analiz komutları

Her şeridin analizi orkestratörde yeniden çalıştırıldı; çıktılar commit'lenen dosyalarla bit-bit aynı.

| şerit | komutlar | ayrıntı |
|---|---|---|
| A | `f0a_analyze.py`, `f0a_tam.py` | `a/RAPOR-A.md` TS.7 |
| B | `analiz_b.py`, `r25_kok.py`, `a10_kok.py`, `hiz_seri.py`, `tam_b.py` | `b/RAPOR-B.md` TS.7 |
| C | `tam_c.py`, `tam_c_ek.py` | `c/RAPOR-C.md` TS.7 |
| D | `tam_analiz.py`, `d13_analiz.py` | `d/RAPOR-D.md` §9.12 |
| E | `scripts/ktp2/f0/e/*.py` | `TASLAK-NOTLARI.md` başı |

## Geçerlilik özeti

| şerit | hücre | koşu | not |
|---|---|---|---|
| A | 12 | 470 | 2 koşu rc = 124 (rinf stres hücresi); oturum kopması 0 |
| B | 56 | 4920 | 2 hücre bütçe dışı, koşulmadı (`b_r25_d300_l0_p1000` ve quinn eşi) |
| C | 21 | 4160 | |
| D | 7 | 540 | D-13 denetim hücresi dahil |
| **toplam** | **96** | **10 090** | sessiz yeniden koşu yok |
