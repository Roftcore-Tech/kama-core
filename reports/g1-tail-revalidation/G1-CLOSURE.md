# G1 — Tail-Recovery Post-P0 Re-Validation (KAPANIŞ)

**Statü:** YEŞİL — tail default-ON adayı geçti. Karar: P3 §2.3 G1 kapısı. Binary: post-P0
(`759d4aa8`+), aynı drand tohumları (Model A). Amaç: `validated_v1`'in P0-sonrası motorda hâlâ
doğru davrandığını + P0'ın primary hücreyi bozmadığını göstermek.

## Kapı (üç kontrol, hepsi geçti)

| kontrol | tail-ON (kama-tail-v1) | tail-OFF (kama-arq) | kriter | sonuç |
|---|---|---|---|---|
| h3b GE-burst%50 (n=3) | medyan 1.000 [1.0,0.99,1.0] | 0.990 | ON ≥ OFF | ✅ |
| h3c GE-burst%60 (n=3) | medyan 1.000 [0.97,1.0,1.0] | 0.990 | ON ≥ OFF | ✅ |
| p2 rastgele%80 (n=10) | medyan 0.535 · ort 0.523 | medyan 0.410 · ort 0.443 | no-harm (ON ≥ OFF−0.05) | ✅ |

**Yorum:**
1. **Burst (asıl gerekçe):** post-P0 motorda tail-ON, tail-OFF'a burst rejiminde eşit-veya-üstün;
   P0 fix + tail birlikte kama-tail-v1'i h3b/h3c'de 1.000'e taşıdı. Jammer/EW rejiminde tail-recovery'nin
   gerekli olduğu doğrulandı.
2. **p2 no-harm:** tail-ON p2 rastgele-kayıpta tail-OFF'a ZARAR VERMİYOR (n=10; ort +0.08, hafif önde
   ama n=10 varyanslı — bunu "tail-ON p2'de üstün" diye YORUMLAMA; otoriter p2 karşılaştırması banked
   n=50 nötr sonuçtur: 0.505/0.495). Kriter tek-yönlü "zarar-yok"tur, geçti.
3. **P0 primary-regresyon YOK:** post-P0 tail-ON ort 0.523 ≈ banked medyan 0.505 → P0'ın queue_losses
   düzeltmesi p2 headline hücresini bozmadı; banked +38pp hükmü (P0-öncesi ölçüm) sarsılmadı.

## Sınırlılık (dürüstlük)
- n küçük (burst n=3, p2 n=10); bu bir ön-kayıtlı test DEĞİL, mühendislik re-validation'ı (kapı).
- terminal-tail kanonik iki-süreç harness'ı (dr_arqtail_product) koşulmadı; ama P0 tail-probe
  kod-yolunu DEĞİŞTİRMEDİ (yalnız queue_losses Duplicate-branch) ve deterministik tail testi
  (burst_loss_recovery Sınıf-B) post-P0 geçiyor.
- Veri: `data/` (kama out.jsonl + realized + driver log; cap HARİÇ). 38 koşu, 0 exogenous-INVALID.

## KARAR: G1 GEÇTİ → G2 (tail default-ON) Abkarada onayına HAZIR
Flip = ürün-kuruluşu `set_tail_recovery_config(validated_v1())` çağırır; `TailRecoveryConfig::default()
.enabled` **false KALIR** (R1 kuralı: doğrulanmamış default-bounds sevk edilmez). L10-med tuning açığı
ürün-v2 backlog (G3).
