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
**Reaktif FEC ile ARQ aynı delik için yarışıyor; reorder altında yarış kaybediliyor ve parity yine de bütçe tüketiyor.**
Ölçüldü (60 koşu, n=5, `c4-attrib-n5.analysis.txt`): C4'te alınan repair'lerin yalnız **%9–14'ü faydalı** (C1 loss-only:
%43–57); `wasted_complete` = 0 her yerde (parity blok tamamlanırken elde değil), evict payı benzer (%17–24) → parity
blok **zaten kapandıktan sonra** varıyor (hayalet giriş; kalan kova C4 %67–70, C1 %26–38): retransmit ya da geç
orijinal deliği önce dolduruyor. Gönderici tarafında aynı parity, retransmit'i geciktiriyor: C4 ON'da retransmit
seçimi 3× (2 344 vs 759), token reddi 0.92–0.95 (OFF 0.87); parity kuyruğu doluyken 50 ms başına retransmit reddi
44–57 (boşken 0.9–4.6), token bakiyesi 75–97 vs 470–1 600 — bu mekanizma C1'de de var, ama C4'te zamanın
**%53–76**'sında sürüyor (C1 %10–26) ve orada faydası ≈0. Net: tel +13/+23 %, ≤2×RTT teslim −4…−9 pp (n=5).
Q3: retransmitler çoğunlukla gerçek (spurious 19–22 / ~130). Q4: parity kaynağı SpecPers (L2) ağırlıklı; A'da
uzun/stall koşularda StructGate; karar büyüklüğü C1 ile aynı (7–11 kB) → lane aşırı ateşlemiyor, parity C4'te
ödemiyor. Q5: teslim gerilemesi parity-emisyon → token baskısı → geciken retransmit zinciriyle zamansal olarak
örtüşüyor. Evict/cap (32) ana mekanizma DEĞİL (öngörü P2 ✓); öngörü P1'in kova tanımı hatalıydı (tamamlanma
ANINDA tutulan repair yerine tamamlanma SONRASI gelen repair — hayalet) — düzeltilmiş okuma yukarıda.

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

**Sonuç (n=5, 60 koşu; `c4-attrib-n5.analysis.txt`, koşular `runs-c4-attrib-n5.tar.gz`):**

| hücre | kol | d2_ok (off) | tel % | parity B | repairs rx → useful / evicted / kalan(hayalet) | rec/rx | retx sel / den (off) |
|---|---|---|---|---|---|---|---|
| E1-L0 | A / B | 0.979 (0.979) | 0 | 0 | 0 | — | 0 / 0 |
| E1-L20 | A | 0.531 (0.281) | +13.8 | 8 464 | 74 → 42 / 13 / 19 | 0.45 | 673 / 552 (612 / 493) |
| E1-L20 | B | 0.562 | +23.0 | 12 328 | 115 → 49 / 22 / 44 | 0.30 | 876 / 740 |
| E2-C1b | A / B | 0.833 / 0.823 (0.781) | +2.4 / +1.4 | 736 / 644 | 8 → 0 / 0 / 8 | 0 | 228 / 219 (207 / 198) |
| E3-C4 | A | **0.240** (0.333) | +13.1 | 7 176 | 66 → **9** / 11 / 46 | **0.12** | **2 344 / 2 220** (759 / 660) |
| E3-C4 | B | **0.292** | +23.0 | 11 040 | 91 → **8** / 22 / 61 | **0.09** | 2 084 / 1 917 |

Zamansal (ts, 50 ms): parity kuyruğu doluyken Δretx_denied 44–57 (boşken 0.9–4.6), token 75–97 vs 470–1 600; kuyruk dolu
payı C4 %53–76, C1 %10–26. **Erratum:** öngörü P1'in kovası ("tamamlanma anında tutulan repair") sıfır çıktı; asıl kova
"tamamlanma SONRASI gelen repair" (hayalet giriş) — hesaba kalan olarak girdi (repairs_rx − useful − evicted). Vakum
sayaç: `SchedDiag.fec_parity_{token_denied, wire_emitted}` parity yolunda yazılmıyor (0); parity emisyonu
`fec_repairs_tx`/parity byte'ından okundu. İnvaryant `t_risk == t_susp`: tek ihlal stall'da.

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

**ÖLÇÜM (koşudan SONRA; `g1-falsification.analysis.txt`, `g1-stats.txt`, `runs-g1-falsification.tar.gz`; 200 koşu, rc≠0: 0):
tahmin YANLIŞ çıktı — G1 kabul kriterlerini SAĞLIYOR.** C4 n=20 (stall-dışı ortalama, sd, permütasyon p tek-yönlü
"kol > OFF"): OFF 0.262 (sd 0.076, n=17) · A-M0 **0.183** (Δ −0.078, p=0.997 → gerileme anlamlı) · **A-G1 0.301**
(Δ +0.039, p=0.139) · B-M0 0.262 (Δ +0.001) · **B-G1 0.320** (Δ +0.058, p=0.030). Tüm-koşu: OFF 0.222 · A-M0 0.138 ·
A-G1 0.256 · B-M0 0.223 · B-G1 0.256. Parity 7.9/10.8 kB → **4.2/5.9 kB**; tel +15.4/+20.8 % → **+6.4/+10.4 %**; geçen
parity'nin faydası 0.10 → **0.23–0.25** (repairs_rx 36/50 → useful 10/14). C1 n=10: A-G1 0.507 ≡ A-M0 0.508, B-G1 0.610 ≡
B-M0 0.613 (rp ≡ 0 → guard yapısal olarak inert; medyan A-G1 0.490 vs 0.526 farkı n=8–9 gürültüsü, kriter marjı 0.03'ü
0.006 aşar — tel/parity/repair birebir aynı); OFF 0.273 → loss-only fayda **+23/+34 pp** korunur (p ≤ 0.001). E2-C1b:
parity değişmedi (ilk L1 spurious deklarasyonundan önce; ≤ %1.8 tel). E1-L0: parity 0, d2 özdeş. İnvaryant
`t_risk == t_susp`: 0 ihlal (stall dâhil). Cold-start: `t_L1 − t_susp` medyanı C4'te 0 ms her ON kolda.
**Sapma dekompozisyonu:** offline tahmin parity faydasını akış boyunca düzgün ≈0.1 varsaydı; ölçüm israfın erken
yüksek-rp fazında yoğunlaştığını gösterdi (reorder deliklerinin parity'si %100 boşa; gerçek-kayıp fazının parity'si
%23–45 faydalı) — model varsayımı hatası, enstrüman değil.

## 8. Temiz / loss-only / reorder-only / C4 sonuçları (G1 ailesi; stall-dışı medyan; ≤2×RTT teslim)

| hücre | n | OFF | A (M0) | **A+G1** | B (M0) | **B+G1** |
|---|---|---|---|---|---|---|
| C0 temiz (E1-L0) | 5 | 0.979 | 0.979 | 0.979 | 0.979 | 0.979 — parity 0 hepsinde |
| C1 loss-only 20 % (E1-L20) | 10 | 0.250 | 0.526 | 0.490 (≡ M0, rp=0) | 0.594 | 0.615 |
| C2 reorder-only 30 % (E2-C1b) | 5 | 0.854 | 0.823 | 0.802 | 0.792 | 0.833 — parity ≤ 736 B, retx-gürültüsü |
| **C4 karışık 20 %+30 %** | **20** | **0.281** | 0.188 | **0.333** | 0.302 | **0.318** |

## 9. all-runs vs stall-free (C4 n=20 · C1 n=10)
C4 stall 3/5/3/3/4 (OFF/A/A+G1/B/B+G1). Ortalamalar stall-dışı → tüm-koşu: OFF 0.262 → 0.222 · A 0.183 → 0.138 ·
A+G1 0.301 → 0.256 · B 0.262 → 0.223 · B+G1 0.320 → 0.256. Permütasyon (kol > OFF, tek-yönlü): A p=0.997 (gerileme
anlamlı), A+G1 p=0.139, B p=0.496, B+G1 p=0.030; tüm-koşu: A+G1 p=0.227, B+G1 p=0.228. C1: stall 5/2/1/3/1; OFF'un
stall-dışı n=5. Stall politikadan bağımsız; koşular silinmedi (`runs-g1-falsification.tar.gz`).

## 10. Tel / parity / retx etkileri (C4, stall-dışı medyan)
| kol | tel B | Δtel | parity B | retx B | repairs rx → useful |
|---|---|---|---|---|---|
| OFF | 54 233 | — | 0 | 11 213 | — |
| A | 62 191 | +15.4 % | 7 912 | 11 245 | 71 → 9 (0.10) |
| **A+G1** | 57 326 | **+6.4 %** | 4 232 | 10 318 | 36 → 10 (0.25) |
| B | 65 074 | +20.8 % | 10 764 | 11 539 | 93 → 9 (0.10) |
| **B+G1** | 59 494 | **+10.4 %** | 5 888 | 11 420 | 50 → 14 (0.23) |
C1: A+G1 tel +15 % (≡ A), B+G1 +22 % (≡ B) — loss-only bedeli değişmedi. "Büyük pozitif tel + negatif teslim" C4'te
artık normal sonuç DEĞİL: +6/+10 % tel, teslim ≥ OFF.

## 11. Cold-start invaryantı
`t_first_risk == t_first_suspected`: G1 ailesi 200 koşuda **0 ihlal** (stall dâhil); atıf ailesi 60 koşuda 1 ihlal,
stall'da (srtt=0 kuralı). `t_L1 − t_susp` medyanı C4'te her ON kolda 0 ms (n=15–17): G1 ilk L1'i geciktirmez
(rp ilk spurious deklarasyonuna kadar 0). Birim testi `g1_reorder_baskisi_oneriyi_bastirir_riski_degistirmez`:
tahminci/t_first_risk/ilk-L1 özdeş, yalnız öneri bastırılır; rp = 0 iken byte-özdeş.

## 12. Handoff invaryantı
Handoff klampı (`canonical_prop > 0`) dokunulmadı; G1 `speculative_propose`'da klamptan ÖNCE yalnız reorder-belirsiz
rejimde öneriyi sıfırlar. `handoff_devralan_yoksa_spekulatif_dusmez` testi yeşil (fec_loss_risk 24/24).

## 13. A/B kararı
B, C4'te A ile eş (0.320 vs 0.301, gürültü içinde) ve loss-only'de +11 pp (0.61 vs 0.51) ama +7 % daha çok tel →
Pareto-üstün DEĞİL. Kural gereği ekonomik aday **A = 250-250-250** v1 politikası; **B = 250-375-375** yüksek-agresiflik
alternatifi olarak kalibrasyon borcu (kayıp ≥ 15 %'te +9…+15 pp, tel +7–8 %).

## 14. FEC v1 KARARI
_(final)_
