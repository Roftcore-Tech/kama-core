# EK-1 (ext1) — DURUM: SAHİP KARARIYLA DURDURULDU (2026-09-21 ≈ 01:00)

Dondurma `eba39ffb` sonrasında koşu başlatıldı (2026-09-20 23:30, 6 işçi) ve **R1'in 1614/3240 satırı üretilmişken sahip kararıyla durduruldu**; R2 hiç başlamadı.

**Gerekçe (sonuca bakılarak DEĞİL):** Kama ↔ quinn ↔ TCP sorusu Transport v1 kapanışında zaten ön-kayıtlı ve N=30 ile cevaplanmıştı
(`reports/transport-v1/PHASE5-T0-T7-REPORT.md`, 990 koşu; hüküm `TRANSPORT-V1-CLOSURE.md`). Ek-1'in Q bloğu aynı soruyu başka hücrelerde
yeniden soruyordu; B ve G blokları Faz-5 T1a/T3a ve T2a/T2b'ye benzerdi. Durdurma anında **hiçbir Ek-1 sonucu analiz edilmedi ve görülmedi**
(yalnız satır sayısı ve skip/error sayacı = 0 izlendi).

**Kısmi veri:** repo DIŞINDA, sahibin makinesinde `/home/ryuzaki/kama-kvt-results/ext1/` (R1 w0–w5 `result.csv` + koşu dizinleri). Analiz edilmedi, rapora GİRMEDİ,
hiçbir iddiaya dayanak yapılamaz (kısmi replika; hücre blokları işçi sırasına göre yarım).

**Manifest dosyaları repoda DEĞİL** (2026-09-23'te çıkarıldı, 1.4 MB): durdurulmuş bir ekin girdisi oldukları için ağaçta tutulmuyorlar. `EXT-FREEZE.md` §7'deki sha256'lar geçerli; üretici deterministik olduğu için yeniden üretim byte-özdeştir (komutlar orada). Devam edilecekse ÖNCE manifestleri yeniden üret.

**Devam etmek istenirse** (manifest ve analizör donmuş, koşu devam-edilebilir; üretilmiş satırlar yeniden koşulmaz):
`KVT_WORKERS=6 bash baselines/campaign/kvt_campaign.sh reports/kama-vs-tcp-v1/ext1 /home/ryuzaki/kama-kvt-results/ext1`

v1 kampanyası (7000 satır, `docs/reports/kama-vs-tcp-v1.md`) bundan ETKİLENMEZ; tamamdır.
