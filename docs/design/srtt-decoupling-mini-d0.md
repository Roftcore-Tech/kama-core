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
- **H2 ✓ aday-A (delivered-EAI):** expiry-29.3s (ölçüm-öncesi-kayıtlı-öngörü-TUTARLILIĞI — K1-dil: aynı-pencere-denklemi-paylaşılıyor; güç kayıt-önceliğinde); **0.25k-anchor-KORUNDU**
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

## K5-SONUÇ + P4-BULGUSU (dönem-sayaçları; 676↔835 SINIFLANDI)
- P1/P2/P3 üç-modda BİREBİR (622/619 · 89/51 · 18/0) → daralma-teslimi KORUNDU; fark %100 P4-toparlanma.
- **P4-kök (sim-iz):** mode-1 kopma-sonrası **RecoverArq'ta-sıkışık** (srtt-0.3s-temiz, btlbw-333-dürüst
  ama 8k-kanalda 3×-underutilize; RecoverArq'ta-ProbeUp-yok → yeni-kapasite-keşfedilmiyor). Legacy
  bayat-5340'la kazara-dolduruyordu → **MASKELİ-BORÇ açığa-çıktı** (maske-serisinin-yeni-halkası).
- **Ş1 (Abkarada):** ticket = "RecoverArq-bant-keşfi-yok = **bacak-3-ikinci-baş (TUR-5-kaydı), İKİNCİ-
  bağımsız-kanıt** (p5: probe=0/0/0/0 + P4: 3×-underutilize-45s)". Kapsam-değerlendirmesi **TUR-S-D0'ında**
  ("dönüş-sonrası-bant-keşfi" kanal-durumu-makinesinin-doğal-üçüncü-bacağı); ayrı-mini-tur-YOK.
- **Ş2 (RED-kaydı):** "recovery'de-ProbeUp" mini-eki RED — RecoverArq-FAZ-ÖNCELİĞİNE-dokunur =
  P5-GATE-mührünün-bölgesi (parazit-recovery-toprağı); kendi-D0+KIRMIZI-seti-ister; bu-PR'a-girmez.
- **Ş3 (netem-beklenti-düzeltmesi):** legacy-P4 "dolduruyordu"-değil **5.3×-AŞIRI-sürüş**; sim-inf-buffer-
  masum ama netem-sonlu-buffer DROP-üretir → **delta-daralması-beklenir** (159 legacy-lehine-iyimser);
  daralmazsa inf-buffer-varsayımı-sorgulanır. + netem-P4: RecoverArq-ÇIKIŞ-doğrulaması (kalıcı-wedge ✗).
- **Ş4 (karar-kuralı):** netem-P4-telemetrisi → toparlanma-farkı kampanya-temsili-hücre-RMST'sini-
  etkiliyorsa borç TUR-S-kapsamında-KAMPANYA-ÖNCESİ-ZORUNLU (K8-qd-kardeşi); etkilemiyorsa beyanlı-borç.
- **Komşuluk-taraması (K5):** {25,31,37,125 B/s} dört-noktada anchor-birebir (btlbw=rate; iki-mod-özdeş)
  → ZM-2-hükmü genelleşti.

## K9-KALICI-KAYIT
- delivered-sayaç-tanımı: wire_sent=TransmitDecision-adedi (tüm-kind); unique_delivered=unit_id-ilk-teslim;
  dropped=link-down-düşen (kopma-anı-kuyruk-boşaltma-dahil). Dönemler: P1[0,35) P2[35,70) P3[70,100) P4[100,son].
- sim-determinizm: RNG-yok, 1ms-tick, ideal-ACK (20ms-birleştirme, 40B, 1MB/s-dönüş); seed-kavramı-yok.
- 3A2-parametreler: 40k(35s)→3k(35s)→kopma(30s)→8k(45s); UNIT=267B; n=1500; fwd-delay-5ms.
- btlbw→375-kabul-toleransı: expiry-tespiti btlbw<600 (kanal-1.6×-bandı).
- K3-semantik-pinleri (kodda): (i)-freshness=epoch-başı-son-TAM-örnek (1-olay-gecikme-yapısal, determinizm-
  korunur); (ii)-ACK-compression interval=max(ack_elapsed,send_elapsed)-sönümlü, kabul+telemetri;
  (iii)-app-limited-örnek cadence-girdisi-OLAMAZ (bw.rs last_full_bw yalnız-!app_limited); (iv)-zero/
  invalid→son-geçerli-tekrar (alan-kalıcı; hiç-örnek-yoksa 0→.max(1)+INITIAL_RTT-taban). Smoothing/ring-YOK.
- re-flood-gözlemi: sim'de-yok (btlbw-375-sabit-kaldı-expiry-sonrası); netem-gate'te-tekrar-doğrulanır.
- mode-switch-silme-anı: K4 — netem-A/B-sonuçları-kayıt-edildikten-sonra final-cleanup-commit'inde.
- qd-ticket (K8): "secondary-safety-path-deadness" — kampanya-öncesi-kapanma-ZORUNLU; owner=Abkarada-
  sırası (TUR-S-yanı); bu-PR observability-only (netem-telemetrisi qd/soft-drain/abort-enstrümante).
