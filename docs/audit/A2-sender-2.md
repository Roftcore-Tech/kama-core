# A2-sender-2 — `sender.rs` 4801–9600 davranış denetimi

Taban: ürün hattı `125b9afa` (aday `1a4f9efc` bunun içinde). Satır numaraları bu tabandandır. Salt okuma; kaynak dosya, cargo, git ve GitHub'a yazılmadı.

## Kapsam

| dosya | aralık | okunan satır |
|---|---|---|
| `crates/adaptive-transport/src/sender.rs` | 4801–9600 (atanan üretim kodu; 9600 `#[cfg(test)]` başlangıcı) | 4 800 |
| `crates/adaptive-transport/src/sender.rs` | 2725–2834 (bağlam: `cc_on`, `fate_key`, `cc_window`, `cc_feedback_floor`, `cc_feedback_cap`, `cc_fits`) | 110 |
| bağlam (yalnız ilgili satırlar): `budget.rs` 26–54, `arq.rs` 69–92, `pacer.rs` 130–150 ve 240–262, `client-core/src/runtime.rs` 340–390, `Cargo.toml` `[features]` | — | ≈ 130 |

Toplam okunan kod satırı: ≈ 5 040 (atanan aralığın tamamı satır satır).

## Davranış envanteri

### A. Kayıp olayının sonu ve FEC karar zinciri (4809–5145)

- **4811–4815** — kayıp baytı varsa port motoruna `cc.on_congestion_event(lost)` (BBR `recovery_window`). Uyumlu: INV-RATE-1 2026-11-11 metni.
- **`fuzzy_evaluate` (4822–5145)** — ölçümden `FusionInputs` kurar, fuzzy kararı ve FEC oranını üretir. Adımlar:
  - `ack_epoch = max(srtt|INITIAL_RTT=100 ms, EAI)`; ledger tick: `W_loss = K_WL(8) × ack_epoch`, onay ufku `CadenceLegacy` (cadence tabanı) ya da `ChannelDerived{k‰}` (yalnız ölçülmüş srtt; yoksa legacy'ye düşer). Uyumlu: §5 ledger, §16.1 `K_wl`.
  - `jitter_ppm = rttvar/srtt`, `qd_ratio = qd×1000/rtprop` (rtprop yoksa 0 + sayaç). Uyumlu: §16.2 fuzzy qd satırı.
  - `fuzzy.decide` → `d`; sıfır ağırlıkta `diag_fuzzy_no_rule++`.
  - **Spekülatif kol (FEC-RISK v2, 4877–4915):** `fec_loss_risk.enabled` iken `loss.fec_loss_risk_split(now, srtt, weights, persist_min)`; ufuk `FixedSrtt` (α·srtt) ya da `LearnedSpuriousAge` (Q_q × ρ, tavanlı). Ürün varsayılanı açık (`FecLossRiskConfig::bounded_v1()`).
  - **Politika kolu (4924–4980):** `fusion_policy.is_enabled()` iken kanonik öneri politikanın `FecAssist` oranı; spekülatif tavan = sert tavan (FecAssist, ColdStart, LowConfidence, NoDeadlineDeclared) ya da 0. Ürün varsayılanı kapalı (`fusion_policy: None`).
  - **Yapı kapısı (4981–4993):** politika kapalıyken `structure_gate_fec(fuzzy_fec, loss_ppm)`; histogram + açılma sayacı.
  - **Spekülatif öneri (4995–5026)** ve L1/L2 ilk anı enstrümanları.
  - **Tek arbiter (5032):** `final = max(confirmed, structure, speculative)`; toplama yok.
  - **§8.5 bütçe kıskacı (5051–5085):** `BudgetConfig::fixture()` (profil 900‰, `budget_ref` 125 000 B/s); BtlBw = 0 iken tavan profil tavanı; kıskaç oranı sıfırlayacaksa uygulanmaz ve `would_zero++`. Uyumlu: §8.5 AMENDMENT-3.
  - **Kuruluş koruması (5086–5092):** PreConverge'de arbiter çıkışı 0'a çekilir. Uyumlu: §3.3-C/D ruhu, transport-v1 denetimi.
  - Teşhis anlık görüntüsü (`TsSample`, 5096–5122; `diag_sched_summary()` aynı çağrıda altı kez hesaplanır — performans notu), `no_rule` temizliği, `FusionInputSnapshot` (5134–5143).
- **`structure_gate_fec` (5152–5234)** — fuzzy 0 iken: PreConverge → engelle; `loss < 100 000 ppm` → engelle; pencere `n < 32` → engelle. `Q18Iid` (varsayılan): `burst_delta ≤ 50 000 ppm` → `fec_med` (ya da override), değilse ARQ'ya bırak. `BurstFusion`: `n10+n11 ≥ MIN_LOSS_TRANSITIONS` kanıtı ister; burst dalında gereken = `loss × 3/2 / 1000` ‰, seviyeyi aşarsa `fec_high`.

### B. Durum makinesi ve hız (5245–5631)

- **`update_state` (5245–5470)** — sağlıktan acil durum seviyesi ve `last_recovery`; öncelik IDLE > PreConverge (srtt varsa Startup) > RecoverFwd (acil) > RecoverArq (bekleyen kayıp; E1 `e1_probe_keep` açıkken ProbeUp'ı kesmez) > faz içi. Faz içi: Startup → plato → Drain; Drain çıkışı `inflight ≤ CWND_GAIN×BDP ∨ dwell ≥ 3×epoch`; DrainRtprop çıkışı `(≥1×epoch ∧ qualifying) ∨ ≥3×epoch`; ProbeUp başarı `delivered ≥ btlbw_e×2×epoch_e ∧ btlbw > btlbw_e×1,025`, abort `qd > 500‰ ∨ prob kaybı (E1 bayrağı kapalıyken)`, zaman aşımı `4×epoch_e`; Steady → ProbeUp `periyot ≥ 4×epoch ∧ qd ≤ 500‰ ∧ backlog`; Recover* çıkışı tabana (Startup tabanı korunur). ProbeUp kesilmesi ayrı sayılır; Startup gerçek girişinde plato sayaçları sıfırlanır; faz geçişi telemetrisi. Uyumlu: §3 geçişleri, AMENDMENT-1, §16.1 ProbeUp/Drain/DrainRtprop satırları, TUR-7 β1.
- **`qd_ratio_permille_now`, `probe_close_success`, `probe_close` (5473–5499)** — qd/rtprop oranı; ProbeUp çıkış muhasebesi.
- **`startup_plateau` (5506–5544)** — teslim çapalı tur kapısı (ZM-1); E1 bayrağıyla uygulamayla sınırlı tur sayılmaz (qd < E1 freni); büyüme `> %5` (50‰) ise sayaç sıfır; `3` durgun tur → plato.
- **`compute_send_rate` (5548–5580)** — yalnız eski motor (`apply_legacy_rate`): PreConverge → 31 B/s; aksi `effective_send_rate(…, phase.gain, fuzzy gain)` (E1 açıkken cwnd kazancı 2,885); `send_rate_final(rate, min_rate, u64::MAX)`.
- **`pacing_phase` (5584–5597)** — faz→kazanç; Recover* Startup tabanında Startup kazancı. Uyumlu: §16.1 `base_phase`.
- **`e1_startup_cwnd_gain`, `e1_retx_state_ok`, `e1_mark_app_limited_round` (5604–5631)** — E1 bayrakları (ürün yolunda kapalı); port motorunda yeniden iletim RecoverFwd dışındaki her durumda.

### C. Zamanlayıcı (5639–5704)

- **`schedule_transmit`** sırası: (bütçe rezervi — özellik kapalı) → uygulamayla sınırlı tur işareti → öncelikli şerit (`drain_forward_copy_priority`) → ARQ önce (E1 `retx_first` ∨ port motoru ∨ her 4. izin) → yeni veri → durum dalı (RecoverArq ARQ, RecoverFwd ileri kopya, E1 ARQ) → PTO kuyruk probu → kuruluş probu → vadesi gelen kopyalar → FEC paritesi → boş hat yeniden iletimi → (bütçe settle, soğuk başlangıç probları — özellik kapalı) → bootstrap probu.

### D. Yapılandırma, teşhis, kopya kuralı (5712–6236)

- `set_fec_actuation_mode` (her zaman `Ok`), `declare_delivery_deadline` (oturum boyu en küçük değer, sıfırlanmaz), `tightest_deadline_over_srtt` (‰), teşhis erişimcileri (5750–5808), `set_fusion_policy`, `set_initial_window` (kalibre değil).
- `set_cc_initial_window` — port BBR'ı verilen ilk pencereyle yeniden kurar. `set_cc_profile_pacing_cap` — 0 = yok.
- **`set_cc_feedback_starved` (5842–5859)** — kural geçerliyse pacer'a profil hattını kurar; dönüş hattı varsa `LineGuard::new(line, return_line, conservative_start(line))` ve pacer hat hızını `g.pace_bps()`'e (temkinli başlangıç) indirir.
- İz kaydı (`EMIT_LOG_CAP` 65 536), LineGuard ve şerit teşhisleri.
- **`set_forward_copy_rule` (5940–5947)** — `copies > 0 ∨ öncelik kapsamı` değilse kural yok; öncelik sınırı ≥ kopya sınırı.
- **`schedule_forward_copies` (5957–5994)** — teyitsiz uçuş kaydı varsa `copies` tek atışlık kopya, `t0 = şimdi + hat boşalma`, aralık `spacing_ns`.
- **`schedule_forward_copies_guarded` (6007–6036)** — profilde birim `until = t0 + copies×spacing + feedback_ns + RTT` anına dek PTO hedefi olmaz, PreConverge merdiveni ertelenir; tavan `COPY_GUARD_CAP` 64 (dolunca koruma sessizce kurulmaz, `true` döner).
- **`mark_single_shot` / `single_shot` (6057–6075)** — profilde tek iletim işareti, tavan 64, dolunca sayaç + `false`.
- `forward_copies_now` (hat ölçüsüyle ölçekli kopya sayısı), `set_cc_line` (yalnız dar hatta: tam datagram serileşmesi > 500 ms), `line_is_narrow`, `cc_line_observe_backlog` (paylaşılan hat kuyruğu tabanı), `cc_line_charge` (pacer dışı datagram hatta ve LineGuard'a), `base_pto_ns` (RFC 9002 §6.2.1), `apply_cc_line_floor`.
- FEC-RISK, persist, arbiter teşhisleri (6219–6287).

### E. Spekülatif öneri ve teşhis (6309–6743)

- **`speculative_propose` (6309–6369)** — `instant < l1_min` ya da politika tavanı 0 → yok; reorder baskısı eşiği → yok; L2 (persistent) / L1 (bootstrap; sıkı bütçede persistent tavan); kanonik öneri varsa bootstrap tavanına iner; reorder indirimi (veto değil), `spec_min` tabanı, politika tavanı.
- `diag_sched_summary` (özellik kapalıyken sıfır), **`emergency_framing`** (Controller → SegmentBlock; Manual{açık} → SegmentBlock; Manual{kapalı} → Tiny), aktivasyon / kurtarma zamanlayıcısı / birim teşhisleri (salt okuma).
- `set_next_ack_evidence` (0x15 kanıt aralığı; ürün runtime'ı çağırır), `note_ack_arrival_rate`, `note_ack_pair_rate`, **`note_ack_forward_interval`** (uçlar M'de ve gönderilmiş seq'lerde olmalı; `received_wire` LineGuard'dan), LineGuard iz halkası (`LINE_GUARD_TRACE_CAP` 64).
- **`start_budget_holds` / `gap_budget_holds` (6685–6708)** — LineGuard başlangıç / boşluk bütçesi öncelikli olmayan iletimi bekletir.
- **`observe_ack_feedback` (6713–6719)** — ACK tel boyunu LineGuard'a yazar, dönüş yolu gözlemini besler.
- Onay ufku / yapı kapısı seviyesi / yapı politikası ayarları.

### F. FEC otoritesi (6790–7096)

- `open_fec_block_authority` (kimlik + monoton nesil + mod tanığı), `set_manual_fec_enabled` (Controller'da tipli ret), `fec_emission_enabled`, **`push_parity_authorized`** (tek kuyruk girişi; profil yolunda bloğun son kaynağı gönderilene dek serbest bırakma kapısı; kuyruk tavanı yok), `close_fec_block_manual` (`manual_r` kadar parite, kırpma çağıranda), `controller_decision_snapshot`.
- **`fec_controller_latch` (6916–7042)** — yabancı tanık → ret; Manual tanık → 0; snapshot yok → 0; olay uyuşmazlığı → 0 (aynı olay nedenselliği); `no_rule` → 0; acil durumda `MinimalParity` varsa onun oranı (§11 / AMENDMENT-3), yoksa 0 + `EmergencyPendingArbitration`; normalde `compute_requested_r` kırpılmış r. Yuvarlanıp 0 olan öneri sayılır. Uyumlu: §11 AMENDMENT-3, ADR-minimal-parity-exception.
- `enqueue_fec_parity` (deprecated, ürün çağıranı yok).

### G. Teyit muhasebesi (7100–7495)

- `record_confirmed_logical` / `insert_sent_logical` / `record_recovered_logical` — sınırlı defterler (`CONFIRMED_LOGICAL_CAP` 4 096, `SENT_LOGICAL_CAP` 16 384; en eski düşer + sayaç).
- **`on_recovered_control` (7154–7227)** — kurtarılan birim: logical teyit, bloğun bekleyen paritesi iptal, gönderilmemiş DATA kuyruktan çıkar, `cancel_confirmed_units`; BtlBw/pacer/teslim kredisine dokunmaz. Uyumlu: fec-core §10 PATCH-6 (RECOVERED bant kredisi üretmez).
- `abandon_units` (son tarihli PTT birimi bırakma), **`cancel_confirmed_units`** (bekleyen kayıp ve kopyalar silinir; uçuştaki kopyalar `confirmed_release_ns = şimdi + 8×T_loss`), **`release_confirmed_copies`** (süresi dolan kararsız kopya kayıp ilanı olmadan uçuştan düşer), teyit çıktısı tamponu (`CONFIRMED_UNITS_CAP` 4 096, çağrı girişinde kırpılır).

### H. Parite emisyonu ve deneysel bütçe (7502–8013)

- **`plan_fec_parity` / `try_emit_front_parity` (7502–7562)** — başlangıç bütçesi, `cc_fits` + `pacer.try_send`; ret teşhisi. Uyumlu: fec-core §8 "repair: pacer-bypass-YOK · cwnd-dışı-YOK".
- **`drain_released_parity` (7574–7592)** — yalnız profil + port motorunda: kaynakları gönderilmiş bloğun paritesi yeni verinin önüne geçer.
- `emit_parity_at` — parite kaydı birim kimliği = ESI; logical kayıt; `SrPacketClass::Repair`; `.expect` iç değişmez.
- `experimental-fec-budget` yöntemleri (7653–7965; varsayılan derlemede yok): politika geçişi, rezerv, harcama, bilet servisi, settle, blok kabulü (sabit kabul tavanları 64 / 8 / 8 / 2×MTU).
- **`try_send_probe`** (pencere kapısı yok; RFC 9002 §6.2.4) ve **`try_send_pre_parity`** (`cc_fits` + pacer).

### I. ARQ, PTO, kopyalar (8015–8758)

- **`plan_arq_retransmits` (8015–8098)** — `arq.plan(lost, aggressiveness)`; boşluk bütçesi; jeton yoksa dur; orijinal tombstone'u superseded; `CapExceeded{HandToForward}` → tek atışlık kopya; `ConnectionFail` → kayıt sessizce düşer.
- `retire_inflight_to_tombstone` (eski motor PTO yolu), **`cc_bound_unit_copies`** (birim başına uçuşta ≤ 8 kopya).
- **`emit_tail_probe` (8172–8293)** — port motorunda: önce daha küçük kimlikli bekleyen yeni veri (`emit_new_data_probe`, RFC 9002 §6.2.4 "SHOULD send new data"), sonra ARQ kuyruğundaki hedef, sonra uçuştaki hedef (uçuşta kalır, RFC 9002 §6.2 + §7.5). Eski motorda hedef tombstone'a emekli. Uyumlu: CC-PORT, RFC 9002.
- `e2a_arm_second_probe` / `e2a_emit_second_probe` / `e2a_probe2_ready_ns` — ikinci prob (ürün yolunda `pto_probes = 1`), port motorunda RTT örneği yokken tek prob.
- `oldest_probe_target` / `oldest_probe_target_or_pending` — FEC, kararsız, korunan ve tek iletimli kayıtlar hedef olmaz.
- **`plan_forward_duplication` (8526–8566)** — RecoverFwd: bekleyen kayıp yoksa uçuştaki teyitsiz DATA'ya canlılık kopyası (aralık srtt, geometrik); varsa her kayıp için `factor` tek atışlık kopya (`dup_params`). Uyumlu: §11, §5(b) TUR-1a liveness.
- **`drain_establish_probe` (8579–8644)** — PreConverge'de vadesi gelen en eski kopyadan tek datagram, **jeton ve uçuş penceresi muaf**; aralık 500 ms'den ikiye katlanır, tavan 8 s; hattı terk anına göre.
- **`drain_due_duplicates` (8660–8728)** — profil + port motorunda vade sırası, diğer yolda kuyruk sırası; `try_send_pre_parity`; `interval_ns > 0` ise ×2 yeniden planlanır (tavan yok).
- **`drain_idle_retransmit` (8759–8835)** — profilde ve bütün kuyruklar boşken, son iletimi ≥ 1 s önce olan, kopya kuralı kapsamı dışında, kanıt beklemeyen en eski teyitsiz birimin kör kopyası; tur başına ≤ 4; sonraki deneme hattın yarısı kadar sonra.

### J. Öncelikli şerit ve yeni veri (8846–9304)

- `priority_lane_head`, **`queue_priority_head_hole`** (profilde şerit başının en yeni iletimi en büyük teyitliden önceyse paket/zaman eşiği beklemeden ARQ kaydı), **`drain_forward_copy_priority`** (şerit başı ARQ → öncelikli yeni veri → hat kuyruğu ≤ 200 ms iken vadesi gelen öncelikli kopyalar, en eski vade önce), `drop_stale_priority_copies` (1 s'ten bayat kopya, birimin vadesi en geç kopyası kalır).
- **`drain_new_data` (9096–9135)** — profilde serbest parite önce; başlangıç bütçesi; `try_send_pre_parity`; `after_new_data_sent`.
- **`after_new_data_sent` (9141–9220)** — kopya kuralı (RecoverFwd dışında); RecoverFwd'de `factor−1` kopya; PreConverge'de §3.3-B ilk kopya `RTT_floor × 3/2` (`RTT_floor = (birim + 65)/31 B/s`, hattı terk anından), bootstrap / soğuk başlangıç kurulumu. Uyumlu: §3.3-B [K-D2] t₀ = tel çıkış anı.
- `dup_params` (son kurtarma eylemi; yoksa 2 × 20 ms).
- **`track_sent` (9230–9304)** — seq, ilk gönderim, bw örneği (`gain_ge1` damgası), sınıf sayaçları, yalnız DATA kayıp paydasına, DATA sırası, CC `on_sent`, uçuş baytı, LineGuard `on_sent_classed` (öncelik işaretli), uçuş kaydı.

### K. Güven ve gözlemciler (9307–9598)

- **`ack_conf_active` / `ack_confidence_permille` (9307–9353)** — PreConverge ve iş yokken 1000; port motorunda çapa `max(last_ack, first_send)`, eski motorda `last_ack.or(first_send)`; eşik `max(3×srtt, 2×EAI)`; aşılırsa `eşik×1000/sessizlik`.
- Gözlemciler (9359–9587): durum, uçuş, karar, yol kestirimi, kuyruk, BtlBw (`measured_btlbw_bps`), son teslim hızı, kayıp/RTT/ledger/qd/tombstone teşhisleri, `path_timing_snapshot`. Hepsi salt okuma.
- `Default`, `dur_ns` (doygun ns).

**Uyumlu davranış sayısı: 48.** En önemlileri: kayıpta `on_congestion_event` (INV-RATE-1 2026-11-11); PreConverge'de parite kesimi; §8.5 AMENDMENT-3 sıfırlama reddi; §11 MinimalParity latch'i; §3 durum geçişleri ve öncelik sırası; DATA sonrası recovery'de pacing kazancının tabandan gelmesi; RECOVERED'ın bant kredisi üretmemesi; parite emisyonunun pacer + uçuş penceresinden geçmesi; PTO probunun pencere muafiyeti ve hedefin uçuşta kalması (RFC 9002 §6.2.4); §3.3-B kopya t₀'ının tel çıkış anı olması; bütün yapılarda bounded tavanlar (tombstone, defter, iz, koruma, tek iletim); INV-ENGINE-SOC (AEAD/anahtar yok; yalnız `block_id/esi` metadata).

## Bulgular

| fid | tür | önem | konum | davranış | otorite | düzeltme türü |
|---|---|---|---|---|---|---|
| A2S2-01 | TANIMSIZ | orta | 4877–4915, 4995–5032, 6309–6369 | FEC-RISK v2 spekülatif kol ve `max()` arbiter ürün yolunda açık (runtime `bounded_v1`); L0/L1/L2, persist ufku, reorder indirimi, `spec_min` tabanı | engine §8.4/§10.2 yalnız fuzzy oran + §8.5 clamp tanımlar; spec'te bulunamadı (aranan: arbiter, spekülatif, FEC-RISK) | karar-gerekir |
| A2S2-02 | TANIMSIZ | orta | 5152–5234 | Yapı kapısı `Q18Iid` ürün yolunda açık; kanıtsız pencerede Laplace prior'la FEC açar; sabitler 100 000 ppm / 32 / 50 000 ppm / 3/2 türetilmemiş | spec'te bulunamadı (aranan: yapı kapısı, Q18, burst_delta); kodun kendi yorumu sahip kararı ister | karar-gerekir |
| A2S2-03 | UYUŞMAZ | orta | 5729–5745 (yorum 4936) | `tightest_deadline_ns` oturum boyu en küçük değerdir, hiç sıfırlanmaz; bir kez sıkı bütçe bildiren mesajdan sonra spekülatif kol L1'de kalıcı olarak persistent tavanı kullanır | kodun kendi sözleşmesi: "Uçuştaki EN SIKI teslim bütçesi"; deadline ekseni spec'te yok | kod-duzeltmesi |
| A2S2-04 | UYUŞMAZ | orta | 8579–8644 | Kuruluş probu jeton ve uçuş penceresi muaf gönderir | engine §3.3-B: "Kopyalar da AYNI pacer'dan geçer (§13); pacer'ı baypas eden gönderim yolu YOK" (ürün profili belgesi SHIP diyor, otorite değil) | karar-gerekir |
| A2S2-05 | EKSİK | orta | 8630–8636, 8715–8726, 9185–9194 | §3.3-B geometrik kopya aralığı tavansız ikiye katlanır; `COPY_MAX_INTERVAL` ve `RETRY_BUDGET` yok | engine §3.3-B "aralık COPY_MAX_INTERVAL'e kadar büyür"; §12 RETRY_BUDGET; §16.2 satırı "COPY_MAX_INTERVAL/RETRY_BUDGET kodda yok" | karar-gerekir (birim başına son tarih motorda yok) |
| A2S2-06 | EKSİK | orta | 6833–6853, 6877 | `pending_fec` kuyruğu tavansız; parite jetonla sürekli reddedilirse blok sayısıyla büyür; `close_fec_block_manual` `manual_r`'yi kırpmaz | engine §14 "Tüm pool'lar sabit-üst-sınır"; SR-AUDIT-001 | kod-duzeltmesi |
| A2S2-07 | EKSİK | düşük | 5986, 8541, 8558, 9152, 9165, 9186 | `dup_queue` için açık tavan yok (uçuş ve birim sayısıyla dolaylı sınırlı) | engine §14; SR-AUDIT-001 | karar-gerekir (A2S2-05'e bağlı) |
| A2S2-08 | UYUŞMAZ | düşük | 9223–9228 | Varsayılan kopya aralığı 20 ms mutlak | engine §16.2: "dup_spacing=20ms wall-clock … gereken k·birim-serialization"; ZM-2 | karar-gerekir (k [CAL]) |
| A2S2-09 | TANIMSIZ | orta | 8759–8835 | Profilde ACK'siz kör yeniden iletim yolu; `IDLE_RETX_MIN_AGE_NS` 1 s mutlak, tur başına 4, hat payı 1/2 | spec'te bulunamadı (aranan: boş hat yeniden iletimi); §9/§11/§12 tanımlamaz; ZM-2 mutlak süre | karar-gerekir |
| A2S2-10 | UYUŞMAZ | orta | 5842–5859 (bağlam 2775–2801) | LineGuard temkinli başlangıçta pacer hat hızını profil hattının altına kurar; `cc_feedback_cap` tavanı ölçülen hatta indirir, uçuş tabanı `hat × (ufuk + RTT)`'nin altına düşer. Hız, uçuş ve yoklama BBR ile LineGuard arasında bölünmüş | CLAUDE.md YAMA 2026-11-16: "tavan en az hat × (geri bildirim ufku + RTT), pacing profil hattıyla sert sınırlı"; sahip kararı 5 / K2′ (7171705736, 7172579876) LineGuard tavanını varsayar | karar-gerekir |
| A2S2-11 | ÖLÜ | düşük | 5261–5425, 5506–5544, 9245–9250 | Port motorunda eski SRC faz makinesi (Startup/Drain/ProbeUp/DrainRtprop) hız sürmez (`compute_send_rate` yalnız eski motorda); yalnız teşhis ve tüketicisi arşivlenmiş `gain_ge1` damgasını besler | CLAUDE.md YAMA 2026-11-11 (pace = BBR); §3 | karar-gerekir |
| A2S2-12 | UYUŞMAZ | düşük | 9341–9344 | `ack_confidence` eşiği `max(3×srtt, K_CONF_EAI×EAI)` | engine §5.1: "threshold = ACK_SILENCE_MULT × srtt"; `K_CONF_EAI` spec'te bulunamadı | karar-gerekir |
| A2S2-13 | UYUŞMAZ | düşük | 9333–9337 | Eski motorda çapa `last_ack.or(first_send)` | engine §5.1 `max(last_ack_time, first_send_time)`; §16.2 "ack_confidence anchor `.or()` … spec formülü" | kod-duzeltmesi |
| A2S2-14 | EKSİK | düşük | 5579 | Eski motor hızının üst sınırı `u64::MAX` | engine §8.5: `send_rate_final = clamp(fuzzy_send_rate, [MIN_RATE, min(BtlBw_based_cap, profile_cap)])` | karar-gerekir (`profile_cap` değeri tanımsız) |
| A2S2-15 | EKSİK | düşük | 8093 | `CapExceedPolicy::ConnectionFail` kaydı sinyalsiz düşürür ("gerçek impl bağlantı-fail sinyali verir"); ürün yapılandırması bu politikayı seçmiyor | engine §9.1 "FEC/forward'a devret veya connection-fail"; CLAUDE.md §6 tipli hata | kod-duzeltmesi |
| A2S2-16 | UYUŞMAZ | düşük | 5051–5052 | §8.5 kıskacı her çağrıda `BudgetConfig::fixture()` kullanır, `self.budget_cfg`'yi okumaz; yapılandırılan bütçe yok sayılır | engine §8.5 `budget_cap = f(BtlBw)` [CAL]; aynı dosyada `compute_send_rate` `self.budget_cfg` okur | kod-duzeltmesi |
| A2S2-17 | UYUŞMAZ | düşük | 5639–5704 | DATA önce; port motorunda yeniden iletim önce, parite (profil dışı) yeni veriden sonra | engine §13 "priority: retransmit(deadline) > FEC parity > yeni DATA"; §16.1 `K_ARQ_SHARE` satırı "DATA-önce ölçülü-kararı" — spec içi gerilim | karar-gerekir |
| A2S2-18 | UYUŞMAZ | orta | 9025–9055, 9065–9093 | Öncelikli şeritte §3.3-B geometrik kopya (`interval_ns > 0`) çıkınca yeniden planlanmaz, bayat diye düşebilir | engine §3.3-B "ateşlenince ×COPY_MULTIPLIER … deadline'a dek" | kilitli (karar 3, 7171705736) |
| A2S2-19 | TANIMSIZ | orta | 5940–6113, 7235–7250, 8846–9093 | PTT/öncelikli şerit mekanizmaları: ileri kopya kuralı, kopya koruması, tek iletim, son tarihte bırakma, şerit başı deliğinde RFC 9002 eşiği beklemeden ARQ, `COPY_LINE_BUDGET_NS` 200 ms / `COPY_STALE_NS` 1 s mutlak | spec'te bulunamadı (aranan: öncelikli şerit, PTT); RFC 9002 §6.1 eşikleri | kilitli (karar 3) |
| A2S2-20 | UYUŞMAZ | düşük | 6710–6719, 6544–6555, 5070–5073, 8164–8171, 8441–8447, 7967–7970 | Belge ile davranış ayrışıyor: `observe_ack_feedback` "motoru DEĞİŞTİRMEZ" der ama LineGuard'ı besler; `set_next_ack_evidence` "YALNIZ ÖLÇÜM PROTOTİPİ" der ama ürün runtime'ı çağırır; `would_zero` yorumu "owner kararı açık kalem" der (AMENDMENT-3 ile kapandı); prob belgeleri `try_send_pre_parity` der, kod `try_send_probe` kullanır; §9 belgesi yanlış işleve bağlı | CLAUDE.md §9 (açık mühendislik dili); engine §8.5 AMENDMENT-3 | kod-duzeltmesi |
| A2S2-21 | ÖLÜ | düşük | 7084–7096, 7650–7965, 5696–5699, 7158–7168, 9207–9218 | `enqueue_fec_parity` (deprecated, ürün çağıranı yok); `experimental-fec-budget` / `experimental-cold-start` kodu varsayılan derlemede yok; politika, BurstFusion, ChannelDerived, E1, E2a kolları ürün yapılandırmasında kapalı | CLAUDE.md §6 (özellik bayrağı izinli; deprecated içerik appendix) | karar-gerekir |
| A2S2-22 | TANIMSIZ | düşük | 5509–5543, 7912–7918, 7782–7786 | Türetilmemiş sabitler: plato 3 tur / 50‰; bütçe kabul tavanları 64/8/8/2×MTU; `oldest_ticket_age` yaş değil gecikme hesaplar (`deadline.saturating_sub(0)` etkisiz) | engine §3 "Δ<ε, N round" (değer yok, §16.1'de satır yok) | karar-gerekir |
| A2S2-23 | EKSİK | düşük | 8526–8566, 9185–9194, 8579–8644 | AMENDMENT-9 K = 1024 PN tavanı §3.3-B ve §11 yollarında kodda yok | transport-layer-tdd §12 AMENDMENT-9 (SPEC-ONLY); engine §3.3-B, §11 bağ notu. DOĞRULANAMADI: tavan motor seq'ine mi seam PN'ine mi uygulanır | karar-gerekir |
| A2S2-24 | DOĞRULANAMADI | düşük | 5054–5061 | BtlBw örneği yokken kıskaç tavanı profil tavanı (900‰) | engine §3.3-D "FEC/redundancy agresifliği bw-kanıtıyla kademeli açılır"; AMENDMENT-3 ADR bu davranışı kabul ediyor | karar-gerekir |
| A2S2-25 | UYUŞMAZ | düşük | 7597–7600, 8080 | Üretim yolunda `.expect` (iç değişmezle korunur) | CLAUDE.md §6 erişilebilir panik yasağı; §9 tipli hata | kod-duzeltmesi |

### Bulgu notları

- **A2S2-03:** runtime her mesajda `declare_delivery_deadline` çağırıyor (`runtime.rs` 4132). Spekülatif kol ürün yolunda açık olduğu için ilk sıkı bütçeden sonra L1 tavanı kalıcı olarak yükselir. Düzeltme yönü: en sıkı bütçeyi uçuştaki birimlere bağlamak, birim teyit edilince ya da bırakılınca düşürmek.
- **A2S2-10:** Tek başına kod hatası değil, yapı bulgusu. Hız (BBR `pacing_gain × BtlBw`, profil tavanı, LineGuard `pace_bps`), uçuş (`window_without_recovery`, `cc_feedback_floor`) ve bant yoklaması (BBR ProbeBW, LineGuard yoklaması ve çift ölçüsü) iki kontrolcüye bölünmüş. YAMA metni "hat" kelimesini profil hattı mı ölçülen hat mı diye ayırmıyor. Sonraki kararlar tavanı LineGuard'ın yönettiğini varsayıyor; çözüm sahip kararı ister.
- **A2S2-18 ve A2S2-19:** PTT ve metin davranışı sahip kararı 3 (7171705736) ile donduruldu. Bulgular kayda geçti, kod önerilmez.
- Bu aralıkta `receiver.rs` alarm aralığı ve #262 yoklama yolu yok (7186813951, Web'e verildi; #262 dalı parkta).

## Okunan otorite bölümleri

- CLAUDE.md §4 (INV-RATE-1 2026-11-11 metni + 2026-11-16 profil istisnası, INV-FEC-1, INV-ENGINE-SOC), §5, §6, §8, §9, YAMA KAYDI.
- Sahip kararları (#175): 7171705736 (dokuz karar; karar 3 PTT/metin dondurma), 7172579876 (K2′), 7174289733 (#262 ACK sayımı/bütçe), 7174845328, 7175487850, 7175498704, 7175888074, 7186813951.
- `docs/decisions/KARAR-KAYDI-2026-11.md`, `SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md`, `PoC-KAPSAM-001.md`, `DR-CONNECT-1.md` (terim taraması).
- `specs/adaptive-transport-engine-spec.md`: §3 (250–328), §3.3 A–F (329–430), §5 + §5.1 (462–557), §8.1–8.5 (665–728), §9–§9.4 (732–763), §10–§10.2 (767–818), §11 + AMENDMENT-3 (822–889), §12–§15 (944–1030), §16.1 (1066–1114), §16.2 (1116–1194).
- `specs/transport-layer-tdd.md`: §5.1–§5.6 (596–676), §6 (679–695), §7 (699–719), başlık YAMA kayıtları (AMENDMENT-8/9, DR-CONNECT-1).
- `specs/fec-core-tdd-v3.0.md`: §0.8 (50–104), §8–§10 (266–327).
- `docs/transport-v1/adr/ADR-minimal-parity-exception.md` (1–60).
- `specs/identity-and-trust-model.md` §28.1-A satırı (DR-EST-1 "SINIRSIZ").
