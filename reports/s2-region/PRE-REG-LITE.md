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
