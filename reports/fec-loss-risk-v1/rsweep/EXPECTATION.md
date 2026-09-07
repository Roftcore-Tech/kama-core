# R-SWEEP — BEKLENTİ KAYDI (koşudan ÖNCE yazıldı)

**Soru (owner):** FEC açıldığında kaç repair gerçekten ekonomik? L1 / L2 / yapı-gate seviyeleri;
reorder YOK; teslim(≤2×RTT) vs wire Pareto. **Kesirli tasarım:** gate ekseni yalnız 400 msg ×
≥15 % (onaylı oran ≥100k'a ancak orada); 0 %'de yalnız {off, mevcut}; 96 msg ve 400×{5,10}'da
gate 500 sabit, L1×L2 4 kombinasyon. n=5. Politika kodu `<L1>-<L2>-<G>` ‰ (K=8: 125=r1,
250=r2, 375=r3, 500=r4). Mevcut = 125-250-500.

**Koddan/kampanyadan türetilen öngörüler:**
1. **L1 ekseni duyarsız** (reorder'sız): L1 yalnız kuşkunun ilk ~100 ms'inde yaşar (persistence
   0.5×srtt); histogramlarda L1 kararların %2–5'i. `125-x-y` ≈ `250-x-y` (Δd2 gürültü ±0.05 içinde,
   parity_B farkı < %5). Bu bir bulgu değil tasarım gereğidir; L1 kalibrasyonu reorder fazında.
2. **L2 r3 vs r2:** MDS K=8 blok çözülebilirliği r2→r3: %5 0.988→0.998 · %10 0.930→0.977 ·
   %20 0.678→0.839 · %25 0.526→0.713. Beklenti: %5/%10'da Δd2 ≈ 0 (r3 DOMİNE, +%50 parity boşa);
   %20/%25'te Δd2 +5…+12 pp, parity +%50. Pareto: %20+ için r3 sınırda kalabilir, ≤%10 için düşer.
3. **Gate ekseni (400 × 20/25 %):** kampanya r4 = r2 kurtarma / 2× byte ölçmüştü → beklenti
   `x-y-500` DOMİNE (Δd2 ≤ +0.02, wire +5…+10 pp); `x-y-250` ile `x-y-375` arasında küçük fark.
   Gate ancak ~t_susp+6.3 s'de açıldığından 400 msg akışında (8.2 s) etkisi son ~2 s'ye sıkışır →
   ölçülen gate etkisi KÜÇÜK olacak (bu, gate'in değersizliğinin kanıtı DEĞİL, akışın kısalığıdır).
4. **0 %:** tüm kollar özdeş (risk=0 → parity 0); wire farkı 0.
5. **Kısa-akış kapısı (96 × 15/20/25):** parity_before_flow_end varlığı L1/L2 MİKTARINA bağlı
   değil (seviye ateşlemesine bağlı) → değişmez; d2 L2=r3'te ≥ mevcut. Gerileme beklenmiyor.
6. **PE (zamanında-mesaj / parity KB):** r2'de en yüksek; r3'te düşer; r4'te en düşük.
   Marjinal PE r2→r3 %20+'de pozitif ama küçük, r3→r4 ≈ 0.

**Sapma dekompozisyonu:** L1 etkili çıkarsa → persistence penceresi sanılandan uzun ya da L1
başka bir yoldan miktar belirliyor; gate etkisi büyükse → gate sanılandan erken açılıyor
(onaylı oran hesabı yanlış); r3 ≤%10'da kazandırıyorsa → MDS modeli ile ölçüm arasında
paydada (blok hizası/tail) bir şey var.
