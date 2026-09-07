# EVIDENCE-MASS — kanıt kütlesi spekülatif L2 yetkisine girmeli mi? (tarama, n=5)

**Taban:** `f5b73da8` (persistence kalibrasyonu). **Soru (owner):** kanıt kütlesi küçükken risk tahmini ne kadar
FEC yetkisi taşımalı — ilk risk sinyalini GECİKTİRMEDEN. Kapsam: yalnız spekülatif L2 yetkisi; kanonik
kayıp/ARQ/pacing/Q18/yapı-gate/A-B/P2/wire dokunulmadı. Beklenti kaydı: `EXPECTATION.md` (koşudan ÖNCE).

## 0. Taban kapısı, tanım, enstrüman, politika kolu

**Taban kapısı (`f5b73da8`, tam workspace `--all-features`, -j2):** fmt 0 · clippy 0 · **test 101**:
`bootstrap_rtt_probe_and_echo_traverse_the_product_wire` tam-workspace koşusunda 2/2 kırmızı, izole 6/6 yeşil
(feature'lı/feature'sız) → paralel yük altında zamanlama; mekanizma persist raporu §6.7'de (CsProbe çift
yayıcı, F2A kuyruğu `bootstrap_rtt` bayrağına bakmaz). Bilinen ikinci flake: `client_survives_server_death_mid_run`
(sunucu-ölümü yarışı; izole 3/3 yeşil). **İkisi de bu görevin dışında, düzeltilmedi, sonuçlardan ayrı tutulur.**
Diğer 510 test yeşil; test-adı anlık görüntüsü alındı (388 ad).

**Kanıt-kütlesi tanımı:** `evidence_mass = sent_window_units` — `fec_loss_risk_split` paydasının KENDİSİ
(rolling W_loss penceresinde gönderilmiş birimler; kanonik `loss_windowed_ppm` ile aynı payda). Ayrım
(EXPECTATION §0): fate'i bilinmeyen in-flight birimleri içerir; olay üretmiş birimler `declared_units`.
Ham okunur, taban/normalizasyon YOK. Küçük-payda mekaniği: `l2_min_persistent_ppm = 10 000` ⇒ pencere ≤ 100
birimken TEK yaşlanmış gap L2'yi uygun kılar (96-mesajlık akışta pencere 100'ü aşmaz).

**Enstrüman (ürün yolu, salt gözlem):** `RiskSplit.sent_window_units` · `TsSample.{evidence_mass, declared_units}`
(50 ms seri; `t_ms` = akış yaşı) · `Sender::diag_spec_transitions()` = ilk L1 / ilk L2 anında
`(em, instant_ppm, persistent_ppm)` · `FateWindowDiag.fec_spec_transitions` · bench `em_l1=… em_l2=…` + ts.csv
kolonları. Mevcutlar korundu: risk_t (t_susp, t_conf, t_risk, t_parity), spec (…, t_L1, t_L2), lvl, persist.

**Politika kolu (bayrak, default OFF = byte-özdeş):** `FecLossRiskConfig.evidence_mode ∈ {Off, L2Floor, L2Scaled}`
+ `evidence_min_units` (M1) + `evidence_full_units` (M2). Yalnız `speculative_propose` içinde: M1 —
`persistent ≥ l2_min` ama `em < min` ⇒ seviye L1 (L1 tavanı; L0/L1 kararı kütleyi OKUMAZ); M2 — L2 tavanı
`l1 + (l2−l1)·min(em, full)/full` (tamsayı). Handoff klampı, reorder indirimi, `spec_min` tabanı, politika
tavanı SONRA ve değişmeden uygulanır. Birim testleri (7): default Off · Off knob okumaz (örnek dizisi özdeş) ·
floor L2→L1 ve L1 anı/instant değişmez, min=1 ⇒ Off'a özdeş · scaled aritmetiği 125/187/250 · scaled
seviyeyi korur, tavanı ölçekler · temiz yolda hiçbir mod ateşlemez · geçiş-diag'ı ts örneğiyle tutarlı.
Bench: `--em-mode off|floor|scaled --em-min N --em-full N` (unit test: RuntimeConfig'e akış).

**Smoke (E2-C1b, 1 rep):** invaryant `t_risk == t_susp` (374.98 ms = 374.98 ms) ✓; ts.csv yeni kolonlar dolu;
enstrümanın ilk gerçeği: **ilk ~200 ms boyunca `evidence_mass = 1`** — PreConverge ilk ACK'i bekler, kuyruk
sonra boşalır → em@L1 = 43 (öngörü 10–25'in üstü; sapma sınıfı (2), §1'de dekompoze edilir).

_(§1 faz-1 enstrüman sonuçları, §2 eşik seçimi, §3 faz-2 M1/M2, §4 hüküm: koşular bitince.)_
