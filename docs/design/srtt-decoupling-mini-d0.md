# srtt-decoupling mini-D0 — SİM-VERİLİ (Y3 hipotez-taraması sonucu) [ONAY BEKLİYOR]

> Teori-üç-turu (r1-cerrahi-W, r2-saat-haritası) çürüdü → sim-harness (kalibrasyon-geçti: netem-3A2
> repro infl-60×/srtt-29.7s/btlbw-bayat) → hipotez-taraması. Bu not TEORİ-DEĞİL, SİM-ÖLÇÜMÜ.

## SİM-SONUÇ (desen1_sim.rs; 3A2 + 0.25k-anchor; deneysel wbtlbw_mode)
| mod | expiry (flood-durma) | btlbw@70s | 0.25k-anchor | hüküm |
|---|---|---|---|---|
| 0 legacy (srtt) | HİÇ (btlbw-5340-bayat) | flood-sürer | 31✓ | DESEN-1 (mevcut) |
| **1 delivered-EAI** | **29.3s** (btlbw→375=kanal) | **flood-DURUR** | 31✓ | **KAZANAN** |
| 2 rtprop-swap | HİÇ (5340-bayat) | flood-sürer | 31✓ | ÇALIŞMADI (r2-EAI-launder doğrulandı) |

## HÜKÜMLER (sim-verili; teori-tahmin-DEĞİL):
- **H1 ✓:** T1-tek (delivered-EAI-W), **Little-DOKUNULMADAN** (r2-dersi: Little-fren-düzeltici), flood-keser.
- **H2 ✓ aday-A (delivered-EAI):** expiry-29.3s (teori-~28s bağımsız-teyit); **0.25k-anchor-KORUNDU**
  (delivered-EAI-term2=2.097s ZM-2-riski gerçekleşmedi — btlbw-31-doğru). **Y2-ring GEREKSİZ** (A-yeter).
- **H3 ~ TUR-D1:** expiry-29.3s basamak-İÇİNDE (35s) → flood basamak-bitmeden-durur → **TUR-D1
  muhtemelen-gereksiz** (arşiv-tarafı); netem-belirler. DO-NOT-MERGE-korunur.

## MEKANİZMA (sim-doğruladı): delivered-EAI = ack_epoch'ta EAI-rate'i pacing(cwnd/srtt, bayat-btlbw-
laundered) yerine SON-INSTANT-DELIVERED-örneğinden hesapla → W_BTLBW cadence-doğru-boyutlu → bayat-max
srtt-şişkinlikten-BAĞIMSIZ-expire → flood self-stop. Little/dwell'ler DOKUNULMADI (T1-tek yeter, sim-kanıt).

## AÇIK-KALAN (netem-PR-doğrulama; sim-kapsamı-dışı):
- **Teslim-etkisi:** mode=1 delivered=676 vs mode=0=835 (sim; kopma-drop-artefaktı-olabilir). netem:
  temiz-1k=45-korunuyor-mu + teslim-oranı-regresyon-yok-mu. **KIRMIZI: temiz-1k=45 / P5-GATE {100×12} /
  DESEN-2-ramp-13×/1s / 0.25k-anchor.**
- **qd-çöküşü bonus-borç:** W_qd=2×srtt flood-yaşından-uzun → qd≡0 → fuzzy-soft-drain + ProbeUp-abort
  DESEN-1'de zaten-ölü (bu-fix-kapsamı-DIŞI; ayrı-madde, kampanya-öncesi-değerlendir).

## ÖNERİ (onaya): delivered-EAI-modunu KALICI-fix'e-çevir (deneysel wbtlbw_mode-switch temizle,
last_sample_bw()-tabanlı W_BTLBW tek-yol) → netem-PR-doğrulama (KIRMIZI-set) → temizse-PR.
Kod: sender.rs ack_epoch-bloğu (delivered-cadence) + bw.rs last_sample_bw (zaten-eklendi). Little/dwell YOK.
