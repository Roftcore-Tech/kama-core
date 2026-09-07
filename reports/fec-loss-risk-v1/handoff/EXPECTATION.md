# HANDOFF AİLESİ — BEKLENTİ KAYDI (koşudan ÖNCE yazıldı)

**Soru (owner):** spekülatif → kanonik devir teslimde "cliff" var mı? İnvaryant: *kanonik sinyalin
gelmesi FEC kurtarmasını kötüleştiremez.*

**Konfig:** kama-product · profil 0x03 · 256 B · 20 ms aralık · adaptif FEC · RTT 200 ms · 1 mbit ·
reorder YOK · msgs {96, 400, 1000} × kayıp {5, 10, 20} % × {OFF, ON} × n=3 · 50 ms zaman serisi.

**Koddan türetilen beklenti (c809069b + enstrüman):**
1. `t_first_confirmed ≈ t_susp + 3.15 s` (her rep'te; ARQ değişmedi).
2. O anda `loss_windowed_ppm > 0` → spekülatif tavan **250 → 125 ‰ (r=2 → r=1)**, ANINDA.
   `fin_p1 − fin_pre ≈ −125 ‰` ve `sebep_pre = SpecPersist`, `sebep_p1 = SpecBoot`.
3. Kanonik fuzzy bu bant rejiminde 0 (bw.med ≈ 0). Yapı-gate onaylı oran ≥ 100k'da açılır; onaylı
   oran ≈ `p·x/(3.15+x)` → **%20'de ~t_susp+6.3 s'de 500 ‰ (r=4)** açılır; **%5/%10'da ASLA**.
4. Dolayısıyla: %20 → r=2 → r=1 → r=4 (iki basamak: uçurum + aşırı-atış); %5/%10 → r=2 → r=1
   KALICI (kanonik hiç devralmaz). 96 msg → onay yok, kontrol.
5. Teslim ≤2×RTT post-handoff penceresinde düşmeli: blok çözülebilirliği %10'da r=2 0.930 →
   r=1 0.775; %20'de 0.678 → 0.436. Beklenen `Δdel_p1` negatif, ~−5…−15 pp (n=3 gürültü ±10 pp!).

**Sapma dekompozisyonu için:** (a) t_conf sapması → ARQ tarafı; (b) fin_p1 düşmüyorsa → klamp
çalışmıyor ya da gate erken açıldı; (c) teslim düşmüyorsa → r=1 bu rejimde yeterli ya da gürültü.
