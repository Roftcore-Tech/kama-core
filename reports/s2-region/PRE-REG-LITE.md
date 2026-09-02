# S2-REGION — Üstünlük-Bölgesi Haritası (SCREENING ön-kaydı, koşu-öncesi)
**2026-08-27 · Abkarada-onaylı ("üstünlük bölgesini kanıtlayalım, white-paper için").**
Harness `scripts/netem/s2_region.sh`: aynı iki-netns netem + aynı mesaj-planında
kama(ürün p1_endpoint; FEC static-r4 / OFF) vs quinn(bench). SCREENING: farklı binary'ler,
aynı link+plan; formal banka kapi-eval-v1 kalır. REPS=2. Sorular (koşudan önce yazıldı):
**A (p2fec):** FEC-ON, banked %50'yi (FEC-KAPALI ölçülmüştü) ≥%90'a taşıyor mu; quinn ≈0 kalıyor mu?
**B (l{40,60,90}×d{0,150}):** üstünlük-bölgesinin loss×RTT sınırı nerede (quinn nerede çöker, kama nerede yaşar)?
**C (hb{40,60}d300):** kullanıcının adlandırdığı, HİÇ ölçülmemiş rejim (5Mbit+%40-60+300ms):
quinn ARQ-bağımlılığı burada işlevsiz kalıyor mu, kama yaşıyor mu?
Metrik: completion (kama=acked/units, quinn=delivered/msgs). Post-hoc tuning YOK; tek koşu.

## v1b eki (2026-08-27, v1-ham görüldükten sonra ama v1b-koşusundan ÖNCE yazıldı)
v1 bulguları: p2fec kama rep-varyansı büyük (%90-sorusu açık); quinn l40-60'ta BIMODAL (0/40↔40/40);
C-hücreleri gevşek D=45'te quinn'i öldürmedi. v1b: (A') p2fec kama-kolları +3 rep; (B') üç quinn
hücresi +3 rep (bimodallik iddiası için); (C') AYNI hb-rejimi SIKI-deadline (D=10s, 100ms sunum —
gerçek-zamanlı sınıf): hipotez = quinn'in çok-tur-ARQ'su deadline'a sığmaz, FEC-tek-atış sığar.
