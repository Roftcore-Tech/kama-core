# GÖREV r5 (BİRLEŞİK SOURCE-OF-TRUTH): v2 Adverse-Network Benchmark Kampanyası

> **Kimden:** Abkarada. **r5 = tek otoriter belge.** r1/r2/r3/r4 ARŞİV/SUPERSEDED; otorite yalnız
> budur. ÜÇ bağımsız metodoloji review'u + Abkarada kararları işlendi. Üçüncü review "r4 GO" verdi;
> beş son normatif kilit (paced-budget · deadline-byte-ortogonal · FEC-aynı-implementation ·
> evaluation-Model-A · packet-atomic-cap) + terminoloji (dual-mandatory/conjunctive) bu sürümde
> işlendi. **Koşu statüsü:** KAPI-1 tasarım+implementasyon GO · calibration koşuları KAPI-1 taslağı
> onayı sonrası GO · **evaluation/headline koşuları KAPI-1+KAPI-2 tamamlanana kadar NO-GO.**
> **Ciddiyet:** DCP/ASELSAN/TÜBİTAK/müttefik-savunma + white-paper. Her sayı düşman hakeme dayanır.
> **En tehlikeli hata:** Kama'nın FEC/mimari avantajını "QUIC mimarisine üstünlük" diye yanlış
> atfetmek. Tüm iddia-dili buna göre kilitli (§C).

---

## A. İŞ SIRASI VE KAPILAR
ADIM-0 git/spec sync → ADIM-1 rebrand → ADIM-2 tasarım (KAPI-1) → ADIM-3 ön-kayıt (KAPI-2) →
ADIM-4 koşu (KAPI-3 ara-özet) → ADIM-5 rapor (KAPI-4). Onaysız kapı geçilmez. **Koşu kademesi:
calibration koşuları yalnız KAPI-1 TASLAK onayı sonrası; evaluation/headline koşuları yalnız
KAPI-1 FINAL onayı VE KAPI-2 ön-kayıt tamamlandıktan sonra başlayabilir.**

## ADIM-0 — Git/spec sync (İLK)
Ben `spec/` altını güncelledim (identity-and-trust, discovery, routing-rendezvous, ccr — gap-kapatma;
mimari değişmedi). transport-tdd + engine-spec'e DOKUNMADIM. (1) pull/rebase, conflict→bana.
(2) güncel spec'i oku, kampanyayı etkileyen wire/cert/INIT boyutu değişikliği varsa sabiti güncelle+raporla.
(3) otorite = güncel spec.

## ADIM-1 — Rebrand SafeRoom→Kama (mekanik, tek PR, davranış-sıfır kanıtlı).

---

## B. İKİ DÜZLEM + BAŞLANGIÇ DURUMU

### B.1 Düzlem ayrımı (headline yalnız DÜZLEM-A'dan)
- **DÜZLEM-A — established-session data-plane (ANA):** oturumlar test-öncesi kurulmuş; t=0 =
  uygulama mesajlarının verilme anı; handshake byte'ları wire-sayımına GİRMEZ.
- **DÜZLEM-B — modeled cold-start (deneysel, AYRI bölüm):** quinn gerçek TLS/QUIC; Kama
  "byte-modeled init". Her başlık **"experimental — modeled cold-start"**. Üstünlük iddiasına
  KAYNAK OLAMAZ. Caveat (İngilizce, aynen): "Kama security initialization is byte-modeled and
  does not reproduce the causal flight structure, timeout behavior, cryptographic computation,
  validation failures, or loss-recovery dynamics of the production security handshake."
  ("compute-asimetrik" ifadesi YASAK.) quinn'in gerçek davranışları (1200B Initial, PTO-üstel,
  anti-amplification) nötr dille raporlanır — protokol gerçekliği, kusur değil.

### B.2 DÜZLEM-A alt-bölünür (review r3#2 + r4#7)
- **A1 — established, unadapted transition.** Akış (yarış-durumu önleme, r4#7):
  (1) session temiz ağda kurulur → (2) uygulama trafiği DURUR → (3) impairment yüklenir + aktif
  olduğu **barrier ile doğrulanır** → (4) transport-state reset YOK, warm-up YOK → (5) ortak
  barrier t=0'da mesajları serbest bırakır. Böylece ilk paketler impairment'a "kaçmaz".
- **A2 — established, steady-state.** Impairment altında ön-belirlenmiş warm-up sonrası t=0.
  Kirlenme kuralları (r4#7): warm-up byte'ları ölçüm bütçesine GİRMEZ; t=0'da sayaçlar sıfırlanır
  AMA congestion/estimator state KORUNUR; warm-up'tan in-flight paket kalmaz; evaluation-trace
  başlangıcı iki transport için AYNI.
- **KAPI-1 alanları (ikisi için):** session_establishment_network · impairment_activation_time ·
  warmup_duration · warmup_traffic · congestion_state_reset · bandwidth_estimator_reset ·
  initial_cwnd · initial_rtt · prior_path_history · warmup_quiescence_condition · inflight_at_t0 ·
  counter_reset_without_state_reset · warmup_trace_id · evaluation_trace_start_offset.
- **Kama-özel:** A1'de oturum kurulu ama impairment yeni → cold-start DEĞİL, estimator-şok;
  PRE_CONVERGE devrede mi, restore_burst/bw-lock başlangıç durumu ne — AÇIKÇA belgelenir.
- **Headline'ın A1 mi A2 mi olduğu ÖN-KAYITTA sabit** (ikisi de raporlanır).

### B.3 Bağlantı ölümü / reconnect (review r3#3)
DÜZLEM-A'da otomatik reconnect YOK. Oturum ölürse (idle/path-validation/vb.) kalan mesajlar
deadline'da BAŞARISIZ; termination-nedeni raporlanır; iki transport aynı kural. Reconnect testi
gerekiyorsa **DÜZLEM-C — session disruption & recovery** ayrı+ön-kayıtlı (bu kampanyada opsiyonel).

---

## C. KARŞILAŞTIRMA HEDEFLERİ VE İDDİA DİLİ (kampanyanın kalbi)

### C.1 quinn baseline seti (DÖRT rol)
1. **Q-STREAM** — reliable-ordered; stream-mapping (mesaj-başı-stream mı tek-stream mi + HoL
   etkisi) açık; "standart QUIC kullanımı" referansı.
2. **Q-DGRAM-ARQ** — DATAGRAM + app-ARQ; FEC'siz semantik-eşlenik referans.
3. **Q-FEC-ISOLATION** — *birincil nedensel baseline.* "Mümkün olduğunca aynı" YETMEZ (review r5#3):
   AYNI implementasyon — same FEC crate/lib · same exact version/commit · same symbol size · same
   block construction · same source/repair symbol mapping · same decode trigger · same parameter
   table. Transport-entegrasyonunun zorunlu kıldığı her sapma önceden **deviation tablosuna** girer:
   `field | Kama value | Quinn value | why equality impossible | expected methodological effect`.
   Bu tablo KAPI-1 çıktısıdır; tablo-dışı sapma YASAK. Amaç: FEC-algoritma farkını DEĞİL, yalnız
   taşıma/entegrasyon farkını izole etmek.
4. **Q-FEC-REFERENCE** — *bağımsız güçlü doğrulama.* Literatüre/olgun implementasyona dayalı,
   preregistered en güçlü QUIC+FEC. Amaç: gerçek sistem rekabeti.

### C.2 ISOLATION + REFERENCE okuma kuralı (Abkarada karar — nedensellik kilidi)
İkisi **dual mandatory comparison baseline'dır** (primary olan endpoint/hypothesis'tir; bunlar
karşılaştırıcı — review r5-terminoloji). **Conjunctive success rule (ön-kayda aynen):**
"architectural/integration interpretation succeeds only if BOTH mandatory baseline comparisons meet
the preregistered effect and confidence criteria." Kritik headline hücrelerinde:
- İkisi de AYNI yönde geçilir → sınırlı **mimari/entegrasyon avantajı** yorumu (conjunctive kural).
- Yalnız ISOLATION geçilir → iddia **entegrasyon/mekanizma avantajına** daraltılır.
- Yalnız REFERENCE geçilir → sonuç **FEC-seçimi/implementasyon farkı** olabilir (mimari değil).
- **Ölçümden SONRA uygun baseline seçmek YASAK** (conjunctive kural bunu yapısal olarak imkânsız
  kılar). Üçü de (Kama/ISOLATION/REFERENCE) aynı kanal-türevli bütçe + payload + deadline +
  app-ACK semantiği + başarı kriteriyle koşar.

### C.3 İddia dili (normatif, dış-rapor)
- FEC-baseline'ları koşulmadan "QUIC mimarisinden üstün" DENMEZ. ISOLATION+REFERENCE aynı yönde
  değilse de denmez (C.2).
- İzinli en güçlü form: "Kama, preregistered Quinn 0.11.11 Q-FEC-ISOLATION ve Q-FEC-REFERENCE
  baseline'larını <profil>'de eşit kanal-türevli IP-byte bütçesinde geçti; ISOLATION sonucu
  transport-entegrasyon avantajını destekler."
- "Aynı algoritma şart değil" cümlesi (r3) İPTAL — ISOLATION zaten aynı FEC'i kullanıyor;
  REFERENCE'ta fark kabul edilir ama mimari-iddiaya kaynak yapılmaz.
- Dış-terim: "asıl headline rakibi" → **primary comparison baseline**; "aynı teslim garantisi" →
  **same delivery success criterion** (finite bütçe+deadline'da mutlak garanti yok).
- Araştırma sorusu (dış, aynen): "The campaign tests whether Kama retains useful message-delivery
  capability in synthetic high-loss/high-delay/asymmetric/sub-kilobit IP channels where
  preregistered Quinn configurations may experience connection-establishment or recovery
  limitations." İç tez ("düştüğü yerde ayakta") dış-raporda kullanılmaz. Üç-katmanlı win-modeli korunur.

### C.4 Hizmet-semantiği tablosu (her hücrede zorunlu)
güvenilirlik / sıralama / mesaj-sınırları / deadline-sonrası-iptal / dup-suppression / app-ACK /
connection-state — Kama vs ilgili baseline. ACK alanları (r4#10/#11): transport_ack_unit ·
transport_ack_generation_condition · delayed_ack_policy · ack_coalescing_policy · ack_frequency.

---

## D. BÜTÇE (Abkarada karar — operasyonel tam tanım)

### D.1 Türetme (transport-BAĞIMSIZ, ön-kayıtlı)
`B = capacity_bps × mission_deadline_s × permitted_utilization / 8`. **Kama'nın VEYA Quinn'in
ölçülen tüketiminden TÜRETİLMEZ** (dairesellik yasağı). Full-duplex: yönler ayrı
(`B_fwd=C_fwd×D×u_fwd/8`, `B_rev` simetrik). Half/shared-duplex: tek `B_shared=C_shared×D×u_shared/8`.
Kullanılmayan-yön bütçesinin diğer yöne aktarılabilirliği KAPI-1'de normatif sabitlenir.

**Workload-ayrık bütçe biçimi (review r5#1 — tek-seferlik B paced'i açlığa iter):**
- **burst:** one-shot kümülatif hard-cap B (yukarıdaki formül).
- **paced:** token-bucket/windowed (byte cinsinden) — `refill_rate_bytes_per_s = capacity_bps ×
  permitted_utilization / 8`; `bucket_size_bytes = preregistered burst allowance (byte)`;
  `cumulative_ceiling_bytes = capacity_bps × evaluation_window_s × permitted_utilization / 8`.
  Böylece paced gerçekten "sürdürülebilir mesajlaşma" ölçer, ilk mesajların bütçeyi tüketmesi
  engellenir.

### D.1b Deadline↔byte ortogonalizasyonu (review r5#2)
`B(D)=C×D×u` gereği D büyürken hem süre hem byte artar. **Primary operational endpoint, deadline
büyüdükçe zamanı ve izin verilen byte bütçesini bilinçli olarak BİRLİKTE değerlendirir; iki etkinin
nedensel ayrımını YAPMAZ.** Bu ayrım İKİNCİL analizde iki ortogonal eksenle yapılır (ZORUNLU):
`fixed-D, varying-B` ve `fixed-B, varying-D`. Süre-etkisi ile kaynak-etkisi böyle ayrışır.

### D.2 Kapsam
İki yöndeki TÜM data-plane IP byte'ları: original + FEC-parity + ARQ-retx + transport/app-ACK +
probe + padding + duplicate + control. DÜZLEM-A'da önceden-kurulmuş handshake byte'ları HARİÇ.

### D.3 permitted_utilization
Sonuçlar görülmeden seçilir. KAPI-1'de %50/%75/%90 adaylarının operasyonel anlamı + çalışma
süresi kıyaslanır; **Abkarada tek primary değeri onaylar.** Diğerleri yalnız preregistered
sensitivity.

### D.4 Uygulama biçimi
Kritik headline hücrelerinde bütçe **gerçek hard-cap** olarak **pre-impairment egress'te**
uygulanır: tükenince yeni paket gönderilemez, kalan mesajlar deadline'da başarısız. TÜM hücrelerde
ayrıca `delivered_unique_payload vs cumulative_IP_bytes` eğrisi. Payload-türevli {2×,4×,8×}×payload
bütçeleri YALNIZ ikincil diagnostik/sensitivity — primary/ölçüm-sonrası-seçim DEĞİL.

**Packet-atomic hard-cap (review r5#5):** `B_remaining < next_packet_ip_length` ise paket kanala
enjekte EDİLMEZ — partial-packet/overshoot YOK. **Policer pacing'i değiştirmemeli:** ortak
budget-governor transport çıkışında VEYA pre-impairment eBPF/TC hook; genel rate-policer yeni
gecikme/kuyruk ÜRETMEZ (yoksa cap, ölçtüğü şeyi kirletir). İki transport için özdeş governor.

### D.5 Bütçe metrikleri
success_under_budget(B,D) · actual_bytes_consumed · unused_budget · budget_exhaustion_time ·
delivered_unique_payload_bytes / B. "İki sistem de başarılı ama Kama daha az harcadı" → düşük-maliyet
iddiası buradan.

---

## E. FEC-BASELINE BAĞIMSIZ DOĞRULAMA (review r4#8 — KAPI kapısı)
Baseline'ı bizim ekip yazdığı için "zayıf/yanlış implemente" itirazına karşı kabul kapıları:
known-answer test vectors · deterministic erasure tests · decode-boundary (k−1, k, k+r symbol) ·
property/fuzz · **bağımsız lib/reference ile çapraz-doğrulama** · loss-free regression ·
wire-budget accounting test · deadline/ARQ-termination test. Baseline bug'ı bulunursa etkilenen
evaluation kampanyası yeni commit'le BAŞTAN (cherry-pick yasağı burada da).

### E.1 FEC kurulum yolu (KAPI-1 kıyası → Abkarada seçer)
Yol-A (hazır Rust lib: RaptorQ RFC6330 / RS, DATAGRAM üstü sarmalı) vs Yol-B (literatür referans
şeması birebir). Kıyas: iş-günü · dış-savunulabilirlik · eşdeğerlik-zarfı · bakım.
**Yol-A/Yol-B seçimi ESAS OLARAK Q-FEC-REFERENCE implementasyonunu belirler.** Q-FEC-ISOLATION'ın
FEC implementasyonu seçim konusu DEĞİLDİR: Kama'nın kullandığı exact crate/lib, version/commit ve
parametreler ZORUNLU olarak tekrar kullanılır; yalnız QUIC-DATAGRAM entegrasyon katmanı ayrışır ve
her zorunlu sapma deviation tablosuna (C.1) girer. İş-yükü ayrı tahminlenir, duvar-saati bütçesine
eklenir.

---

## F. PROFİL MATRİSİ (tam şema — eksik alanlı profil KAPI-1'den geçemez)

**Şema:** profile_id · motivation(iç) · external_label · duplex_model(full|half|shared) ·
forward/reverse/shared_capacity · one_way_base_delay · jitter · delay_distribution ·
correlation · fwd/rev_RNG(independent|shared) · loss_model(random p | gemodel[p,r,1−h,1−k] |
trace-driven) · queue(discipline,limit_bytes,limit_packets,target_BDP,cross_traffic) · qdisc_order ·
ip_version · path_mtu · max_udp_payload · fragmentation_policy · pmtud_policy · minimum_frame_size ·
message_coalescing_policy.

- **Grup A (v1 mirası):** p1..p7 v1 parametreleriyle + şema doldurulur.
- **Grup B (sentetik saha-motivasyonlu; dış-adlar):**
  h1 ultra-low-rate high-RTT (0.20kbps, %40, one-way 1200ms, jitter 400ms, dağılım+korelasyon
  belirt; RTT ~2.4s, aralık ~1.6-3.2s) · h2 constrained cellular-like (8kbps,%25,jitter300±200;
  cross-traffic yoksa "finite bottleneck queue", bufferbloat DEME) · h3a/b/c burst-erasure
  (250kbit, gemodel TAM-4-param, ÜÇ hücre loss40/50/60; zaman-tabanlı burst→trace-driven) ·
  h4p periodic complete-erasure (%100 1s-ON/2s-OFF, ÜÇ faz: on/off/uniform_random-start) ·
  h4s sustained high-erasure (%85+100kbit) · h4a severe reverse-path (fwd%10/rev%90) ·
  h5 high-erasure cold-start [DÜZLEM-B] (250kbit+%90, ÜÇ RTT: rtt20/200/2000) ·
  h6 high-rate baseline (10Mbit temiz+%1, RTT 20/200ms).
  Dipnot (aynen): "Scenario labels indicate the motivating operational condition; the impairment
  models are not claimed to reproduce the corresponding RF channel." Jammer/trafo/dağ dış-adlarda
  profil-adı değil.
- **Grup C (sub-kbps):** p8=1k/p9=0.5k/p10=0.25k × {temiz,%20,%40}; duplex_model AÇIK (netem
  çift-yön-ayrı = full-duplex; half-duplex telsiz temsil edilmiyorsa yazılır).

---

## G. WORKLOAD + DEADLINE
- **İki workload:** burst (hepsi t=0, "backlog-drain" adıyla) + paced (profil-aralıklı; Sınıf-III
  örn 1msg/30s, survival-messaging).
- **Paced deadline mesaj-başı (r3#6):** `deadline_i = submission_i + mission_deadline`;
  `latency_i = delivery_i − submission_i`; `test_end = max_i(message_deadline_i)` — aynı
  mission_deadline tüm mesajlara uygulanıyorsa `test_end = last_submission_time + mission_deadline`.
  Paced metrikleri: deadline-içi-oran · latency-dağılımı · stale-teslim-oranı · deadline-sonrası-red.
- **Mission deadline (transport-BAĞIMSIZ, r3#5):** iki transport AYNI, mutlak, ön-kayıtlı D-seti
  (profil-sınıfına göre {30,60,120,300} / Sınıf-III {120,300,600}; KAPI-1'de sabit). Hiçbir deadline
  transport wire-maliyetinden türetilmez. Serialization-lower-bound YALNIZ analiz metriği
  (`normalized_completion`), başarı-eşiği DEĞİL; half/shared'da paylaşımlı formül.
- **Mesaj planı:** Sınıf-I 100×256B · II 30×64B · III 10×32B (kapasite-hesabıyla doğrula, KAPI-1).

---

## H. WIRE ACCOUNTING (review r3#4 + r4#4, #9)
- **Capture: qdisc-ÖNCESİ, yön-başı sender-egress** (netem-loss uygulanMADAN önce). İki hook
  (forward_pre_impairment + reverse_pre_impairment); her paket YALNIZ bir kez (çift-sayım yasağı).
  Uç-sayaçları yalnız çapraz-teyit (`A.tx≈B.rx+drops`); toplam formülüne girmez.
- **Üç düzlem ayrı (r4#4):** `sender_injected_ip_bytes` (primary bütçe — enjekte edilen tüm IP,
  netem'in sonra düşürdükleri DAHİL) · `bottleneck_admitted_ip_bytes` · `receiver_observed_ip_bytes`.
  Üç seviye byte: udp_payload / ip_wire / l2_observed. Dış-terim **"IP-layer observed bytes"**.
- **Rapor cümlesi (aynen, r4#4):** "The primary byte budget measures IP bytes injected by the
  endpoints into the emulated channel, including transmissions subsequently erased by the impairment
  model; it is not a measurement of physical RF airtime." ("airtime" kelimesi bütçe-tanımında YASAK.)
- **Offload:** quinn `enable_segmentation_offload(false)` + iki uçta `ethtool -K gro/gso/tso/lro off`;
  komutlar repro-blokta.
- **Decomposition öncelik-kuralı (çakışma-yok):** 1 handshake/control · 2 probe-only · 3 retx-app ·
  4 first-tx-app · 5 ack-only · 6 padding/header/crypto · 7 unattributed. İki eksen ayrı:
  packet-purpose + byte-composition. Otoriter-toplam=pcap; semantik=qlog/iç-trace; reconcile (>%1→açıkla).

---

## I. İSTATİSTİK (review r3#7,#8,#9,#10 + r4#5,#6,#9)
- **Cluster (r3#7):** CI'lar repeat/trace-cluster bazlı; bootstrap = seed/repeat-cluster (mesaj-bazlı
  DEĞİL). "30 repeat×100 msg=3000 örnek" YASAK; n = repeat sayısı.
- **Paired (r3#8):** aynı-trace-replay = paired design. Metrikler: Δsuccess · paired RMST-diff ·
  paired wire-efficiency-diff · paired deadline-delivery-diff. Binary: McNemar/paired-bootstrap.
  Headline formu: "aynı N trace'te teslim farkı +X puan, %95 paired CI [...]".
- **Sıfır-teslim güvenli maliyet (r4#9):** primary paired-maliyet = `delivered_unique_payload /
  total_ip_bytes` (0-teslimde 0) VEYA Δwire_efficiency. `bytes/delivered_msg` yalnız iki-sistem-de
  ≥1-teslimde ikincil; tanımsız koşullar GİZLENMEZ.
- **Multiplicity (r4#5):** matris devasa → primary-endpoint hiyerarşisi ZORUNLU (KAPI-1):
  primary_endpoint (örn `E(B,D)=eşit-B-altında-D-içinde-teslim-edilen-unique-payload-oranı`) ·
  primary_profile_family · primary_workload · primary_plane(A1|A2) · primary_deadline ·
  primary_comparator. Çoklu-primary→Holm / simultaneous-CI / tek-birleşik-endpoint. Exploratory
  sonuçlar "unadjusted exploratory" etiketiyle.
- **Güç-analizi (r4#5):** n calibration-trace'ten paired-power ile: target-power %80/%90 ·
  min-relevant-effect (örn +15pp) · alpha ön-kayıtlı · discordant-pair-rate calibration'dan.
  "Neden n=50?" bilimsel cevaplanır. Taban: geliştirme n=10 · headline n≥30 · dış-iddia-kritik n=50.
- **Randomizasyon (r3#9):** seeded balanced randomization (10-blokta 5/5 Kama-önce/Quinn-önce,
  sıra manifest-seed'iyle karışık) — "dönüşümlü" değil.

---

## J. KANAL ADALETİ (review r3#10 + r4#6, #10)
- **Trace-replay:** zamana-bağlı önceden-üretilmiş impairment trace; AYNI trace iki transport'a.
  Pulse/outage/faz sabit manifestte.
- **Exogenous/endogenous (r3#10):** "impairment-doğrulanamadı" etiketi YALNIZ exogenous mekanizma
  hatasına dayanır; iki transport'un realized-loss'unun eşitsizliği (endogenous) invalidation DEĞİL.
- **Realized-kanal (r3#8):** her repeat: configured vs realized rate/loss/byte-loss/delay-p50-95-99/
  queue-drops/netem-drops/transport-drops. 0.2-0.25kbps'te netem clock-granularity için realized ŞART.
- **ÖN-KOŞUL — ARP/neighbor-pin (TUR-1x bulgusu, 2026-07-13; talimatla eklendi):** TÜM v2 koşuları
  `pin_neigh` (harness/netem-profiles.sh, PERMANENT çift-yön neighbor-pin) İLE koşulur — pin'siz koşu
  GEÇERSİZ. Realized-channel doğrulaması "neighbor-pin'li; **ARP-katmanı drop'ları netem-drop'tan
  AYRI doğrulanır**" şartını içerir. KÖK: ARP istek/probe'ları da egress netem kuyruğuna girer;
  sub-kbps'te çözümleme timeout → kernel arp_queue paketleri SESSİZCE düşer — netem "dropped" sayacı,
  Udp Snd/RcvbufErrors dahil HİÇBİR sayaca görünmez (seq-düzeyi yer-gerçeğiyle kanıtlandı: temiz
  1kbps'te 3/3 koşuda ardışık 2-datagram kaybı; pin sonrası 0). Pin'siz her hücrenin realized-loss'u
  kontamine olurdu — **measurement-tool-first dördüncü kez kampanyayı kurtardı** (kayıt:
  memory/measurement-tool-first + time-model-v1 §0.8 + commit e383832).
- **Calibration/evaluation kör-ayrımı — TEK model seçilir (review r5#4; iki yöntem aynı anda
  uygulanamaz):**
  - **Model A (SEÇİLEN — daha basit+güçlü):** configs-freeze → **public-randomness** ile
    evaluation-seed üretimi → evaluation başlar. (1) trace-generator sürümü+dağılım-parametreleri
    commit · (2) calibration-set ile ayar · (3) tüm transport-configleri commit-kilit · (4)
    evaluation-seed'leri public-randomness'tan SONRA üretilir.
  - **Model B (yalnız üçüncü-taraf mevcutsa alternatif):** bağımsız taraf evaluation-setini üretir
    → encrypted-archive/hash-commitment yayımlar → configs-freeze → arşiv açılır.
  - Default = Model A. İki sisteme aynı tuning bütçesi; configler evaluation görülmeden commit'li.

---

## K. SAAT (review r3#5 + r4#11)
KAPI-1: clock_source · clock_sync_method · max_allowed_offset · max_allowed_drift ·
timestamp_resolution · timestamp_location. Tercih: aynı-host-ayrı-namespace (ortak saat) > PTP >
hw-timestamp. Farklı-host zorunluysa repeat öncesi+sonrası offset; eşik aşımı→repeat GEÇERSİZ.
**h6 caveat (r4#11):** aynı-host-veth "loopback değil ama bağımsız-fiziksel-host da değil" —
rapora açık. h6'da CPU/throughput iddiası yapılacaksa: CPU-pinning · ayrı-fiziksel-core · IRQ-affinity ·
sabit-governor · background-load-kontrol · tercihen iki-fiziksel-host+PTP.

## L. KAMA DATA-PLANE KRİPTO (review r3#13)
DÜZLEM-A'da Kama data-frame gerçek-AEAD mi +16B-maket mi açık. Gerçekse algoritma/provider PİNLE;
modeled ise h6/yüksek-bant CPU/throughput üstünlük-iddiasında KULLANILMAZ. CPU-zamanı+core-util
AYRICA ölçülür (iki transport). App-ACK simetrisi (r4#10): payload-boyu/aggregation/delay/retx/
FEC-koruması/deadline/dup-suppression iki tarafta aynı; Kama app-kabulü SACK ile dolaylıysa
harness'in ürettiği bağımsız aynı-format app-ACK kullan.

---

## M. F-KAPISI AYRIMI (review r2#15)
engineering_regression_gate (p2/p3 monotonik-recovery, TV-COLDSTART — release-blokör, kampanya-dışı)
vs comparative_experiment (bu kampanya, sonuç-zorunluluğu yok, tüm sonuçlar tutulur). Motor fix
gerekirse: "commit-A failed campaign" kapanır, commit-B baştan (cherry-pick yasağı).

---

## KAPI-1 ÇIKTI LİSTESİ (tablo halinde bana; onaysız koşu yok)
1. **FEC:** Yol-A/B kıyası → seçim; ISOLATION vs REFERENCE iş-yükü ayrı; ISOLATION'ın Kama-eş
   FEC **aynı-implementation eşlemesi** (crate/commit/symbol/block/mapping/trigger/param) +
   **deviation tablosu**; E'deki 8 doğrulama-kapısı planı.
2. **Bütçe:** permitted_utilization %50/%75/%90 kıyası → primary öneri; yön-arası-aktarım kuralı;
   **paced token-bucket parametreleri** (refill/bucket/ceiling) vs burst one-shot; **packet-atomic
   hard-cap governor** tasarımı (pacing-nötr, eBPF/TC-hook); delivery-vs-byte eğri üretimi;
   fixed-D/fixed-B ortogonal sensitivity ekseni.
3. **Düzlem:** A1 barrier-protokolü + A2 warm-up-kirlenme kuralları; 14 established-state alanı dolu;
   headline A1/A2 seçimi; reconnect kuralı (+DÜZLEM-C kararı).
4. **Capture:** qdisc-öncesi hook şeması; üç-düzlem byte tanımı; çift-sayım-yok kanıtı;
   kayıp-paket-dahil cümlesi.
5. **İstatistik:** primary-endpoint hiyerarşisi + multiplicity(Holm/tek-endpoint) + paired-power
   n-gerekçesi + cluster-bootstrap + sıfır-teslim-güvenli maliyet metriği.
6. **Kanal:** trace-generator + calibration/evaluation kör-ayrım **Model A** (public-randomness
   kaynağı seçimi; üçüncü-taraf varsa Model B) + realized-kanal log şeması + exogenous/endogenous eşik.
7. **Saat:** 6 alan + tercih + geçersizlik eşiği + h6 CPU-izolasyon planı.
8. **Semantik+packetization:** ACK-birim/generation/coalescing/frequency + 7 MTU/IP alanı (özdeş zarf).
9. **Profiller:** tüm şema alanları dolu; mesaj planları; mission-deadline setleri.
10. **Duvar-saati bütçesi:** FEC mini-ADIM'leri (ISOLATION+REFERENCE ayrı) DAHİL toplam süre +
    hücre-bazlı n tahsisi.

## DEĞİŞMEZ GÜÇLÜ YANLAR (koru)
Başarısız-hücre-düşürme yasağı · tuning'de-yeni-hücre · ön-kayıt-commit · v1-dokunulmazlığı ·
per-repeat-ham · netem-reaktif-sınır-dürüstlüğü · seed/image-hash · quinn-konfig-şeffaflığı
("hasma en iyi silah" YASAK → "reference config preregistered to avoid knowingly disadvantaging") ·
DPI/ölçülmemiş-güvenlik kapsam-dışı · c(t)+Kaplan-Meier ana-çıktı · iki-düzlem · sentetik-dış-adlar.

## YASAKLAR
FEC-baseline'sız / ISOLATION+REFERENCE-uyumsuz "QUIC mimarisine üstünlük" · ölçüm-sonrası-baseline-
seçimi · bütçeyi Kama/Quinn-tüketiminden türetmek · "airtime" demek · capture'ı qdisc-sonrasına
koymak · mesaj-bazlı-örneklem · unpaired-headline · calibration-trace'te-headline · modeled-AEAD ile
yüksek-bant-CPU-iddiası · gizli-reconnect · A1/A2-ön-kayıtsız · multiplicity-düzeltmesiz-headline ·
cherry-pick-rerun · onaysız-profil/plan-değişikliği · hücre-düşürme · v1'e-dokunma ·
kampanya-sırasında-motor-koduna-dokunma · payload-bazlı-maliyet.

---
*r5 sonu. r1/r2/r3/r4 arşiv/superseded. KAPI-1 tabloları → onayım → koşu. Koşu-öncesi son karar
noktaları: FEC-yol seçimi (yalnız REFERENCE), permitted_utilization primary, headline-düzlem A1/A2.*
