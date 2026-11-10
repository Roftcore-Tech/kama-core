# E1-HIZ — E1 gösterim bayrakları (tasarım notu, 1 sayfa)

Etiket: *gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel.* Dayanak: CC §2.1, §4.1, §4.4
(CC-K5, CC-K6 imzalı); sahip kararı v4 I.3; ISEMRI-YOL3-M2 KK-M2-01..08. Bayraklar çalışma zamanı
yapılandırmasıdır (`RuntimeConfig.e1_flags` → `Sender::set_e1_flags`), varsayılan beşi kapalı; kapalıyken
motor önceki kod yolunu aynen çağırır (karar dizisi `origin/main` ile bit-aynı: `tests/e1_goster.rs`).

| bayrak | kural | yer (`sender.rs` P bölgesi, `rate.rs`) |
|---|---|---|
| `e1_startup_cwnd` (c) | Faz `PacingPhase::Startup` iken (Startup ve Startup tabanlı RecoverArq/RecoverFwd overlay'i) cwnd kazancı 2885/1000; hız, `cwnd/srtt` tavanı ve kalkış tabanı tek yardımcıdan (`e1_startup_cwnd_gain`). Drain hedefi `CWND_GAIN × BDP`, MIN_CWND değişmez. | `compute_send_rate`, `cwnd_over_srtt`; `rate::effective_send_rate_cwnd_gain`, `rate::cwnd_with_gain` (eklemeli) |
| `e1_applimited_plateau` (d) | Ayrı tur mandalı: `schedule_transmit` başında `tx_queue` boş ∧ `pacer.tokens() == pacer.burst_cap()` → mandal; tur işaretinde tüketilir; işaretli tur `startup_stable_rounds`/`startup_last_btlbw`'ye dokunmadan `false`. **qd freni GİRER** (G1 model kararı): tur işaretinde qd ≥ 250‰ ise tur yine değerlendirilir. `track_sent` örnekleyicisine dokunulmaz. | `startup_plateau`, `e1_mark_app_limited_round` |
| `e1_drainrt_gate` | DrainRtprop girişi: rtprop bayat ∧ qd ≥ 250‰. | `on_timer(RtpropProbe)` |
| `e1_probe_keep` (e-dar) | Bekleyen kayıp ProbeUp'ı kesmez; proba atfedilen kayıp iptal ettirmez (`probe_loss_attributed` gözlem olarak kurulur); qd iptali ve acil durum yolu kalır; retx koşulu `!pending_lost.is_empty() ∧ state ≠ RecoverFwd`. | `update_state`, `schedule_transmit` iki retx koşulu |
| `retx_first` | ARQ-önce koşulu `retx_first ∨ tick % K_ARQ_SHARE == 0`. Kapalıyken TUR-7 β2 mührü (DATA-önce, her 4. izinde ARQ-önce) ve parite sırası aynen. | `schedule_transmit` |

**Değişmezler.** Hiçbir bayrak kaybı hız girdisi yapmaz (INV-RATE-1; "kayıp > p̂ ise plato" alınmadı);
`diag_*` karar girdisi olmaz (INV-007; `pacer.tokens()`/`burst_cap()` kovanın gerçek durumudur); SR1
freni ve gain-cycling reddi aynen; RNG yok (ADR-17); tel biçimi değişmez.

**G2 seçimi (2026-11-09 teşhisi; sahip v4 EK-A §2 kabulü).** Gösterim seti = **K1** = (c) + (d) + DrainRtprop
+ (e-dar); `retx_first` kapalı. Gösterim analoğunda PTT p95 medyanı K1 8,63 s ↔ K0 15,29 s, dosya bitişi
19,2 ↔ 23,9 s; manşet R1 (tuzaksız) K2 1,02, K2−c 0,56 [ölçüldü; `reports/yol3/2026-11-09/g2/OKUMA.md`].
Model beklentisi (K2−c) tutmadı. R3 koruması K1'de ayrıca koşar (EK-A §2.1); emisyon ayrıştırması, tamamlanma
ve tuzaklı koşular `reports/yol3/2026-11-09/g2/EK-A-EKSIKLER.md`. N = 10, betimsel.

**qd freni kararı (KK-M2-08).** GİRER — S1 Δ 0,000 (K2 0,878/0,878; K2−c 0,443/0,443), S3 PTT p95
1,373/1,373 (K2), 1,297/1,297 (K2−c) [tahmin; N = 30, medyan; frenli/frensiz]. S3'te (d) hiç tur
işaretlemedi; fren orada etkisiz [tahmin].

**Beyanlı sınırlar.** `INVARIANTS.md` "Beyanlı sınırlar (gösterim, 2026-11)" (CC §4.4 üç satır). Ek
beyan: engine:962 (retx önce) ↔ :1064 (TUR-7 β2, DATA önce) spec çelişkisi; `retx_first` açıkken kod
:962 yönündedir, kapalıyken :1064 mührü geçerlidir (spec gösterimde değişmez).

**Aralık'a devir (CC-2).** (1−p) kapalı döngü kanalı, (a), O3–O8, MIN_CWND, kalkış izni, (e) tam overlay,
Drain hedefi 1×BDP, varsayılan açılış ve golden supersession'ları (sahip, 8 Ara) bu notun dışındadır.
