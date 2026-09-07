# FEC v1 — KAPANIŞ RAPORU

## 1. Main checkpoint SHA
_(merge sonrası doldurulur)_

## 2. Final C4 dalı SHA
_(Faz-2 sonunda)_

## 3. Evidence-politika temizliği
`f2f5e68b`: `EvidenceMode::{L2Floor, L2Scaled}`, `evidence_min_units/evidence_full_units`, `speculative_propose`
M1/M2 dalları ve `l2_cap_with_evidence`, client-core re-export, bench `--em-mode/--em-min/--em-full` +
ClientCfg/ServerCfg/ArmCfg alanları, 6 politika testi + bench bayrak testi, aile script'i M1/M2 kolları KALDIRILDI.
KALAN tanı (ürün davranışına etkisiz): `RiskSplit.sent_window_units`, `TsSample.{evidence_mass, declared_units}`,
`Sender::diag_spec_transitions()`, `FateWindowDiag.fec_spec_transitions`, bench `em_l1/em_l2` + ts.csv kolonları.
`speculative_propose` L2 tavanı yeniden `persistent_cap_permille` (c3d639f2 öncesiyle özdeş).

## 4. Tam kapı sonuçları
**Checkpoint ağacı (46291974), CI-eşdeğeri yerel (-j2):** fmt 0 · clippy `--all-features` 0 · clippy `--no-default-features` 0 ·
`test --workspace --locked` 858/0 · `test --workspace --all-features` **1 214/0** (167 binary) · `--no-default-features` 858/0 ·
test-adı bütünlüğü (rakam-duyarlı): kaybolan yalnız 7 kaldırılan evidence testi, sessizce yiten test yok.
Bootstrap-OFF: **gerçek feature-OFF izolasyon ihlali** (build-kompozisyonu): `experimental-cold-start` derlendiğinde
DR-CS-1 M2 (F2A) CS-probe merdiveni ilk DATA emisyonunda koşulsuz arm oluyordu → `bootstrap_rtt: false` belgesi
("motor probe PLANLAMAZ") `--all-features`'ta ihlal; `runtime_e2e` OFF kolu tam-workspace kapısında 3/3 kırmızı
(token zamanlamasına bağlı; izole yeşil). Düzeltme (fdfacbd2 → 46291974): `Sender::set_cs_probe_lane(on)`
(feature yoksa no-op), client-core runtime `bootstrap_rtt`'yi ona yansıtır; transport `set_bootstrap_rtt` yalnız
DR-CS-3'ü anahtarlar (kama-integration DR-CS-3 paneli legacy-ladder kontrol fixture'ı korunur — ilk deneme
fdfacbd2 bunu kırmıştı: "legacy ladder compiled but did not fire"). Test yarışı DEĞİL; ürün semantiği belgeyle
hizalandı. Server-death flake'i (`client_survives_server_death_mid_run`) ayrı tutuldu; yerel kapılarda yeşil.
**Fail-fast bulgusu:** `cargo test` ilk kırmızı binary'de durur → önceki "tam kapı" sayıları kesikti; gerçek
`--all-features` ≈ 730 test / 94 binary (kama-integration `dr_cs3_v2_channel` 1 212 s).

## 5. C4 kök nedeni
_(Faz-2A)_

## 6. Kök nedeni kanıtlayan enstrüman
**Enstrüman (salt gözlem, mevcut durum):** alıcı repair partisyonu `fec_repairs_{useful, wasted_complete, wasted_evicted}` +
blok akıbetleri (`decoded`, `complete_with_repairs`, `evicted_with_repairs`); gönderici `SchedDiag` 12'li özet
(`diag-sched-telemetry`, feature forwarding client-core/bench) → `FateWindowDiag.sched`; `TsSample` += pacer token
bakiyesi, bekleyen parity/kayıp, kümülatif sınıf sayaçları; analizör `scripts/netem/fec_c4_attrib_analyze.py`.
Hücreler: E1-L0 (temiz) · E1-L20 (loss-only) · E2-C1b (reorder-only) · E3-C4 (karışık) × {off, A, B}, 96 msg, n=5.

**Beklenti (KOŞUDAN ÖNCE):** (P1) C4'te `wasted_complete` repairs_rx'in ≥ %60'ı (C1'de ≤ %40) — ARQ/geç-orijinal
parity'yi yeniyor; (P2) `wasted_evicted` küçük (≤ %15) ve C1≈C4 — evict ana mekanizma değil; (P3) gönderici
`data_token_denied/data_selected` ve `retx_denied/retx_selected` oranları C4 ON'da C1 ON'a benzer (yer-değiştirme
maliyeti her iki hücrede aynı sınıfta; C4'ü ayıran maliyet değil fayda); (P4) `retx_emitted` kümülatifi
`parity_emitted`'dan önce yükselir (zaman serisi) ve C4'te parity'nin ilk emisyonu retransmit'ten daha geç;
(P5) E2'de repairs_rx'in ~%100'ü `wasted_complete` (kayıp yok); E1-L0'da parity 0; (P6) invaryant `t_risk == t_susp`
stall-dışı 0 ihlal. Sapma: P1 tutmaz ve `wasted_evicted` büyükse → alıcı cap (32) reorder altında bağlayıcı (sizing);
P3 tutmaz (C4'te denial oranı belirgin yüksek) → bütçe rekabeti gerçek ve arbitrasyon adayı.

Ön-atıf (mevcut veri, `ev1`/`ev3_e3sup` KP-SERVER): C4'te parity alıcıya ulaşıyor (repairs_rx 62–94)
ama kurtarma 6–8 birim (loss-only C1: 30–38; verim 0.06–0.20 vs 0.34–0.41), decode_fail 0, evict benzer, enq≈emit.

## 7. Minimal düzeltme — tek sınırlı deneme (G1), KOŞUDAN ÖNCE kayıt
**Aday (yalnız mevcut sinyal):** `speculative_propose` içinde `declared_units ≥ reorder_min_evidence_units (8)
∧ reorder_pressure_permille ≥ R` ⇒ spekülatif öneri YOK (L0). `R = 300‰` (`[CAL]`, default kapalı=u32::MAX).
Gerekçe: offline C4 verisinde parity-planlanan tick'lerin %43–55'i rp ≥ 300‰ (≥ 500‰ yalnız %17–19); rp yörüngesi
0.5 s'de 590–670‰ → 2 s'de 150–175‰; C1'de rp ≡ 0 → guard yapısal olarak inert (loss-only fayda korunur);
`t_first_risk` değişmez (risk hesaplanır, yalnız öneri bastırılır). Daha düşük R gerçek-kayıpla seyrelmiş
rp'ye biner ve "reorder varsa FEC kapalı"ya dönüşür (yasak şekil).
**Tahmin (KOŞUDAN ÖNCE):** G1 C4 parity'sini ~%40–55 azaltır, tel +13/+23 % → ~+7/+12 %; fayda ≈ 0 olduğu için
d2 OFF'un hâlâ 2–5 pp ALTINDA kalır → kabul kriteri 4 ("≥ OFF, gürültü payında") SAĞLANMAZ; C1'de A+G1 ≡ A,
B+G1 ≡ B (rp = 0); E2'de parity'nin çoğu kesilir (tel −1…−2 %). Kabul: C4 n=20'de d2(G1) ≥ d2(OFF) − 0.02 VE
C1 n=10'da d2(A+G1) ≥ d2(A) − 0.03. Beklenen sonuç: **Path B**.

## 8. Temiz / loss-only / reorder-only / C4 sonuçları
## 9. all-runs vs stall-free
## 10. Tel / parity / retx etkileri
## 11. Cold-start invaryantı
## 12. Handoff invaryantı
## 13. A/B kararı
## 14. FEC v1 KARARI
