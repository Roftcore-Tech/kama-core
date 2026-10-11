# WAIVER — geçmişte karşılanmamış ölçüt geriye dönük geçmiş sayılmaz; PLAN-4GUN ONAYLANDI yalnız bu kaydın yazılmasını onaylar.

**G-D-WAIVER-001** — G-D ("Transport geçişi tamam") kapısının geriye dönük kaydı.

| Alan | Değer |
|---|---|
| Kayıt | G-D-WAIVER-001 |
| Kapı | G-D — Transport geçişi tamam. BL `docs/sprint_plan/SPRINT-BACKLOG-FREEZE.md:487-488`: "D1 + D2 + D3. INT-02'nin ön koşulu; kripto ve application kapılarından bağımsız." |
| Dayanak | `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` v1.1: D16 (v1.1), KART K4G-04 NE 3 |
| Yazan | Claude Code, şerit L-GATE (K4G-04), 2026-10-11 |
| Onayın kapsamı | "PLAN-4GUN ONAYLANDI" (Abkarada, 2026-10-11) yalnız bu kaydın yazılmasını onaylar. (ii)'deki ölçüt geriye dönük olarak karşılanmış sayılmaz (D16). |
| BL | Bu belge BL'ye dokunmaz. BL'deki G-D kapı satırı K4G-16 (NE 1) ile güncellenecek. |
| Satır atıfları | main `b58717b7`. Atıf yapılan dosyalar `0156e941`'de değişmedi. |

## (i) Tarihsel olay

| Alan | Değer |
|---|---|
| Ne zaman | 2026-10-08, yaklaşık 03:00 (+03). Saat orkestratör kaydındandır ve yaklaşıktır. Doğrulanabilir alt sınır: ilan PR #106'nın merge'ünden (2026-10-07T23:27:27Z = 2026-10-08 02:27:27 +03) sonradır. |
| Hangi commit | `995e037e`: [PR #106](https://github.com/Roftcore-Tech/kama-core/pull/106) "L-D: transport geçişi — D1a ARQ kimliği = full_wire_pn, D2 Control::Ack/Sack silimi, D3 golden + AMENDMENT-11" (squash merge). İlan bu merge'e bağlıdır. Arada merge edilen #107 (`eefa3b5b`) ve #108 (`c10fcc4b`, 02:55:31 +03) yüzünden ilan anında main'in ucu `c10fcc4b` olabilir. İkisi de WS-D kapsamına dokunmaz: #107 yalnız `kama-app` ve BL'yi, #108 `kama-app`, `Cargo.lock` (kama-app → blake2 kenarı), BL, `scripts/ci/layer_check.py` ve `scripts/ci/seam-coverage.json`'ı değiştirir. |
| Kim | Orkestratör (Claude Code). Kapı, iş emri 2026-10-06b ile orkestratöre **ön-yetkili** verilmişti. #106 gövdesi: "**Kapı:** G-D (ön-yetkili; kabul: engine + integration testleri yeşil, golden'lar amendment'la uyumlu)". Orkestratör G-D'yi geçildi diye ilan etti. |
| Nerede | Orkestratörün Abkarada'ya verdiği pencere raporunda. Repoda yazılı kayıt bırakmadı: BL'deki G-D satırı (BL:487-488) durum taşımaz; #106 gövdesi yalnız "Merge sonrası G-D ilan edilir." der. Bunu 2026-10-11'de `git grep` ile yapılan repo taraması doğruladı. |
| O gün kullanılan kabul ölçütü | #106 başlığındaki ön-yetki kabulü: "engine + integration testleri yeşil, golden'lar amendment'la uyumlu". BL'deki kapı tanımı daha geniştir: "D1 + D2 + D3". D1'in Done'u iki parçalıdır (BL:467): "mevcut kayıp/kurtarma davranış testleri yeni kimlikle yeşil; interim çerçeve kodda yok." |
| O gün sunulan kanıt | #106 yerel kapı tablosu: `kama-transport-schema` + `kama-engine` + `adaptive-transport` 459/0 (43 ikili) · `kama-noise-wire` 273/0 · `client-core` 73/0 (`runtime_e2e` 20/20) · `kama-integration` (`control_path`, `pipeline`, `byte_accounting`) 16/0 · `noise-conformance.sh` mock / real / vectors / identity 18 / 66 / 3 / 1 · `seam-conformance.sh --target mock`, `seam-api-drift.py`, `layer_check.py`, `no_inline_expected.py`, `word_coverage.py` yeşil · fuzz `--bins` 11 hedef derlendi. Ayırt edici test `runtime_e2e::arq_identity_is_the_pn_not_the_interim_seq_field`: mutasyon A ve B 3/3 kırmızı, mutasyonsuz 5/5 yeşil. Hosted CI 12/12 kontrol SUCCESS: core-ci [37701582161](https://github.com/Roftcore-Tech/kama-core/actions/runs/37701582161), spec-word-coverage [37701582125](https://github.com/Roftcore-Tech/kama-core/actions/runs/37701582125). Spec: `specs/transport-layer-tdd.md` §12 [AMENDMENT-11] (:846-898). |
| BL DURUM satırları | D1 (BL:468): "KİMLİK KAPANDI (L-D, D1a); 'interim çerçeve kodda yok' beyanlı KARŞILANMADI (S3)." · D2 (BL:476): "KOD KAPANDI (L-D)." · D3 (BL:485): "KAPANDI (L-D)." |

## (ii) Karşılanmayan ölçütler

| Ölçüt | Kaynak | Durum | Kanıt |
|---|---|---|---|
| D1 Done'un ikinci parçası: "interim çerçeve kodda yok" | BL:467 | **KARŞILANMADI**: o gün (`995e037e`) de, bugün (`0156e941`) de. 11 B ara çerçeve veri yolunda kalır: `crates/client-core/src/wire.rs:3`, `:16-17` (`[0x00][seq u16 BE][unit_id u64 BE][payload]`, `HDR_DATA = 11`). `BatchPayload` codec'i var (`crates/client-core/src/batch.rs`, #99), ama `client-core`'da `lib.rs:41`'deki `pub mod batch;` dışında onu kullanan yer yok. Ara çerçevenin u16 `seq` alanı yerindedir; alıcı onu okumaz. | BL:468; #106 gövdesi ("**Beyanlı KARŞILANMAYAN (S3, 2026-10-08)**: 'interim çerçeve kodda yok' — 11 B çerçeve veri yolunda kalır; `BatchPayload` codec'i var (#99), geçiş sonraki sprint."); `specs/transport-layer-tdd.md:859-861` |

Aşağıdaki iki not karşılanmayan ölçüt değildir; tabloyu değiştirmez:

- **NON-GATING KVT tek netem profili koşulmadı.** Neden: `sch_netem` yüklü değildi (T5, sahip eylemi). Bu bir G-D ölçütü değildir (BL KATMAN 6 madde 5; AMENDMENT-11 beyan 5).
- **Ayırt edici testte sonradan gözlenen oynaklık.** 2026-10-11 K4G-01b kapısında (PR #116, HEAD `472c6af0`) D1'in ayırt edici testi `runtime_e2e::arq_identity_is_the_pn_not_the_interim_seq_field` yalıtılmış bir koşuda bir kez düştü ("kalıcı silinen birim teslim edilemez"). Tekrar ölçümler: dalda 10/10, main `2f705d32`'de 10/10; adım tekrarları 98/0, 98/0. Olası mekanizma (ürün profilinde FEC parity'si "silinen" birimi kurtarır) doğrulanmadı. #116 takip önerisini Abkarada kararına bırakır. Bu, o gün sunulan kanıtın (5/5 yeşil, mutasyon 3/3) sonradan görülen bir sınırıdır.

## (iii) Waiver

| Ölçüt | Gerekçe | Kapanış kartı / KATMAN 6 satırı |
|---|---|---|
| D1 "interim çerçeve kodda yok" | Abkarada'nın S3 kararı (2026-10-08): "interim çerçeve kodda yok" bu sprintte beyanlı olarak karşılanmaz; veri yolunun `BatchPayload`'a geçişi sonraki sprinte bırakılır. Repodaki izi: BL:468 "(S3)"; `specs/transport-layer-tdd.md:859-861` (AMENDMENT-11): "Ara çerçevedeki u16 `seq` alanı YERİNDE: S3 kararı (2026-10-08) gereği 'interim çerçeve kodda yok' bu sprint beyanlı KARŞILANMAZ (`BatchPayload` codec'i var, `crates/client-core/src/batch.rs`; veri yolu geçişi sonraki sprint)." Waiver'ın dayandığı ayrım da aynı amendment'tadır: ARQ/ACK kimliği `meta.full_wire_pn`'e taşındı; ara çerçevenin u16 alanı "motor seq'inin alt 16 bitini taşımaya devam eder ama alıcı OKUMAZ — kimlik değildir." | KATMAN 6 satırı "BatchPayload veri yolu geçişi". **Bu satır bugün BL KATMAN 6'da YOK** (BL:521-543). K4G-16 NE 1 ile eklenecek; plan §1.2 A16: "KATMAN 6'ya 4 erteleme yazılmadı (… BatchPayload veri yolu)". Sahibi Abkarada (plan §5: "Abkarada (kripto/transport) … BatchPayload veri yolu"). INT-02 raporunun beyanlı boşluklarına da girer (plan K4G-05 NE 4: "ara çerçeveden BatchPayload'a geçiş sonraki sprint"). |

Bu kayıt D1'in "interim çerçeve kodda yok" ölçütünü karşılanmış saymaz. Kayıt yalnız iki şeyi tescil eder: orkestratörün 2026-10-08'deki ilanını ve o gün karşılanmayan ölçütü. BL satırı K4G-16 ile güncellenecek.
