# Kama Transport Katmanı — Olgunluk Raporu

**Tarih:** 2026-09-02 · **Referans:** main @ `ca65e6cb` (PR #45 "transport product-grade kapanış" merge'ü sonrası)
**Amaç:** transport katmanının bugünkü olgunluğunu, eksiklerini ve geliştirme adaylarını **objektif** olarak,
iddia-dili kurallarına uyarak kaydetmek. Her hüküm kanıt-yoluyla bağlıdır; overclaim içeren hiçbir etiket
("production-ready", "hostile-ready", "QUIC'i yendik") kullanılmaz.

---

## 0. Tek-paragraf özet

Transport **baseline'ı** product-grade olarak kapanmıştır: gerçek runtime kabuğu, doğrulanmış sınırlı
kuyruk-kurtarma, sertleştirme (fuzz + amplifikasyon düzeltmeleri), dondurulmuş transport-API ve 10/10 hosted-CI
mührü mevcuttur; ön-kayıtlı r5 kampanyasında p2 (%80 kayıp) hücresinde Quinn primary-referansına karşı
Holm-düzeltmeli **+38,4 puan** eşleştirilmiş üstünlük bankadadır. Buna karşılık **ürün-düzeyi değer önerisi henüz
kanıtlanmış sayılmaz**: §4.4 GO/NO-GO kapısı açıktır (en güçlü hücre 0,505 — %90 kategorik barın altında ve FEC
kapalıyken ölçüldü), patlamalı-kayıpta Kama referansların gerisindedir (P0 düzeltmesi açığı −7,3→−1,7 puana
küçülttü ama kapatmadı ve dondurulmuş yeniden-kanıt kampanyası tamamlanmadı), gerçek kripto katmanı yoktur
(crypto crate 3 satır; Sprint-6, spec-audit kararlarına bağlı). Projenin kendi 2026-08-26 değerlendirmesi:
*"Transport ~%40 olgun; değer önerisi KANITLANMADI."* Bu iki hüküm çelişki değildir — biri hücre-düzeyi banked
sonuç, öteki ürün-düzeyi kapı durumudur (claims/whitepaper.5 kaydının çözümü de budur).

## 1. Bugün NE VAR — implementasyonu tamamlanmış ve kilitli

`crates/adaptive-transport`: 26 modül, 17.338 LOC, `#![forbid(unsafe_code)]`, ~330 test.

| Alan | Durum |
|---|---|
| SACK-V2 kümülatif ACK (0x14; 256µs ack_delay, seen_count/arrival_rate) + u16→u64 seq çözümü | ✅ golden |
| RTT/RTprop/W_qd tahmincileri (kuantum-döngü, keep-last, delivered-EAI W_BTLBW [Y3/K6R]) | ✅ |
| BtlBw delivered-rate + geçerlilik kapısı (app_limited / ack-compression / reverse-impaired reddi) | ✅ |
| Faz makinesi (PreConverge → Startup/Drain/Steady/ProbeUp/DrainRtprop/Recover*) | ✅ |
| Loss = hipotez modeli (ledger Suspected→Confirmed/Spurious/Expired; tombstone + un-loss; kadans-farkındalıklı eşik) | ✅ |
| ARQ seçici retransmit + K_ARQ_SHARE | ✅ |
| Acil ileri-çoğaltma (asimetrik Schmitt + dwell histerezisi) — ACK-bağımsız kurtarma | ✅ |
| PreConverge cold-start (davranışsal prior, geometrik kopya, dört yasak, kademeli tahliye) | ✅ |
| Sınırlı PTO kuyruk-kurtarma (`tail_recovery.rs`) — iki-netns/netem'de doğrulanmış | ✅ mekanizma |
| Fuzzy motor matematiği (Mamdani, log2-fp, sıfır-ağırlık fallback'i; ADR-17 determinizm) | ✅ golden |
| S1 ürün-FEC yolu (explicit R2 actuator; sealed DATA-önce-PARITY) + kama-fec-codec profilleri | ✅ |
| kama-noise-wire mühürleme katmanı (EXTENDED 26B outer + 11B inner; PN; AEAD-differential-oracle) | ✅ (handshake HARİÇ) |
| INV-RATE-1 / INV-FEC-1 / INV-ENGINE-SOC — üçü de yapısal + test-kilitli | ✅ |

**Banked ölçüm kanıtı:** r5 kapi-eval-v1 p2 KAMA-SUPERIOR (+38,4pp ort. / +47,0 medyan, CI [+31,5, +44,9],
Holm-p 0,0003, n=50) · DR-ARQ-PRODUCT-TAIL-1 GO (terminal-tail OFF 8/360 → ON 329/360; invariant ihlali 0/2290)
· FS-1 v1.3.2 O2_FEC_SPECIFIC 12/12 + R2A F0-only 868 vs A1-only 0 (coding > kör-çoğaltma, eşit konfigüre bütçe)
· wall-clock FEC aralığı rtt250'de gerçek (p99 T_fec < p01 T_arq).

## 2. Bugün NE YOK / kapalı — dürüst eksik listesi

### 2.1 Ürün hükmünü bloke edenler
1. **Gerçek güvenlik katmanı YOK.** `crates/crypto` = 3 satır; workspace'te x25519/snow/DH sıfır. G2 kapısı
   (güvenlik paritesi) bu yüzden koşulamıyor; §4.4.7 code-then-encrypt uçtan-uca denetlenemiyor. Sprint-6,
   58-bulguluk spec-audit'in owner A-kararlarına bağlı (S2-noise kümesinde 11 BLOCKER; ör. rekey çelişkisi:
   ccr 2^31 vs noise-wire no-rekey).
2. **§4.4 GO/NO-GO kapısı AÇIK.** En savunulabilir hücre p2: 0,505 — asimetrik üstünlük ama %90 kategorik
   barın altında ve **FEC kapalıyken** ölçüldü. "QUIC mimarisine üstünlük" iddiasının tek yolu (FAZ-2
   ISOLATION+REFERENCE conjunctive) hiç koşulmadı.
3. **Patlamalı kayıp (h3) hâlâ geride.** P0 düzeltmesi doğrulandı (h3b: 0,927→0,983; açık −7,3→−1,7pp) ama
   hücre hâlâ kaybediliyor; kama-arq kolunun ayrı terk-yolu teşhis kuyruğunda; **burst-reproof-v1 dondurulmuş
   ön-kaydı 15/120'de yarıda kaldı** — NOT-WORSE/STILL-WORSE hükmü üretilmedi (kayıtlı ön-kayıt-bütünlüğü
   ihlali; tamamlanması gerekir).
4. **TCP/KCP kıyası HİÇ koşulmadı.** Kollar planlandı, kod yazılmadı (`crates/bench`'te TCP/KCP izi yok).
   "Kama beats TCP/KCP" sınıfı hiçbir cümle kurulamaz.
5. **Sürekli-blackout metriği (BENCH-01) yok.** p6 "intermittent" proxy'dir; "QUIC session-survival kaybeder"
   iddiası ikili metrik olmadan yapılamaz. quinn'in p6=1,00'i de bu yüzden "blackout-proof" okunamaz.

### 2.2 Varsayılan-kapalı (bilinçli) mekanizmalar
tail-recovery (mekanizma GO; `validated_v1` konfigürasyonu ayrı — shipped `Default` DEĞİL) · füzyon/FEC
politikası (`FusionPolicy::disabled()`; kalibrasyon DR-FUSION-POLICY-1'e bağlı — loss-yapısı × oran × RTT grid'i)
· `fate_estimator` (ürün çağıranı yok; tam da Q17/Q18'in istediği loss-yapısı gözlemcisi) · FEC actuation
(`Manual{enabled:false}`) · cold-start batch API (feature-gated) · DR-EST-1 establish-probe (opt-in).
Bunlar borç değil, kalibrasyon/karar bekleyen güvenlik durumlarıdır; ama toplamda ~2049 LOC uyuyan ürün
mühendisliği demektir.

### 2.3 Spec-kayıtlı açık borçlar (motor içi)
- **E-FULL ölçülen `return_bw` yok** — ACK kadansı / T_SPACING / K_c hâlâ statik-önsel 31 B/s türevi;
  arrival-türevi yapılamıyor çünkü sensing ön-şartı (starvation ≠ serialization ayrımı) çözülmedi (M4/M5).
- **W_BTLBW daralma anti-fix'i açık borç** (TUR-D1 EK-3; shrink-dedektör ailesi ARŞİV/MUST-NOT-IMPLEMENT).
- **§16.2 SUSPECT satırları koddan doğrulanacak:** `dup_spacing=20ms` duvar-saati (serileştirmenin çok altında →
  dekorelasyon amacı ölü) · `deescalate_dwell=1s` (sub-kbps'te <1 srtt) · acil-merdiven event-count sayımı ·
  `MIN_CWND=4×MTU` mutlak (sub-kbps BDP'nin 3-48×'i) · geometrik-kopya deadline tavanı kodda yok.
- **Compression'ın yürütücüsü yok** (spec normatif, zstd workspace'te hiç yok — S2-noise.16).
- **ADR-16 sürüklenmesi:** ADR `reed-solomon-erasure` der, kod `reed-solomon-simd 3.1.0` kullanır; FEC profil
  allowlist'i (0x01-0x06) hiçbir spec'te yok (S2-noise.19) — spec-audit A-kararlarında çözülmeli.
- **K-1 (`pn.rs:52` taşma; mevcut çağıranlarla erişilemez-kanıtlı) ve K-2 (`loss.rs:44` sıfıra bölme; public
  config yolu açık)** — test branch'inin Kani ispatları bilerek kırmızı bekliyor; düzeltme owner kararında.
- **Production fragmenter yok** — SourcePrefix üretimde ama `chunk_offset` hep 0; E−11'den büyük mesaj FEC
  yolundan geçemez (ayrı ürün DR'ı).
- [CAL] sabitlerinin çoğu hâlâ placeholder; netem kalibrasyonu (ADR-18) sonrası golden'a yazılacak.

### 2.4 Ölçüm/altyapı eksikleri
- c(t)/Kaplan-Meier ve delivered-vs-byte eğrileri (r5 ana-çıktısı) üretilmedi — dış-rapor açık kalemi.
- Nightly netem kapısı: test branch'i pipeline'ı getirdi ama self-hosted runner atanmadı; ayrıca CI kör
  noktaları (miri 0-test, trybuild 0-test) test branch'i merge edilene dek main'de duruyor.
- Tek-mesaj/tek-blok FEC stratumu (FS-2) ve burst-kollu coding ölçümü (R2B) yok; congestion-kaynaklı kayıp
  hiç modellenmedi (parity vs congestion-controller etkileşimi ölçülmemiş).

## 3. Geliştirme adayları (öncelik önerisiyle; karar owner'ın)

1. **Burst-reproof-v1'i tamamla** (dondurulmuş ön-kayıt yarım — bütünlük borcu + tek kayıp hücrenin hükmü).
2. **Test branch'ini birleştir** (CI kör noktalarını kapatır: gerçek miri, asan/msan, coverage kapısı, netem
   gate; K-1/K-2 kararını owner'a getirir). 5 dosyalık çakışma çözümü + eşik yeniden-kalibrasyonu gerekir.
3. **Sprint-6 Noise** — owner'ın A1-A10 spec kararları geldiğinde (G2 kapısının ve gerçek E2E'nin ön şartı).
4. **DR-FUSION-POLICY-1** — loss-yapısı-girdili FEC aktivasyonu (`fate_estimator`'ı ürüne bağlama);
   Q17/Q18'in gösterdiği tek geçerli yol.
5. **Tail default-ON kararı** + probe-cap'in kuyruk-uzunluğuna adaptasyonu (L10-med 0/30 tuning bulgusu).
6. **FAZ-2 conjunctive FEC kampanyası** (Q-FEC-ISOLATION + Q-FEC-REFERENCE) — mimari iddianın tek yolu;
   ayrıca FS-2 tek-blok stratumu ve R2B burst-coding.
7. **M4/M5 sensing** — E-FULL return_bw ve blackout-tespitinin kök ön-şartı.
8. §16.2 SUSPECT temizliği (dup_spacing, dwell, MIN_CWND) — her biri kanal-türevli formüle.
9. TCP/KCP kollarının inşası (kıyas ancak ARP-pinli yeni koşularla; v1-dönemi mutlak sayılar taşınmaz).

## 4. İddia-dili hatırlatması (bu raporun kendisi için de bağlayıcı)

En güçlü izinli hükümler: **"transport baseline product-grade"**, **"p2 hücresinde preregistered Q-DGRAM-ARQ
baseline'ına karşı Holm-düzeltmeli üstünlük"**, **"bounded tail-probe mekanizması gerçek-wire'da doğru"**.
Yasak: production-ready/hostile-ready · "QUIC mimarisine üstünlük" (FAZ-2'siz) · CPU/throughput iddiası
(AEAD maket) · TCP/KCP kıyas iddiası · sürekli-blackout iddiası · "ARQ vs FEC" çerçevesi.
