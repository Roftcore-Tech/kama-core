# K4G-05b-R — Kayıt-unutma envanteri: aynı geç ACK'te motor ve runtime

Tarih 2026-11-11. Dal `k4g/05b-r-runtime-sahiplik`. Taban: main `4a9e97f1` (CC-PORT #170 dahil).
Kaynak: #140 (`49864510`) içindeki aynı adlı envanterin bu PR'a uyarlanmış hâli.

**Yetki.** Sahip kararı 2026-11-11: "#140 (K4G-05b) BEKLER. CC-PORT motorun retx/token/HOL
politikasını değiştirdi; #140'ın motor kısmı Aralık'ta KARAR-KAYDI ile kapanır. İçindeki runtime
payload sahipliği (client-core) kısmını ayrı küçük PR olarak çıkar." Bu karar henüz
`docs/decisions/` altında kayıtlı DEĞİLDİR (KARAR-KAYDI'na girmesi bekleniyor).

**Kapsam.** #140'ın motor kısmı (birim sahiplik dizini `UnitOwnership`, teyit kümesi, madde 1–4:
parite kaydı sahibi, teyitli birime karar yok, RTO_MAX tavanı, sahiplenme adayları, K_MAP) bu PR'da
YOKTUR. Motor yalnız KARAR-NÖTR bir teyit çıktısı verir; runtime payload'ı ve app teyidini yalnız
o çıktıdan alır.

Satır atıfları: "taban" = `4a9e97f1`, "dal" = bu dalın son commit'i (`git show <sha>:<yol>`).

## Motorun teyit çıktısı (bu PR)

`Sender::take_confirmed_units() -> Vec<u64>` (dal `sender.rs` 5762) son çağrıdan beri teyit edilen
birim kimliklerini (`OpaqueUnit::id`; seq ya da ESI değil) üretim sırasıyla verir. Tampon
`CONFIRMED_UNITS_CAP = 4096` ile sınırlıdır (dal 1351; en eski düşer,
`diag_confirmed_units_overflow` artar). Olay içinde tekildir (`emit_unit_confirmations`, dal 5692);
olaylar arası tekilleştirme yapmaz.

| Üreten yol | Atıf (taban → dal) |
|---|---|
| Uçuştaki kaydın ACK'i (köken `FecParity` değil) | `on_ack_received` uçuş dalı taban 3275 → dal 3307-3309 |
| Tombstone kaydının geç ACK'i (kol-1/kol-2; köken `FecParity` değil) | taban 3299 → dal 3336-3338 |
| Kayıp ilan edilip yeniden planı bekleyen kaydın un-loss eşleşmesi | taban 3365 → dal 3406-3408 |
| `RecoveredControl`: listelenen her birim (motor kaydı süzgeci yok) | `on_recovered_control` taban 5589 → dal 5683 |

Üretmeyen: parite ACK'i (uçuşta ya da tombstone'da); canlı kaydı olmayan seq'in ACK'i (TTL'i dolup
budanmış ya da tahliye edilmiş tombstone, kopya kökenli kayıp, kuyruk kurtarmasının tombstone'suz
emekliye ayırdığı kopya kaydı, bilinmeyen seq); kaydı çoktan düşmüş seq'in yinelenen ACK'i;
zamanlayıcılar.

**Karar-nötrlük.** Yeni kod yalnız iki çıktı alanına yazar (`confirmed_units_out`,
`confirmed_units_overflow`); hiçbir karar yolu onları okumaz. `acked_units` (kopya iptali) bayt
düzeyinde değişmedi. Pinler: `sender.rs` `tests::k4g05br_teyit` (aşağıda) ve değişmeden yeşil kalan
`tv_*`, `sim_determinism`, `cc_port_sim`, `sender_diag_contract`, `e1_goster`, `e2a_goster`,
kama-engine `n8_v2`, client-core `ctx_first_dispatch`.

## Runtime (bu PR)

`crates/client-core/src/runtime.rs`, `client_task` (dal satırları):
- `drain_confirmed!` (2111): her `session.on_event` dönüşünde `take_confirmed_units()` drenajı.
  `store.remove`, `unit_acked!`, `outstanding` düşümü, `stats.acked` /
  `stats.fec_recovered_ctrl_units` (`ConfirmVia`, 1851) ve `unit_seqs`/`seq_to_unit` budaması YALNIZ
  burada. Store'da olmayan birimin teyidi sessizce atlanır (tekilleştirme = store varlığı; birim
  kimlikleri yeniden kullanılmaz).
- `drive!` (2134): drenaj kararlar işlenmeden ÖNCE; karar döngüsü store'da olmayan (teyitli) birime
  harita kaydı AÇMAZ (2155).
- ACK yolundaki seq → birim → `store.remove` döngüsü (taban 2303-2315) ve RECOVERED yolundaki doğrudan
  silme (taban 2244-2250) kalktı.
- Teşhis: `SenderRecoveryDiag::{seq_to_unit_len, unit_seqs_len, confirmed_units_overflow}` (salt
  gözlem, yalnız `live_diag` açıkken).

## Envanter

Sütunlar: yol · motor atfı (taban) · v1'de bugün etkin mi · geç ACK → motor çıktısı · geç ACK →
runtime (taban) · geç ACK → runtime (bu PR) · test.

| # | Yol | Motor atfı (taban) | v1'de etkin mi | Geç ACK → motor çıktısı | Runtime (taban `4a9e97f1`) | Runtime (bu PR) | Test |
|---|---|---|---|---|---|---|---|
| 1 | **Tombstone TTL** (K_TOMB × T_loss; TTL ≥ ≈ 25.2 s) | Pto budaması 3523-3540; K_TOMB 1513 | Evet (kesinti > TTL sınıfı; erişilebilirlik akıl yürütmesi, ölçülmedi) | Kayıt yok → teyit YOK | `seq_to_unit` seq'i tutar → `store.remove` + `acked()`; sonraki iletimler başlık-only | Payload yerinde, olay yok; sonraki iletim gövdeli; onun ACK'i teyit eder | `late_ack_after_ttl_keeps_payload` |
| 2 | **TOMBSTONE_MAX tahliyesi** (4096, en eski önce) | `queue_losses` 3790-3799, `retire_inflight_to_tombstone` 6257-6266; sabit 1554 | Kod yolu var; bir TTL içinde ≥ 4096 tombstone girişi gerekir | Satır 1 ile aynı | Satır 1 ile aynı | Satır 1 ile aynı | `late_ack_after_cap_keeps_payload` |
| 3 | **Kopya kökenli kayıp** (tombstone bırakmaz, `pending_lost`'a girmez) | `queue_losses` 3726-3746 | Evet (PRE_CONVERGE kopyaları, RecoverFwd, ARQ tavanı sonrası devir) | Kayıt yok → teyit YOK | `store.remove` + `acked()` | Satır 1 ile aynı | `late_ack_after_dup_loss_keeps_payload` |
| 3b | **Kopya kaydının kuyruk kurtarmasıyla emekliliği** (tombstone yok) | `retire_inflight_to_tombstone` 6249-6253 | Evet (tail-recovery ürün varsayılanı) | Satır 3 ile aynı | Satır 3 ile aynı | Satır 3 ile aynı | Ayrı test yok (aynı "kayıt yok → teyit yok" kolu; motor testi (c) kolu kayıp ilanıyla sınar) |
| 4 | K_MAP budaması | Tabanda ve bu PR'da YOK | — | — | — | — | **Yalnız #140** |
| 5 | **FEC parite ACK'i — ESI ad çakışması** | Parite kaydı `unit.id = esi`; uçuş dalı 3284 `acked_units.push(esi)` → 3443-3445 esi kimlikli KAYNAK birimin kopyaları iptal | Evet (FEC ürün profilinde açık) | Teyit YOK (`FecParity` süzgeci) | Parite seq'i haritaya girmez (`carries_unit` 2021-2025) → payload kalır | Aynı; teyit çıktısı süzgeci olmasaydı runtime 0..R birimlerini silerdi | `parity_ack_no_confirmation_payload_kept` (faz 1); motor `k4g05br_parity_ack_never_confirms_esi_namesake`. Kopya iptali (ters yön) bu PR'da AÇIK kalır — **#140 motor kısmı** |
| 6 | Parite kaydının süre dolumu | Tabanda ve bu PR'da YOK | — | — | — | — | **Yalnız #140** |
| 7 | **Teyit sonrası kalan kayıt** (ör. geç orijinal teyit eder, kaybolan yeniden gönderim `pending_lost`'ta) | Motor teyitli birimin `pending_lost` kaydını budamaz (taban = bu PR) | Evet | Kalan kayıt ARQ'ya girer (karar üretilir) | İlk ACK'te `store.remove`; kalan kararlar başlık-only (`stale_decisions`) VE harita kaydı açar (budanmaz) | `stale_decisions` sürer (budama #140 madde 2); harita kaydı AÇILMAZ | `store_miss_guard_loss_and_reorder`; motor `k4g05br_decision_neutral_after_confirmation_with_pending_work` |
| 8 | **FEC RECOVERED** | `on_recovered_control` 5589-5647: `pending_lost`/`dup_queue` iptali; uçuştaki kayıt kalır (satır 7'ye düşer) | Evet | Listelenen her birim teyit edilir | `store.remove` + `acked()` motordan ÖNCE | Drenajda (motordan SONRA); store'da varsa düşer — sonuç tabanla aynı | `store_miss_guard_fec_recovered` |
| 8b | RECOVERED, motorun hiç göndermediği birim | — | #140 envanterine göre erişilemez (bu PR'da yeniden incelenmedi) | Bu PR: yankılanır (#140: teyit etmez) | Store'da varsa düşer | Aynı (tabanla) | **#140 ayrımı**; motor testi yalnız motorun bildiği birimi sınar |
| 9 | Runtime `PN_SEQ_WINDOW` (65 536 PN) | runtime 78 | Evet | Çevrilmez → motora gitmez | Silme yok | Değişmez | — |
| 10 | Runtime store sınırı (`max_inflight_units` 4096) | teklif kapısı runtime 2458 | Evet | — | Doluyken teklif alınmaz | Değişmez; `CONFIRMED_UNITS_CAP ≥ max_inflight_units` pinli | `confirmed_units_cap_covers_default_store_bound` |
| 11 | Oturum kapanışı (`drain_cap`) | runtime 2675-2693 | Evet | Motor düşer | Store düşer, `undelivered` sayılır | Değişmez | — |
| 12 | Runtime `repair_store` tahliyesi (64 blok) | runtime 2565 | Evet | — | Parite gövdesi, birim payload'ı değil | Değişmez | — |
| 13 | Bench / harness (ürün dışı; `crates/bench/src/kama.rs`, sim, `fs_harness`, `udp_runner`) | — | — | Drenaj yapmazlar: tampon tavanda kalır (≈ 32 KiB / Sender), taşma sayacı büyür | Payload deposu yok | Değişmez; karar etkisi yok (nötrlük testi taşan ikizle) | — |

Satır 7 ve 8'de bu PR, tabandaki harita sızıntısını (kod okuması) kapatır ama başlık-only kararları
(`stale_decisions`) kapatmaz: o, motorun teyitli birimin bekleyen işini budamasını ister (#140
madde 2).

## Davranış farkı ve açık soru (EVOLUTION §9.5)

Motor kararları ve tel biçimi değişmez. Runtime davranışı değişir: `ClientHandle::acked()`,
`outstanding()` düşümü, `ClientStats.acked` ve payload silme artık motorun teyit çıktısına bağlıdır.
Motorun kaydını unuttuğu bir iletimin geç ACK'i (tombstone TTL'i ≥ ~25 s, TOMBSTONE_MAX tahliyesi,
kopya kökenli kayıp, kopya kaydının kuyruk kurtarmasıyla emekliliği) birimi teyit etmez; birimin
sonraki iletimi başlık-only yerine gerçek gövdeyle gider ve app-düzeyi teyit gecikebilir. Bir ACK
çerçevesi içindeki teyit sırası motor sırasıdır (uçuş/tombstone `acked_seqs` sırasıyla, ardından
un-loss), runtime seq sırası değil (`acked()` akışının çerçeve içi sırası donmuş sözleşmede tanımlı
değildir; INV-011 teslim tarafında sırasızlığı beyan eder).

Soru: bu, `docs/transport-v1/EVOLUTION.md` §9.5 anlamında "davranış değiştiren" bir değişiklik midir
(§9.5 yalnız doküman, salt-gözlem tanı, test ve CI değişikliklerini sürüm kırmadan kabul eder)? Evetse
hangi kimliği taşır? #140 kendi motor değişikliği için `kama-v1.1-full` / `transport-v1.1` önermişti.
Bu PR'ın varsayılanı: kimlikler DEĞİŞMEZ (`kama-v1-full` / `transport-v1`); §22'nin altı beyanı PR
açıklamasındadır. İlgili donmuş metin (değiştirilmedi, bu PR onu daraltır):
`docs/transport-v1/ARCHITECTURE.md` ("`acked()` göndericiye ilk-ACK-kazanır birim id akışı verir") ve
`docs/transport-v1/TRANSPORT-CRYPTO-HANDOFF.md` (teslim olayı ve gönderici onayı satırları). Bu PR'da
"ilk ACK" = motorun canlı kaydıyla eşleşen ilk ACK ya da RECOVERED.

## Testler — ölçülmüş sonuçlar

`crates/client-core/tests/k4g05b_runtime_ownership.rs`. Taban kolu: `4a9e97f1` + taban kopyası (yalnız
iddia 5'in gövdesi boş ve tavan pini yok; alanlar tabanda yok). Dal kolu: bu dal. Her kol tek koşu,
yerel loopback; süreler o koşunun çıktısıdır (`--nocapture`).

Koşu düzeni: iki kol aynı makinede ardışık, aynı test kaynağıyla; testler dosya içinde sırayla
(serileştirme kilidi). Paylaşılan hedef dizininde parmak izi yol-bağımsız olduğundan her koldan önce
kaynak zaman damgaları tazelendi (karışık sürüm derlenmesin; ilk taban denemesi bu yüzden geçersiz
sayıldı ve tabloya girmedi). Her hücre TEK koşudur; süreler gözlemdir, dağılım ölçülmedi.

| Test | Envanter satırı | Taban `4a9e97f1` | Dal |
|---|---|---|---|
| `late_ack_after_ttl_keeps_payload` | 1 | **Kırmızı** — iddia (1): geç ACK çerçevesi `mapped_seqs = 3`, `new_units = 1`; geç ACK sonrası olaylar `[1, 4072]` (hedef 1) | Yeşil — geç ACK çerçevesi `mapped_seqs = 3`, `new_units = 0`; faz B teyidi 12.5 s (tavan 22.5 s); `stale_decisions = 0` |
| `late_ack_after_cap_keeps_payload` | 2 | **Kırmızı** — iddia (1): `new_units = 1`, hedef 1 olaylarda; kayıt 20.5 s'de düştü, tablo 4096 | Yeşil — kayıt 16.0 s'de düştü (tablo 4096); geç ACK öncesi boşaltma 19.7 s (bütçe 60 s); faz B 10.1 s; `stale_decisions = 25` |
| `late_ack_after_dup_loss_keeps_payload` | 3 | **Kırmızı** — iddia (1): `new_units = 1`; olaylar `[0, 44]` (hedef 0) | Yeşil — `new_units = 0`; faz B 0.83 s; `stale_decisions = 0` |
| `parity_ack_no_confirmation_payload_kept` | 5 | Yeşil — `acked = 4`, `fec_recovered_ctrl_units = 4` | Yeşil — `acked = 4`, `fec_recovered_ctrl_units = 4`; faz 2 2.9 s |
| `store_miss_guard_loss_and_reorder` | 7 | Yeşil — `stale_decisions = 0` (harita iddiası tabanda yok) | Yeşil — `stale_decisions = 0`; haritalar boş |
| `store_miss_guard_fec_recovered` | 8 | Yeşil — `stale_decisions = 3` (harita iddiası tabanda yok) | Yeşil — `stale_decisions = 3`; haritalar boş |
| `confirmed_units_cap_covers_default_store_bound` | 10 | — (sabit tabanda yok) | Yeşil |

Taban: 3 kırmızı / 3 yeşil (iddia 1, beklenen ayrışma). Dal: 7 yeşil.

- **Bekçiler (satır 7, 8):** iki kolda da yeşil; bu koşuda ayırt edici değiller. `stale_decisions` iki
  kolda aynıdır (satır 8'de 3): bu PR başlık-only kararları kapatmaz. Tabandaki harita sızıntısı kod
  okumasıdır (taban kancasız), ölçülmedi; dalda haritaların boş olduğu ölçüldü.
- **Fikstür geçmişi (aynı gün, tabloya girmedi):** testler paralel koşarken tahliye kolu iki kolda da
  ön koşulda kırmızıydı (dal: kayıt 25.17 s'de düştü, tablo en çok 3078; taban: 25.21 s ve 25.17 s,
  tablo en çok 3973) — TTL tahliyeden önce geldi; serileştirmeyle (üç koşu) tahliye 9.7–20.5 s'de
  gerçekleşti. Birim başına iki düşüş denemesi ACK ilerlemesini
  durdurdu (25 s'de en çok 28 tombstone) ve bırakıldı. Tahliye kolunun TTL'e payı dardır; yavaş
  makinede ön koşul kırmızısı (iddia değil) beklenebilir.
- **Boşluk bekçisi:** her geç-ACK kolunda geç ACK çerçevesinin istemcide işlendiği ve motor seq'ine
  çevrildiği (`mapped_seqs > 0`) iki kolda da doğrulandı; kollar yalnız `new_units`'te ayrışır.

## Motor birim testleri (`sender.rs` `tests::k4g05br_teyit`, iki motor: `Sender::new`, `Sender::with_cc_port`)

- `k4g05br_inflight_ack_confirms_unit_ids_not_seqs` — Data / Retransmit / Duplicate uçuş kaydının
  ACK'i birim kimliğini verir (`acked_seqs` sırası); ikinci drenaj boş.
- `k4g05br_tombstone_late_ack_confirms_once_per_event` — kol-1 (tombstone + un-loss aynı olayda; tek
  teyit) ve kol-2 (superseded tombstone).
- `k4g05br_unloss_without_tombstone_confirms` — tombstone tahliye edilmiş, kayıp kaydı bekliyor.
- `k4g05br_late_ack_without_live_record_does_not_confirm` — (a) TTL budaması, (b) tahliye, (c) kopya
  kökenli kayıp, (d) bilinmeyen / `next_seq` üstü seq, (e) yinelenen ACK.
- `k4g05br_parity_ack_never_confirms_esi_namesake` — uçuştaki ve tombstone'daki parite ACK'i.
- `k4g05br_recovered_control_confirms_listed_units` — bilgi sırası, olay içinde tekil, karar yok.
- `k4g05br_confirmed_units_buffer_bounded_drop_oldest` — tavan, en eski düşer, sayaç.
- `k4g05br_decision_neutral_after_confirmation_with_pending_work` — satır 7 durumunda drenajlı motor
  ile hiç drenajsız ikizi her olayda aynı karar ve uyanma.
- `k4g05br_confirmation_output_is_decision_neutral_{legacy,cc_port}` — LCG izli kanal modeli (kayıp,
  geç teslim, kesinti, RECOVERED, parite); drenajlı motor ile taşan ikizi her olayda aynı karar ve
  uyanma, son durumda aynı tanı yüzeyleri.

Mutasyon denetimi (yerel, commit edilmedi): parite süzgecini, un-loss itişini, tombstone itişini ve
olay içi tekilleştirmeyi kaldırmak ilgili testi kırmızıya çevirdi. Drenaja `pending_lost` budaması
eklemek (#140 madde 2'ye benzer yan etki) satır-7 senaryo testini (ilk ayrışma eski motor kolunda) ve
port motorunun LCG izini kırmızıya çevirdi; eski motorun LCG izi bu mutasyonda YEŞİL kaldı (iz, eski
motorda o durumu nadiren kurar) — eski motor için duyarlılık senaryo testine dayanır.

## #140'ta kalanlar

- Motor: `UnitOwnership` dizini, teyit kümesi ("birim oturumda en çok bir kez"), teyitli birime karar
  üretmeme (madde 2), RTO_MAX tavanı (madde 3), sahiplenme adayları (madde 4), K_MAP, parite kaydı
  sahibi ve süre dolumu (madde 1). Satır 4, 6, 8b ve satır 5'in kopya iptali.
- Kimlik değişikliği (`transport_v1_freeze.rs` düzenlemesi) — bu PR kimlik değiştirmez.
- Runtime testinin #140'taki parite faz 2'si ("esi adaşı kopyaları sürer, bölünme (r, k−r)") ve
  bekçilerdeki `stale_decisions == 0` iddiası: ikisi de motor kısmına bağlı.
