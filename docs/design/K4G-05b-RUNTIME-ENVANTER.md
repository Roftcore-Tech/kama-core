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

Satır atıfları: "taban" = `4a9e97f1`, "dal" = `f86f2b43` (`git show <sha>:<yol>`).

## Motorun teyit çıktısı (bu PR)

`Sender::take_confirmed_units() -> Vec<u64>` (dal `sender.rs` 5786) son çağrıdan beri teyit edilen
birim kimliklerini (`OpaqueUnit::id`; seq ya da ESI değil) verir: olaylar üretim sırasıyla, bir
olayın teyitleri olay içinde tekil ve artan birim kimliği sırasıyla (`emit_unit_confirmations`, dal
5707: yerinde sırala + yinelenenleri at, ek ayırma yok). Olaylar arası tekilleştirme yapmaz.

Tampon politikası (dal 1357, 5720): bir `on_event` / `on_event_batch` çağrısının KENDİ teyitleri
hiçbir zaman düşmez; sayıları çağrının girdisiyle sınırlıdır (ACK: canlı kaydı eşleşen seq'ler;
RECOVERED: listelenen birimler). `CONFIRMED_UNITS_CAP = 4096` yalnız DRENAJ EDİLMEMİŞ birikime,
sonraki çağrının girişinde uygulanır (en eski fazlası düşer, `diag_confirmed_units_overflow` düşen
sayı kadar artar; dal 2442, 2703). Runtime her `on_event`'ten hemen sonra drenaj yaptığından
girişte birikim boştur: teyit kaybı store sınırından (`max_inflight_units`), parçalamadan ve ACK
çerçevesi boyundan (`MAX_ACKED_SEQS`) bağımsız olarak yapısal şekilde yoktur. Önceki sürüm (dal
`fcd8efb6`) tavanı çağrı İÇİNDE uyguluyordu: tek ACK çerçevesi 4096'dan çok farklı birimi teyit
ederse en eski teyit düşer ve o birimin payload'ı store'da kalırdı; inceleme bulgusuyla düzeltildi.

| Üreten yol | Atıf (taban → dal) |
|---|---|
| Uçuştaki kaydın ACK'i (köken `FecParity` değil) | `on_ack_received` uçuş dalı taban 3275 → dal 3320-3322 |
| Tombstone kaydının geç ACK'i (kol-1/kol-2; köken `FecParity` değil) | taban 3299 → dal 3349-3351 |
| Kayıp ilan edilip yeniden planı bekleyen kaydın un-loss eşleşmesi | taban 3365 → dal 3421 |
| `RecoveredControl`: listelenen her birim (motor kaydı süzgeci yok) | `on_recovered_control` taban 5589 → dal 5696 |

Üretmeyen: parite ACK'i (uçuşta ya da tombstone'da); canlı kaydı olmayan seq'in ACK'i (TTL'i dolup
budanmış ya da tahliye edilmiş tombstone, kopya kökenli kayıp, kuyruk kurtarmasının tombstone'suz
emekliye ayırdığı kopya kaydı, bilinmeyen seq); kaydı çoktan düşmüş seq'in yinelenen ACK'i;
zamanlayıcılar.

**Karar-nötrlük.** Yeni kod yalnız iki çıktı alanına yazar (`confirmed_units_out`,
`confirmed_units_overflow`); hiçbir karar yolu onları okumaz (çağrı girişindeki budama da yalnız
bu iki alana dokunur). `acked_units` (kopya iptali) bayt
düzeyinde değişmedi. Pinler: `sender.rs` `tests::k4g05br_teyit` (aşağıda) ve değişmeden yeşil kalan
`tv_*`, `sim_determinism`, `cc_port_sim`, `sender_diag_contract`, `e1_goster`, `e2a_goster`,
kama-engine `n8_v2`, client-core `ctx_first_dispatch`.

## Runtime (bu PR)

`crates/client-core/src/runtime.rs`, `client_task` (dal satırları):
- `drain_confirmed!` (2127): her `session.on_event` dönüşünde `take_confirmed_units()` drenajı.
  `store.remove`, `unit_acked!`, `outstanding` düşümü, `stats.acked` /
  `stats.fec_recovered_ctrl_units` (`ConfirmVia`, 1858) ve `unit_seqs`/`seq_to_unit` budaması YALNIZ
  burada. Store'da olmayan birimin teyidi sessizce atlanır (tekilleştirme = store varlığı; birim
  kimlikleri yeniden kullanılmaz).
- `drive!` (2150): drenaj kararlar işlenmeden ÖNCE; karar döngüsü store'da olmayan (teyitli) birime
  harita kaydı AÇMAZ (2171).
- `seq_to_unit` artık hiçbir yolda okunmaz (ACK yolu seq → birim araması yapmaz); yalnız
  `seq_to_unit_len` sızıntı bekçisini besler. Şekli #140'ın runtime kısmıyla aynı tutuldu;
  kaldırılması Aralık birleşmesine kalır.
- `ClientStats::stale_decisions` iki kaynaklıdır: teyitli birime karar (satır 7) ve runtime'da FEC
  kapalıyken ya da gönderim kaydı olmadan gelen `FecParity` kararı (ürün varsayılanlarında yok). İki
  kaynakta da karar tele ÇIKMAZ (aşağıda "Atlama politikası").
- `ClientStats::stale_skipped` (ve `ClientLiveDiag::stale_skipped`, bench `KP-CLIENT` satırı):
  gövdesi olmadığı için tele çıkarılmayan karar toplamı, `= stale_decisions + fec_repair_missing`.
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
| 7 | **Teyit sonrası kalan kayıt** (ör. geç orijinal teyit eder, kaybolan yeniden gönderim `pending_lost`'ta) | Motor teyitli birimin `pending_lost` kaydını budamaz (taban = bu PR) | Evet | Kalan kayıt ARQ'ya girer (karar üretilir) | İlk ACK'te `store.remove`; kalan kararlar başlık-only (`stale_decisions`) VE harita kaydı açar (budanmaz) | Karar sürer (budama #140 madde 2) ama ATLANIR: tele datagram çıkmaz (`stale_decisions` + `stale_skipped`); harita kaydı AÇILMAZ; motorda ardılı "Atlama politikası" | `store_miss_guard_loss_and_reorder`; motor `k4g05br_decision_neutral_after_confirmation_with_pending_work` |
| 8 | **FEC RECOVERED** | `on_recovered_control` 5589-5647: `pending_lost`/`dup_queue` iptali; uçuştaki kayıt kalır (satır 7'ye düşer) | Evet | Listelenen her birim teyit edilir | `store.remove` + `acked()` motordan ÖNCE | Drenajda (motordan SONRA); store'da varsa düşer — sonuç tabanla aynı | `store_miss_guard_fec_recovered` |
| 8b | RECOVERED, motorun hiç göndermediği birim | — | #140 envanterine göre erişilemez (bu PR'da yeniden incelenmedi) | Bu PR: yankılanır (#140: teyit etmez) | Store'da varsa düşer | Aynı (tabanla) | **#140 ayrımı**; motor testi yalnız motorun bildiği birimi sınar |
| 9 | Runtime `PN_SEQ_WINDOW` (65 536 PN) | runtime 78 | Evet | Çevrilmez → motora gitmez | Silme yok | Değişmez | — |
| 10 | Runtime store sınırı (`max_inflight_units` 4096) | teklif kapısı runtime 2458 (dal 2563) | Evet | — | Doluyken teklif alınmaz | Değişmez; teyit kaybı bu sınırdan bağımsız (motor çağrının kendi teyitlerini düşürmez; parçalama açıkken store bu sınırı aşabilir, sonuç değişmez) | motor `k4g05br_confirmed_units_call_output_never_dropped`; runtime testlerinde `confirmed_units_overflow == 0` |
| 11 | Oturum kapanışı (`drain_cap`) | runtime 2675-2693 | Evet | Motor düşer | Store düşer, `undelivered` sayılır | Değişmez | — |
| 12 | Runtime `repair_store` tahliyesi (64 blok) | runtime 2565 | Evet | — | Parite gövdesi, birim payload'ı değil | Değişmez | — |
| 13 | Bench / harness (ürün dışı; `crates/bench/src/kama.rs`, sim, `fs_harness`, `udp_runner`) | — | — | Drenaj yapmazlar: birikim her çağrı girişinde tavana iner (≈ 32 KiB + son çağrının teyitleri / Sender), taşma sayacı büyür | Payload deposu yok | Değişmez; karar etkisi yok (nötrlük testi taşan ikizle) | — |

Satır 7 ve 8'de bu PR, tabandaki harita sızıntısını (kod okuması) kapatır ve teyitli birime verilen
kararı tele çıkarmaz (atlar). Kararın kendisini (`stale_decisions`) kapatmaz: o, motorun teyitli
birimin bekleyen işini budamasını ister (#140 madde 2).

## Atlama politikası (sahip kararı 2026-11-11, #174 madde 2)

Karar metni: "gövdesiz gönderim DATAGRAM ÜRETMEMELİ — runtime o kararı atlamalı (sayaç:
stale_skipped), tele boş/eksik çerçeve çıkmamalı."

PR için tek cümle: **Gövdesi olmayan motor kararı (teyitli birime karar ya da repair-store'da
sembolü olmayan parite) artık datagram üretmez; runtime kararı atlar ve `stale_skipped`'te sayar,
tele boş gövdeli DATA, yalnız önekli DATA_FRAG ya da boş sembollü FEC çıkmaz (röle sayımıyla her
runtime testinde 0).**

| Yol (`runtime.rs` `drive!`) | Önceki (`36820b37`) | Bu PR | Neden sayacı |
|---|---|---|---|
| (a) Data / Retransmit / Duplicate, birim store'da yok (teyitli birim; satır 7, 8 → 7) | Başlık-only: boş gövdeli DATA; parçalama açıkken 11 B sıfır önekli DATA_FRAG | Atlanır (`continue`; seal yok, PN harcanmaz, harita kaydı yok) | `stale_decisions` |
| (b) FecParity, FEC açık, sembol repair-store'da yok (blok tahliyesi / latch-0) | Boş sembollü FEC çerçevesi | Atlanır | `fec_repair_missing` |
| (c) FecParity, FEC kapalı ya da gönderim kaydı yok | Zaten tele çıkmıyordu | Aynı | `stale_decisions` |

`stale_skipped` = (a) + (b) + (c) = `stale_decisions + fec_repair_missing` (tanım; her runtime testinde
iddia edilir). Kurtarma kapalıyken bastırılan karar ayrı sayaçtadır (`recovery_suppressed`). Sıfır-önek
gövdesi (`stale_body`) kaldırıldı. Tel biçimi, golden vektörler ve profil kimliği değişmedi.

**Ardıl (motorda).** Motor kararı işlemiştir: atlanan kararın kaydı uçuştadır ve ACK'i hiç gelmez;
motor onu kayıp gibi işler (INV-RATE-1: kayıp hızı düşürmez, kurtarma kolunu besler). Önceki
başlık-only emisyonda alıcı seq'i ACK'liyor ve motor kaydı kapatıyordu. Atlama bu kapanışı kaldırır;
motor aynı birime yeniden karar verebilir ve bunlar da atlanır. Bu ardıl iki yerde ölçüldü:

*Motor simi* (`sender.rs` `tests::k4g05br_teyit::k4g05br_ghost_after_skipped_stale_decision`; motor
kodu değişmedi). Satır-7 durumu (geç orijinal birimi teyit eder, yeniden gönderim kaydı
`pending_lost`'ta) kurulur; ardından 120 s sahte saat: runtime pompası taklidi (`Pto` 5 ms,
`RtpropProbe` 200 ms), kayıpsız sıralı kanal (ACK gönderimden 40 ms sonra), diğer birimler 50 ms
aralıkla (100 B). "Atla" kolunda teyitli birime verilen kararlar ACK'lenmez; "başlık" kolunda ACK'lenir.
Tek koşu, belirlenimli; sayılar `--nocapture` çıktısıdır.

| Motor | Kol | Hayalet karar (120 s) | Tür ve anlar (teyitten, ms) | 120 s'de motor birimi tutuyor mu | Hedefin uçuş baytı tepesi | Faz / hız sonu | Diğer birimler (2400 sunulan) |
|---|---|---|---|---|---|---|---|
| legacy + `validated_v1` kuyruk kurtarması (runtime motoru) | atla | 14 | 2 Retransmit (5, 60) + 12 Duplicate (1590, 3130, …, 24715, 45195, 91240); 10 s kovaları `[10,1,1,0,1,0,0,0,0,1,0,0]` | Evet: kopya kuyruğunda, aralık 81.9 s (sonraki ≈ 173 s) | 400 B | 49 faz geçişi, hiç `Idle` yok; son `DrainRtprop`, btlbw 2500, pacing 1875 | 2389 teyit; ortalama 143 ms, en büyük 1400 ms |
| aynı | başlık | 1 (5 ms, Retransmit) | — | Hayır (26 s'de bıraktı) | 100 B | 3551 faz geçişi (`Idle` ↔ `Startup`/`Drain`); son `Startup` | 2399 teyit; ortalama 40 ms, en büyük 40 ms |
| legacy (`Sender::new`, kuyruk kurtarması kapalı) | atla | 14 | 2 Retransmit (5, 1540) + 12 Duplicate (3040 … 96200) | Evet (aralık 81.9 s) | 400 B | 52 geçiş; son `DrainRtprop`, pacing 1875 | 2388 teyit; ortalama 116 ms, en büyük 720 ms |
| aynı | başlık | 1 | — | Hayır (26 s) | 100 B | son `Startup` | 2399; 40 / 40 ms |
| cc_port (`Sender::with_cc_port`) | atla | 266 | 254 Retransmit (ilk ≈ 13 s, ≈ 50 ms aralıkla) + 12 Duplicate; kovalar `[200,62,1,1,0,1,0,0,0,1,0,0]` | Evet (aralık 81.9 s) | 100 B | 253 zaman eşiği kaybı, 267 BBR tıkanıklık olayı; pencere 4800 B | 2399; 40 / 40 ms |
| aynı | başlık | 1 | — | Hayır (1 s) | 100 B | 2 tıkanıklık olayı; pencere 254 700 B | 2399; 40 / 40 ms |

Atla ↔ başlık, birim birim teyit gecikmesi: runtime motorunda 2389 ortak birimin 629'unda değişti
(en büyük fark 1360 ms); kuyruk kurtarmasız legacy'de 2388'in 565'inde (680 ms); cc_port'ta 2399'un
0'ında. Okuma: legacy motorda hayalet birim kopya kuyruğunda tavansız geometrik aralıkla kalır
(yalnız birimin herhangi bir kaydının ACK'i iptal eder); kuyruk boş kalmadığı için motor `Idle`'a
dönmez ve bu seyrek, app-limited kanalda `Steady`/`DrainRtprop`/`ProbeUp` döngüsüne girer
(`DrainRtprop`'ta pacing 1875 < sunulan yük 2000 B/s). cc_port'ta yeniden iletim tavanı yoktur:
atlanan her karar zaman eşiğiyle kayıp sayılır ve BBR penceresi tabana iner; bu yükte diğer
birimlerin gecikmesi değişmedi, daha yüksek yükte etkisi ölçülmedi. Etkinin büyüklüğü bu kanal
modeline özgüdür; netem ya da ürün koşusunda ölçülmedi. Bellek sınırlıdır (kopya kuyruğunda tek
kayıt; uçuş, kayıp, kopya kuyruk tepeleri ≤ 5 / 0 / 1; tombstone ≤ 8), panik yok. Kaynakta kapatma
#140 madde 2'dir (motor teyitli birimin bekleyen işini budar, karar üretmez).

*Runtime* (legacy + `validated_v1`, gerçek UDP loopback; `store_miss_guard_fec_recovered`, tek koşu):
blok teyidinden 0.36 s sonra ilk atlama; `stale_skipped` 1. s'de 2, 3. s'de 3, 10. s'ye dek 3 (yeni trafik
yok); motorun uçuş kuyruğunda 2 kayıt 10 s boyunca kaldı, kopya yok. Aynı koşuda tahliye kolunda
`stale_skipped = 123`, diğer kollarda 0 (aşağıdaki tablo).

**Tel bekçisi ve mutasyon.** Runtime testlerinin röle düğümü istemciden gelen her datagramı kurallardan
önce sayar; boş gövdeli DATA, yalnız önekli DATA_FRAG ve boş sembollü FEC sayısı her testte 0 olmalı
(iddia 6). Vakum: FEC bekçisinde en az bir atlanan karar ön koşuldur (motor kısmı birimin işini
budadığında ters çevrilir). Mutasyon (yerel, commit edilmedi, yedekten geri alındı): (a) yolunda
başlık-only emisyon geri konunca FEC bekçisi kırmızı — 18 istemci datagramının 3'ü boş gövdeli DATA
(PN 15, 16, 17). (b) yolunu tetikleyen runtime testi yok (repair-store tahliyesi ya da latch-0 bu
fikstürlerde kurulmuyor); o yol kod okuması ve sayaç kimliğiyle kapsanır.

## CI (sahip kararı 2026-11-11, #174 madde 1)

`k4g05b_runtime_ownership` testleri (gerçek zamanlı, sırayla, yerel tek koşu 133.9 s) `#[ignore]` ile
hızlı kümeden (`test-fast`) çıktı. Her PR'da test-fast'a paralel `test-runtime` işi
`cargo test -p client-core --all-features --test k4g05b_runtime_ownership -- --ignored` koşar;
vakum bekçisi "`test result: ok. 6 passed; 0 failed`" birebir. İş `ci-verdict` `needs` listesindedir.
Önbellek: iki iş aynı `Swatinem/rust-cache` `shared-key`'ini (`core-ci-test`) kullanır, `test-runtime`
yalnız okur (`save-if: false`). main'de testler `#[ignore]` olduğundan `full-workspace` koşumuna girmez;
`core-main` `product-profile` işine aynı adım eklendi. Yerel kapı (`scripts/ci/local-gate.sh`) adımı ve
`ci.yml` özeti güncellendi. Hayalet simi (`k4g05br_ghost_after_skipped_stale_decision`, belirlenimli,
yerel ≈ 6 s) hızlı kümede kalır.

## Davranış farkı ve açık soru (EVOLUTION §9.5)

Motor kararları ve tel biçimi değişmez. Tel TRAFİĞİ değişir: gövdesi olmayan karar artık datagram
üretmez (önceki başlık-only emisyon kalktı; "Atlama politikası"), motor o kaydın ACK'ini almaz.
Runtime davranışı değişir: `ClientHandle::acked()`,
`outstanding()` düşümü, `ClientStats.acked` ve payload silme artık motorun teyit çıktısına bağlıdır.
Motorun kaydını unuttuğu bir iletimin geç ACK'i (tombstone TTL'i ≥ ~25 s, TOMBSTONE_MAX tahliyesi,
kopya kökenli kayıp, kopya kaydının kuyruk kurtarmasıyla emekliliği) birimi teyit etmez; birimin
sonraki iletimi başlık-only yerine gerçek gövdeyle gider ve app-düzeyi teyit gecikebilir. Bir ACK
çerçevesi içindeki teyit sırası artan birim kimliğidir, runtime seq sırası değil (`acked()`
akışının çerçeve içi sırası donmuş sözleşmede tanımlı değildir; INV-011 teslim tarafında
sırasızlığı beyan eder).

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
iddia 5'in gövdesi boş; alanlar tabanda yok). Dal kolu: `fcd8efb6` (teyit tamponu politikası
düzeltmesinden ÖNCEKİ commit). Her kol tek koşu, yerel loopback; süreler o koşunun çıktısıdır
(`--nocapture`). Düzeltmeden sonra (`216206bc` motor, `f86f2b43` runtime) dosya yalnız kapı
koşusunda (`test -p client-core`, çıktı yakalanmış, tek koşu) yeniden koştu: 6/6 yeşil, 88.9 s.
Tablodaki süre ve sayaç gözlemleri yeniden ölçülmedi.

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

Taban: 3 kırmızı / 3 yeşil (iddia 1, beklenen ayrışma). Dal (`fcd8efb6`): 6 yeşil (o commit'teki
yedinci test `confirmed_units_cap_covers_default_store_bound` yalnız `max_inflight_units ≤
CONFIRMED_UNITS_CAP` sabit karşılaştırmasıydı; tampon politikası düzeltmesiyle yükü kalmadı ve
kaldırıldı).

- **Bekçiler (satır 7, 8):** iki kolda da yeşil; bu koşuda ayırt edici değiller. `stale_decisions` iki
  kolda aynıdır (satır 8'de 3): bu PR kararı kapatmaz (o koşuda karar başlık-only gidiyordu; atlama
  politikasından sonra tele çıkmaz). Tabandaki harita sızıntısı kod
  okumasıdır (taban kancasız), ölçülmedi; dalda haritaların boş olduğu ölçüldü.
- **Fikstür geçmişi (aynı gün, tabloya girmedi):** testler paralel koşarken tahliye kolu iki kolda da
  ön koşulda kırmızıydı (dal: kayıt 25.17 s'de düştü, tablo en çok 3078; taban: 25.21 s ve 25.17 s,
  tablo en çok 3973) — TTL tahliyeden önce geldi; serileştirmeyle (üç koşu) tahliye 9.7–20.5 s'de
  gerçekleşti. Birim başına iki düşüş denemesi ACK ilerlemesini
  durdurdu (25 s'de en çok 28 tombstone) ve bırakıldı. Tahliye kolunun TTL'e payı dardır; yavaş
  makinede ön koşul kırmızısı (iddia değil) beklenebilir.
- **Boşluk bekçisi:** her geç-ACK kolunda geç ACK çerçevesinin istemcide işlendiği ve motor seq'ine
  çevrildiği (`mapped_seqs > 0`) iki kolda da doğrulandı; kollar yalnız `new_units`'te ayrışır.

**Atlama politikasından sonraki koşu** (dal, `-- --ignored --nocapture`, tek koşu, yerel loopback,
6/6 yeşil, 133.9 s). Yukarıdaki tabloyla kontrollü kıyas DEĞİLDİR (farklı derleme ve koşu; süreler
gözlem):

| Test | İstemci datagramı (gövdesiz) | `stale_skipped` (= `stale_decisions` + `fec_repair_missing`) | Gözlem |
|---|---|---|---|
| `late_ack_after_ttl_keeps_payload` | 7118 (0) | 0 | faz B 19.2 s |
| `late_ack_after_cap_keeps_payload` | 13 820 (0) | 123 (123 + 0) | kayıt 17.6 s'de düştü (tablo 4096); faz B 23.8 s |
| `late_ack_after_dup_loss_keeps_payload` | 209 (0) | 0 | faz B 0.87 s |
| `parity_ack_no_confirmation_payload_kept` | 17 (0) | 0 | faz 2 3.1 s |
| `store_miss_guard_loss_and_reorder` | 543 (0) | 0 | — |
| `store_miss_guard_fec_recovered` | 15 (0) | 3 (3 + 0) | ilk atlama teyitten 0.36 s sonra |

## Motor birim testleri (`sender.rs` `tests::k4g05br_teyit`, iki motor: `Sender::new`, `Sender::with_cc_port`)

- `k4g05br_inflight_ack_confirms_unit_ids_not_seqs` — Data / Retransmit / Duplicate uçuş kaydının
  ACK'i birim kimliğini verir (seq'ler ters sırada ACK'lenir, çıktı artan kimlik sırası); ikinci
  drenaj boş.
- `k4g05br_tombstone_late_ack_confirms_once_per_event` — kol-1 (tombstone + un-loss aynı olayda; tek
  teyit) ve kol-2 (superseded tombstone).
- `k4g05br_unloss_without_tombstone_confirms` — tombstone tahliye edilmiş, kayıp kaydı bekliyor.
- `k4g05br_late_ack_without_live_record_does_not_confirm` — (a) TTL budaması, (b) tahliye, (c) kopya
  kökenli kayıp, (d) bilinmeyen / `next_seq` üstü seq, (e) yinelenen ACK.
- `k4g05br_parity_ack_never_confirms_esi_namesake` — uçuştaki ve tombstone'daki parite ACK'i.
- `k4g05br_recovered_control_confirms_listed_units` — olay içinde tekil, artan kimlik sırası, karar
  yok.
- `k4g05br_confirmed_units_call_output_never_dropped` — (a) tavandan çok teyit üreten TEK çağrının
  bütün teyitleri döner, sayaç 0; (b) drenajsız birikim sonraki çağrının girişinde tavana iner (en
  eski fazlası düşer, sayaç düşen kadar), o çağrının teyitleri yine eksiksiz.
- `k4g05br_decision_neutral_after_confirmation_with_pending_work` — satır 7 durumunda drenajlı motor
  ile hiç drenajsız ikizi her olayda aynı karar ve uyanma.
- `k4g05br_confirmation_output_is_decision_neutral_{legacy,cc_port}` — LCG izli kanal modeli (kayıp,
  geç teslim, kesinti, RECOVERED, parite); drenajlı motor ile taşan ikizi her olayda aynı karar ve
  uyanma, son durumda aynı tanı yüzeyleri.

Mutasyon denetimi (yerel, commit edilmedi; `fcd8efb6` üzerinde, tekilleştirme o sürümde ilk görülme
sırasını koruyan kümeyle yapılıyordu): parite süzgecini, un-loss itişini, tombstone itişini ve olay
içi tekilleştirmeyi kaldırmak ilgili testi kırmızıya çevirdi. Drenaja `pending_lost` budaması
eklemek (#140 madde 2'ye benzer yan etki) satır-7 senaryo testini (ilk ayrışma eski motor kolunda) ve
port motorunun LCG izini kırmızıya çevirdi; eski motorun LCG izi bu mutasyonda YEŞİL kaldı (iz, eski
motorda o durumu nadiren kurar) — eski motor için duyarlılık senaryo testine dayanır.

Mutasyon denetimi (yerel, commit edilmedi, yedekten geri alındı; `216206bc` üzerinde): eski çağrı-içi
düşürme geri konunca `k4g05br_confirmed_units_call_output_never_dropped` kırmızı (eski motor
kolunda tek çağrının ilk 37 teyidi düştü; test ilk ayrışmada durur). Sıralama + tekilleştirme
kaldırılınca üç test kırmızı: sıra (`k4g05br_inflight_ack_confirms_unit_ids_not_seqs`,
`k4g05br_recovered_control_confirms_listed_units`) ve olay içi tekil teyit
(`k4g05br_tombstone_late_ack_confirms_once_per_event`).

## #140'ta kalanlar

- Motor: `UnitOwnership` dizini, teyit kümesi ("birim oturumda en çok bir kez"), teyitli birime karar
  üretmeme (madde 2), RTO_MAX tavanı (madde 3), sahiplenme adayları (madde 4), K_MAP, parite kaydı
  sahibi ve süre dolumu (madde 1). Satır 4, 6, 8b ve satır 5'in kopya iptali.
- Kimlik değişikliği (`transport_v1_freeze.rs` düzenlemesi) — bu PR kimlik değiştirmez.
- Runtime testinin #140'taki parite faz 2'si ("esi adaşı kopyaları sürer, bölünme (r, k−r)") ve
  bekçilerdeki `stale_decisions == 0` iddiası: ikisi de motor kısmına bağlı.
- Aralık birleşmesinde ele alınacak iki ayrım: bu PR'ın teyit çıktısı olay içinde artan birim
  kimliği sırasıyla ve çağrı girişinde budanan birikimle çalışır (motor testleri bunu pinler);
  #140'ın `confirmed_out`'u kendi sırası ve tavanıyla çalışır. Runtime (`drain_confirmed!`) iki
  biçime de bağlı değildir.
