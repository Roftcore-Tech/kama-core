# PERSIST AİLESİ — BEKLENTİ KAYDI (koşudan ÖNCE)

**Soru (owner):** reorder yüzünden L2 ne zaman yanlış açılıyor; cold-start'ı geciktirmeden nasıl engellenir?
**Modlar:** P0 fixed 0.5×srtt (sevk) · P1 learned Q90·1.25 · P2 learned Q95·1.25 (ikisinde taban 0.5×srtt =
P0) · P3 fixed 1.0×srtt (kör referans). **Politikalar:** A `250-250-250`, B `250-375-375`. n=3, 96/400 msg.
**Hücreler:** C0 temiz · C1a/b `delay 100ms 20ms reorder 15/30%` (ömür ≈ OWD = 0.5 srtt — çakışma hücresi) ·
C2 `delay 100ms 60ms` jitter-reorder (ömür ≤ 0.3 srtt) · C3 RTT 50 `delay 25ms 40ms` (ömür ≤ 1.3 srtt) ·
C4 kayıp 20 % + reorder 30 % · C5 kayıp 20 % (kontrol).

**Öngörüler (koddan):**
1. **C5/C0 (spurious yok):** P0 ≡ P1 ≡ P2 **byte-özdeş** (öğrenilen ufuk taban=0.5 srtt'ye düşer);
   P3 L2'yi +100 ms geciktirir → t_L2 +~100 ms, d2 −0.02…−0.05.
2. **Cold-start:** t_first_L1 her hücrede moddan bağımsız (L1 anında).
3. **C1a/b (çakışma):** P0'da yanlış L2 (ölçülmüştü: 3–156); P1/P2 ilk spurious örneklerinden sonra ufku
   ≈125–165 ms'e yükseltir → yanlış L2 belirgin düşer ama ilk ~100 ms'de (halka boşken) sıfır DEĞİL;
   P3 (200 ms) baştan ~0. Parity/wire A-P1/P2 < A-P0; B-P0'ın reorder cezası A-P0'dan büyük (L2=r3).
4. **C2 (jitter 60 @200):** ömür < 100 ms → tüm modlar ≈ 0 yanlış L2, P0 zaten yeterli.
5. **C3 (RTT 50, jitter 40):** ömür (≤65 ms) > 0.5 srtt (25 ms) → P0 ÇOK yanlış L2; P1/P2 Q95≈55–65 ms
   ×1.25 ≈ 70–80 ms → düşer; P3 (50 ms) hâlâ kuyruğun altında → kısmen yanlış L2.
6. **C4 (kayıp+reorder):** P1/P2 reorder-L2'yi eler, gerçek kayıp (asla Spurious olmaz) L2'yi korur; ufuk
   ~150 ms'e çıktığı için gerçek-kayıp L2 +50 ms gecikir → d2 −0.00…−0.03, parity daha düşük; B > A +10 pp.

**Sapma dekompozisyonu:** (1) tutmazsa → öğrenilen yol spurious'suz da ufku değiştiriyor (bug); (2) tutmazsa →
L1 bir yerden persistence'a bağlı (tasarım ihlali); (3/5)'te yanlış L2 düşmüyorsa → quantile örnekleri
gelmeden bloklar kapanıyor (halka gecikmesi) ya da yaş ölçümü yanlış nicelik.
