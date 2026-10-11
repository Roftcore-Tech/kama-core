# WAIVER — geçmişte karşılanmamış ölçüt geriye dönük geçmiş sayılmaz; PLAN-4GUN ONAYLANDI yalnız bu kaydın yazılmasını onaylar.

**G-IMPL-S1-WAIVER-001** — G-IMPL-S1 ("Application IMPLEMENTATION-GO") kapısının geriye dönük kaydı.

| Alan | Değer |
|---|---|
| Kayıt | G-IMPL-S1-WAIVER-001 |
| Kapı | G-IMPL-S1 — Application IMPLEMENTATION-GO. BL `docs/sprint_plan/SPRINT-BACKLOG-FREEZE.md:420-421`: "Aynı vektörler + aynı conformance-core; Crypto/hub-RPC bağımlılığı sıfır (statik)." BL:514: "**IMPLEMENTATION-GO** = gerçek kod aynı vektörleri tüketiyor + aynı conformance-core'u geçiyor + sözleşme dışı davranış icat etmiyor." Sıra (BL:506): "G-S1 → B2.* → G-IMPL-S1". |
| Dayanak | `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` v1.1: D16 (v1.1), KART K4G-04 NE 3 |
| Yazan | Claude Code, şerit L-GATE (K4G-04), 2026-10-11 |
| Onayın kapsamı | "PLAN-4GUN ONAYLANDI" (Abkarada, 2026-10-11) yalnız bu kaydın yazılmasını onaylar. (ii)'deki dört ölçütün hiçbiri geriye dönük olarak karşılanmış sayılmaz (D16). |
| BL | Bu belge BL'ye dokunmaz. BL'deki G-IMPL-S1 kapı satırı K4G-16 (NE 1) ile güncellenecek. |
| Satır atıfları | main `b58717b7`. Atıf yapılan dosyalar `0156e941`'de değişmedi. "O gün" sayıları `47c17534`'e göredir. |

## (i) Tarihsel olay

| Alan | Değer |
|---|---|
| Ne zaman | 2026-10-08, yaklaşık 03:40 (+03). Saat orkestratör kaydındandır ve yaklaşıktır. Doğrulanabilir alt sınır: ilan PR #109'un merge'ünden (2026-10-08T00:20:03Z = 03:20:03 +03) sonradır. |
| Hangi commit | `47c17534`: [PR #109](https://github.com/Roftcore-Tech/kama-core/pull/109) "L-T1: S1 KamaTransport gerçek adaptörü — ClientTransport + S1 conformance real hedefi (11/12 iki hedefte, H-PROGRESS beyanlı boşluk)" (squash merge). İlan bu merge'e bağlıdır. Not: PR #110 (L-B2d + S8, `6be4ff44`) 03:27:38 (+03)'te merge oldu. İlan saati yaklaşık olduğundan ilan anındaki main ucu `47c17534` ya da `6be4ff44` olabilir. |
| Kim | Orkestratör (Claude Code). Kapı, iş emri 2026-10-06b ile orkestratöre **ön-yetkili** verilmişti. #109 gövdesi: "**Kapı:** G-IMPL-S1'in son parçası (G-IMPL-S1 = L-T1 + L-A5c + L-B2a; ön-yetkili; kabul: S1 conformance `--target real` gerçek kriptoyla yeşil, layer_check C6 dahil yeşil, kama-app'te kripto sembolü yok — blake2 DAR istisnası sahip onaylı)." Orkestratör G-IMPL-S1'i geçildi diye ilan etti. |
| Bileşenler | L-T1 = #109 (`47c17534`); L-A5c = #103 (`b6790269`) + #104 (`5272da20`); L-B2a = #98 (`6f074328`, AMP codec, zstd'siz) + #105 (`f2b99b57`, TEXT + RECEIPT) |
| B2.* (BL sırası) o gün | B2.1 KISMİ (zstd; (ii) 2) · B2.2 #105 · B2.3 #108 (`c10fcc4b`; #108 gövdesi: "G-IMPL-S1'in parçası değil") · B2.4 #107 (`eefa3b5b`) · B2.5: Transport tarafı #109, Application tarafı #110 (`6be4ff44`, #109'dan sonra merge) |
| Nerede | Orkestratörün Abkarada'ya verdiği pencere raporunda. Repoda yazılı kayıt bırakmadı: BL'deki G-IMPL-S1 satırı (BL:420-421) durum taşımaz; #109 gövdesi yalnız "Merge sonrası G-IMPL-S1 ilan edilir." der. Bunu 2026-10-11'de `git grep` ile yapılan repo taraması doğruladı. |
| O gün kullanılan kabul ölçütü | #109 başlığındaki ön-yetki kabulü (yukarıda). BL tanımı daha geniştir (bu belgenin başındaki "Kapı" satırı). |
| O gün sunulan kanıt | #109 yerel kapısı: `seam-conformance.sh --target real` YEŞİL: real-adapter 4/4; conformance-core real 17/17; §6 @ real 11/12 koştu, H-PROGRESS = GAP-S1-REAL-PROGRESS. `--target mock` YEŞİL: §6'da 11/14 iki hedefte, 1/14 yalnız sahtede, 2/14 statik. `seam-conformance-selftest.sh` 80/0 · `layer_check.py` YEŞİL (C1–C6) · `client-core` 77/0. Adaptör mutasyonları M1–M5'in her biri yalnız beklenen satırı kırmızı yaptı; mutasyonsuz real süiti 5/5 yeşil (17/17). Hosted CI 12/12 kontrol SUCCESS: core-ci [37706537279](https://github.com/Roftcore-Tech/kama-core/actions/runs/37706537279), spec-word-coverage [37706537312](https://github.com/Roftcore-Tech/kama-core/actions/runs/37706537312). |

## (ii) Karşılanmayan ölçütler

O günkü durum `47c17534` içindir.

| # | Ölçüt (kaynak) | Karşılanmayan | Kanıt |
|---|---|---|---|
| 1 | "aynı conformance-core" (BL:421; S1 §6) | **H-PROGRESS real hedefte yok.** Real hedefte 12 davranış satırının 11'i koştu. H-PROGRESS yalnız sahtede koşar (`mock_only`); real'de beyanlı boşluktur (`GAP-S1-REAL-PROGRESS`). Ürün Transport'u yalnız `Measuring` üretir; `Suspended` / `LongOutage` tetikleri ve hub prob'u yok. | #109 gövdesi; BL:391; `scripts/ci/seam-coverage.json:94-99`; PoC-KAPSAM-001 §5 ("`H-PROGRESS` gerçek hedefte yalnız `Measuring`") |
| 2 | "aynı vektörler" (BL:421) | **zstd pending.** AMP kayıt vektörlerinden sıkıştırılmış kayıt taşıyanlar `PENDING_ZSTD` listesindedir. B2.1 kabul sayımına girmezler, testte ayrıca koşulurlar. Liste `47c17534`'te 7 vaka; bugün 8 (8. vaka #110 / S8 ile geldi). B2.1 DURUM: "KISMİ". | BL:399; BL:540 (KATMAN 6 madde 8); `crates/kama-app/tests/amp_vectors.rs:44-54`; `git show 47c17534:crates/kama-app/tests/amp_vectors.rs` → `PENDING_ZSTD: [&str; 7]` (#98 merge'ü `6f074328`'de de 7) |
| 3 | "Crypto … bağımlılığı sıfır (statik)" (BL:421) | **blake2 dar istisnası.** `kama-app` → `blake2` 0.10 normal bağımlılığı vardır (`FILE_BEGIN.whole_file_hash`, BLAKE2s-256, AMP §3 0x02). `kama-noise-wire`'a kenar yoktur (layer_check C1–C6 yeşil, #109); ama ham kripto primitifi kenarı sıfır değildir. | #108 gövdesi ("blake2 DAR istisnası (Abkarada 2026-10-08, literal: 'yalnız FILE_BEGIN.whole_file_hash; layer_check L1'de adıyla')"); `scripts/ci/layer_check.py:77-82` (`APP_RAW_CRYPTO_EXCEPTION = {"blake2": "normal"}`); `crates/kama-app/Cargo.toml:21-25` |
| 4 | IMPLEMENTATION-GO "gerçek kod … aynı conformance-core'u geçiyor" (BL:514), Application için | **S1 real hedefi kama-app'i çalıştırmıyor.** Real hedef `ClientTransport`'tur (gerçek `NoiseWireProtection`, TEST anahtarları, UDP loopback). Karşı uç aynı adaptörün echo yapan örnekleridir. `kama-app` (Messenger / FileTransfer / Ptt) yalnız sahte Transport'la sınandı. | #109 gövdesi ("S1 conformance — real hedef"); BL:385; plan §1.1 (`6be4ff44`): "86 test (37 birim + 49 entegrasyon), **hepsi sahte transport'la**"; plan §1.3 |

## (iii) Waiver

| # | Gerekçe | Kapanış kartı / KATMAN 6 satırı |
|---|---|---|
| 1 H-PROGRESS | `Suspended` / `LongOutage`'ın gerçek tetikleri (link ve güç sinyali) ve hub prob'u bu sprintte KAPSAM DIŞI (iş emri 2026-10-06b §4; PARK P-4). Direct-mode'da hub yoktur. PoC'de beyanlı açıktır (PoC-KAPSAM-001 §5: "`Suspended/LongOutage` tetikleri hub sprintinde"). | BL KATMAN 6 madde 1 (BL:525): "ConnectPhase::Suspended/LongOutage'ın gerçek tetikleri … Sonraki sprint 'DR-CONNECT-1 IMPL'". Hub sprinti: plan D18 ve K4G-16 NE 1'in KATMAN 6 eki "heartbeat uygulaması + eşik/TTL kararları (D18)". |
| 2 zstd | Abkarada kararı (2026-10-08): ruzstd REDDEDİLDİ; bu sprint `compression_id = NONE`. Gönderici sıkıştırmaz; alıcı sıkıştırılmış kaydı tipli `zstd-decoder-unavailable` ile düşürür (BL:540). Plan D14; PoC-KAPSAM-001 §3 ve §5 ("zstd yok"). | BL KATMAN 6 madde 8 (BL:540); K4G-16 NE 1 "zstd çözücü kararı (D14)"; plan §5 (Hayri: "zstd çözücü kararı sonrası sıkıştırma") |
| 3 blake2 | Bütün-dosya özeti Application'ın işidir: BL B2.3 (BL:406), "Application yalnız: `transfer_id`, offset, bütün-dosya hash'i, birleştirme". `whole_file_hash` (BLAKE2s-256) bu yüzden kama-app'te hesaplanır. Abkarada 2026-10-08'de dar istisnayı onayladı. İstisna `layer_check.py` L1'de adıyla ve yalnız normal bağımlılık türüyle sınırlıdır; genişlemesi kapıda kırmızı verir (`--selftest` 5 vaka, #108). | **KATMAN 6 satırı ve kapanış kartı YOK.** İstisna sahibin 2026-10-08 onayıyla yürürlüktedir. Bu kayıt yalnız ölçütün harfiyen karşılanmadığını tescil eder. |
| 4 S1 real kama-app | O gün kama-app'i gerçek anahtar, saat, entropi ve UDP ile çalıştıran ürün yolu (bileşim kökü) yoktu (plan §1.2 A13). Application'ın gerçek Transport ve gerçek Crypto ile ilk uçtan uca koşusu INT-02'ye bırakılmıştı (BL:508: "G-IMPL-S1 + G-IMPL-S3 + G-D → INT-02: uçtan uca mesaj"). | Bu sprintte (CORE-CLOSE) kapanır; KATMAN 6 satırı değildir. K4G-11: `crates/kama-node`, `two_nodes_loopback.rs`; kama-app motorları gerçek `ClientTransport` + gerçek `NoiseWireProtection` + UDP ile koşar. K4G-05: INT-02; kama-app Messenger + gerçek `ClientTransport` + gerçek `NoiseWireProtection`, loopback UDP + netem. |

Bu kayıt (ii)'deki dört ölçütün hiçbirini karşılanmış saymaz. Kayıt yalnız iki şeyi tescil eder: orkestratörün 2026-10-08'deki ilanını ve o gün karşılanmayan ölçütleri. BL satırı K4G-16 ile güncellenecek.
