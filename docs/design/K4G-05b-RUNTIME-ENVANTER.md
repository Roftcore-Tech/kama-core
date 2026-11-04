# K4G-05b — Kayıt-unutma envanteri: aynı geç ACK'te motor ve runtime (CEVAP 13 §3, 11-EK §3)

Tarih 2026-10-24. Şerit: 05b RUNTIME (`k4g/05b-runtime`). Taban: main `1b1425d2` (v1) ve 05b motor dalı
`7f81f184`. Satır atıfları iki tabanda da doğrulandı (`git show <sha>:<yol>`). `runtime.rs` iki tabanda
kimlik sabitleri dışında özdeştir; satır `> 231` için 05b = main + 4.

**Okuma kuralı (CEVAP 13 §3):** silme yolunun varlığı tek başına kusur kanıtı değildir. Her satırda aynı
olay için iki ucun sonucu yan yana yazılır; ayrışma yalnız ikisi farklıysa vardır.

## Ortak mekanizma (bütün satırlar buna dayanır)

| Uç | Kayıt | Silme / teyit | Atıf (main / 05b) |
|---|---|---|---|
| runtime | `store` (birim → gövde), `seq_to_unit`, `unit_seqs` | `drive!` her Data/Retransmit/Duplicate kararında `seq_to_unit`'e ekler; **hiç budamaz** (yalnız birim store'dan düşünce) | `runtime.rs` 1906-1909 / 1910-1913; ekleme 1997-1999 / 2001-2003 |
| runtime | ACK yolu | haritadaki HERHANGİ bir seq'in ACK'i → `store.remove(unit)` + `unit_acked!` (+ `stats.acked`) — motor sonucuna bakmaz | 2276-2287 / 2280-2291 |
| runtime | RECOVERED yolu | `unit = block·K + esi` → `store.remove` + `unit_acked!`, SONRA motora `RecoveredControl` | 2206-2230 / 2210-2234 |
| runtime | store-miss | gövdesi olmayan karar başlık-only çıkar, `stats.stale_decisions += 1` | 2072-2078 / 2076-2082 |
| motor (main) | uçuş + tombstone + `pending_lost` | ACK eşleşmesi: uçuş 3075, tombstone 3094, un-loss (yalnız `pending_lost`) 3147-3160; **teyit kümesi yok**, "teyit" = `dup_queue.retain` (3209-3211) | `sender.rs` (main) |
| motor (05b) | aynı + sahiplik dizini (`units`) + teyit kümesi | teyit YALNIZ canlı kayıt eşleşmesinde (uçuş 3336, tombstone 3361, un-loss 3423-3439) ve RECOVERED'da (5640); `confirm_units` 6601; çıktı `take_confirmed_units` 6672 | `sender.rs` (05b) |

Sonuç: main'de runtime'ın "teslim edildi" kararı ile motorun "kayıt eşleşti" kararı **iki ayrı tablodan**
verilir. Motor bir iletim kaydını unuttuğunda runtime haritası o seq'i hâlâ tutar.

## Envanter

Sütunlar: yol · motor satırı (main / 05b) · v1'de (main) bugün etkin mi · geç ACK → motor · geç ACK → runtime
(main) · ayrışma · sonuç · uyarlama sonrası beklenen. "Uyarlama" = runtime payload'ı ve app teyidini YALNIZ
`take_confirmed_units()` drenajından alır (bu şeridin değişikliği).

| # | Yol | Motor satırı (main / 05b) | v1'de bugün etkin mi | Geç ACK → motor | Geç ACK → runtime (main) | Ayrışma | Sonuç | Uyarlama sonrası beklenen |
|---|---|---|---|---|---|---|---|---|
| 1 | **Tombstone TTL** (K_TOMB × T_loss; T_loss = max(9/8·srtt, 3/2·EAI), EAI ≥ 65/31 s ⇒ TTL ≥ ≈ 25.2 s) | Pto budaması 3262-3277 / 3568-3587; K_TOMB 1512 / 1595 | **Evet.** Kayıt kayıp ilanıyla ya da kuyruk-probu emekliliğiyle (main 5844, 05b 6689) tombstone'a girer; ilk taze ACK TTL'den geç gelirse bu yola düşer (ör. kesinti başında alıcıya ulaşmış, ACK'i kesintide kaybolmuş kayıt; kesinti > TTL — INT-02'nin 60 s kesintisi bu sınıftadır; erişilebilirlik akıl yürütmesi, ölçülmedi) | Kayıt yok → Expired/Unknown. main: eşleşme yok, kopya iptali yok; 05b: teyit YOK, birim sahipli kalır, yeniden gönderilir | `seq_to_unit` seq'i hâlâ tutar → `store.remove` + `acked()` + `outstanding −1` | **Var** (runtime teslim der, motor sahipli tutar) | Veri kaybı **yok** (ACK, alıcının o datagramı aldığını kanıtlar); **israf + sızıntı**: sonraki yeniden gönderimler gövdesiz (store-miss, `stale_decisions`↑); motor sonradan teyit ettiğinde `store.remove` None döner, temizlik çalışmaz → o birimin sonraki seq'leri `seq_to_unit`/`unit_seqs`'te kalır | Geç ACK teyit üretmez → payload yerinde, olay yok; sonraki yeniden gönderim gövdeli; onun ACK'i teyit eder → payload + haritalar o anda düşer. Test: `late_ack_after_ttl_keeps_payload` |
| 2 | **TOMBSTONE_MAX tahliyesi** (4096, en eski önce) | queue_losses içi 3502-3510 + prob emekliliği 5857-5863 / `make_tombstone_room` 6277-6288 (çağrı 3837, 6703); sabit 1553 / 1636 | **Evet (kod yolu var); ürünün düşük-bant rejiminde erişilemez.** Bir TTL içinde ≥ 4096 tombstone girişi gerekir (≥ ≈ 163 kayıp/prob olayı/s) — yalnız yüksek hız + yoğun kayıpta | Satır 1 ile aynı (Expired/Unknown; 05b teyit yok) | Satır 1 ile aynı | **Var** | Satır 1 ile aynı: veri kaybı yok; israf + sızıntı | Satır 1 ile aynı. Test: `late_ack_after_cap_keeps_payload` |
| 3 | **Duplicate kökenli kayıp** (kayıt tombstone bırakmadan düşer) | queue_losses Dup dalı 3438-3455 (yalnız kopya yeniden kurma) / 3764-3792 (`own_record_gone` + geometrik sahip) | **Evet.** Kopya kayıtları her sürümde var: PRE_CONVERGE §3.3-B ve kuruluş probu, RecoverFwd tek seferlik kopyalar, ARQ_RETX_CAP sonrası devir, kopya kaybı sonrası yeniden kurma. Kopya alıcıya ulaşıp ACK'i kaybolur/gecikirse kopya kayıp ilan edilir ve kaydı silinir. main 3441-3442'deki yorum ("kopya kaybolduysa birim HÂLÂ teslim edilmemiştir") tam bu durumu dışarıda bırakır | Kayıt yok → Unknown; un-loss yalnız `pending_lost`'a bakar, kopya oraya girmez. main: eşleşme yok; 05b: teyit YOK | `store.remove` + `acked()` | **Var** | Satır 1 ile aynı: veri kaybı yok (kopya ulaşmış); israf + sızıntı. main'de motor kopya zincirini yeniden kurduğu için gövdesiz kopyalar sürer | Satır 1 ile aynı. Test: `late_ack_after_dup_loss_keeps_payload` |
| 3b | **Duplicate kökenli kaydın kuyruk-probu emekliliği** (tombstone yok) | `retire_inflight_to_tombstone` Dup dalı 5849-5853 / 6694-6700 | **Evet** (tail-recovery `validated_v1` ürün varsayılanı açık) | Satır 3 ile aynı | Satır 3 ile aynı | **Var** | Satır 3 ile aynı | Satır 3 ile aynı; ayrı test yok (aynı "kayıt yok → teyit yok" kolu; tetik farkı: kayıp ilanı yerine prob) |
| 4 | **K_MAP budaması** (birim başına ≤ 8 kayıt, en eski budanır) | main'de **yok** / `own_on_sent` 6312-6322 + `prune_unit_record` 6331-6344; `K_MAP` 1644 | **Hayır** (v1'de yok). `7f81f184`'te budama ETKİNDİ (DUR-30 ile çelişki; DUR adayı olarak bildirildi) — motor ucunda `4fec0556` ile KAPATILDI (aday tabanı `730b69c1`); DUR adayı kapandı | 05b: Expired/Unknown, teyit YOK | (05b tabanı + uyarlamasız runtime) `store.remove` + `acked()` | **Var** (`7f81f184` + uyarlamasız runtime) | Satır 1 ile aynı | Uyarlamayla tutarlı: budanmış kaydın geç ACK'i payload'ı silmez. 05c'de budama açılınca ek runtime işi gerekmez (birim başına O(1) runtime kaydı 05c'nin) |
| 5 | **FEC parite ACK'i — ESI ad çakışması** (ters yön) | main 3075-3083: parite kaydı `unit.id = esi` → `acked_units.push(esi)` → 3209-3211 **esi kimlikli KAYNAK birimin kopyaları iptal** / 05b 3349-3351: sahip `FecParity`, birim yok → hiçbir şey | **Evet** (FEC ürün profilinde açık, 0x05) | main: kopya iptali (motorun "teyit" eylemi) yanlış birime; 05b: teyit yok, kopya iptali yok | Parite seq'i `seq_to_unit`'e girmez (`carries_unit` 1993-1996 / 1997-2000) → payload kalır, olay yok | **Var (main, ters yön):** motor birimin kopyalarını bırakır, runtime payload'ı tutar | **Sahipsizlik riski:** PRE_CONVERGE'de ARQ ve PTO kapalı, tek kurtarma kopyadır → esi birimi kuruluşa dek hiç yeniden gönderilmez; parite ACK'i RTT örneği vermediği için kuruluş da gelmeyebilir (veri ulaşmaz). Kuruluştan sonra ARQ telafi eder | 05b motoru + uyarlama: iki uç da teyit etmez; kopyalar sürer. Taban-kırmızı kanıtı MOTOR düzeyindedir: `k4g_p0_engine.rs` `esi_inflight_arm` / `esi_tombstone_arm`. Runtime düzeneği (`parity_ack_keeps_payload_and_copies_of_esi_namesake`) main'de bu ayrışmayı YENİDEN ÜRETMEDİ (run1: tabanda yeşil); test bekçi olarak kalır |
| 6 | **Parite kaydının süre dolumu** (05b madde 1: ilansız parite K_MAP × RTO_MAX sonra Expired) | yok / Pto 3588-3597, `expire_parity_record` 6257-6271 | Hayır (v1'de parite kaydı tombstone TTL'iyle ya da ACK'le kapanır) | Parite birim değil: teyit yok | Parite haritada yok: silme yok | Yok | — | Değişmez |
| 7 | **Teyit sonrası kalan kayıt** (motor unutmaz, runtime unutur) | main: teyit kümesi yok — birimin bir kaydı ACK'lenince diğer kaydı sonradan kayıp ilan edilirse `pending_lost`'a girer (queue_losses 3419-3528, `pending_lost.push_back` 3493; teyit kontrolü yok), prob da hedefleyebilir / 05b: `confirmed` kontrolü 3763, 3827; prob süzgeci `probe_eligible_owner` | **Evet** (ör. orijinalin geç ACK'i tombstone'a düşerken retransmit uçuşta ve kaybolur; ya da RECOVERED sonrası uçuştaki kayıt) | main: kalan kayıt ARQ/prob'a girer; 05b: birim teyitli, karar üretilmez | İlk ACK'te `store.remove` (doğru: teslim kanıtlı) | **Var (main)** | Veri kaybı yok; israf: main'de gövdesiz yeniden gönderim (`stale_decisions`) + sızıntı. 05b motoru bunu kendi tarafında kapatır | Uyarlama + 05b: store-miss yok. Bekçi: `store_miss_guard_*` |
| 8 | **FEC RECOVERED** | main 5212-5271: `pending_lost`/`dup_queue` iptali, uçuştaki kayıt kalır (satır 7'ye düşer) / 05b 5576-5641: aynı + `confirm_units` 5640 | **Evet** | main: kısmi iptal; 05b: teyit (motorun kaydını tuttuğu birim için) | `store.remove` + `acked()` motordan ÖNCE | main: satır 7 sınıfı ayrışma; 05b: yok | main: israf (gövdesiz ARQ) | Uyarlama: payload `RecoveredControl` sonrası drenajdan düşer; `fec_recovered_ctrl_units` drenajda sayılır. Bekçi: `store_miss_guard_fec_recovered` |
| 8b | **RECOVERED, motorun hiç göndermediği birim** | 05b `confirm_units`: dizinde girdisi olmayan birim çıktı üretmez (6601-6609) | **Erişilemez**: `schedule_transmit` sırası kaynak → parite (4632 → 4654) ve parite boyu ≥ kaynak boyu; blok kapanışı K kaynağın sunumundan sonra | — | — | — | — | Erişilirse tutarlı: motor teyit etmez, payload kalır, kaynak gövdeli gider |
| 9 | Runtime **PN_SEQ_WINDOW** budaması (65 536 PN) | yok (runtime) — `runtime.rs` 78, `map_pn!` 1973-1984 / 1977-1988 | Evet (65 536 PN geride kalan ACK) | Çevrilmez → motora gitmez | Çevrilmez → silme yok | Yok (iki uç da yok sayar) | — | Değişmez |
| 10 | Runtime **store sınırı** (`max_inflight_units` 4096) | yok — teklif kapısı 2430 / 2434 | Evet | — | Silme yok: doluyken teklif alınmaz | Yok | — | Değişmez |
| 11 | **Oturum kapanışı** (shutdown, `drain_cap`) | yok — boşaltma 2647-2664 / 2651-2668, `undelivered` 2665 / 2669 | Evet | Motor düşer | Store düşer, `undelivered` sayılır | Yok (birlikte) | Payload terk edilir, sayılır (app-düzeyi konu) | Değişmez |
| 12 | Runtime **repair_store** tahliyesi (64 blok) | yok — 2534-2539 / 2538-2543; eksik → `fec_repair_missing` | Evet | — | Parite gövdesi; birim payload'ı değil | Kapsam dışı | — | Değişmez |
| 13 | **Bench / harness kopyaları** (ürün dışı) | `crates/bench/src/kama.rs` 90-217: payload deposu YOK (sabit gövde); ilk ACK = teslim damgası; `udp_runner.rs`, `fs_harness.rs`, `examples/*`: payload deposu yok; motor test aynaları `tests/k4g05b_support`, `tests/k4g_p0_engine.rs` | — | — | Payload sahipliği yok; bench'in "teslim" damgası geç ACK'te de doğru (alıcı almıştır) | Payload ayrışması yok | Ölçüm semantiği: bench, motorun tanımadığı geç ACK'i teslim sayar (doğru) | Değişmez (ürün yolu `kama_product.rs` → `client-core` runtime'ı kullanır, uyarlamayı alır) |

## Ayrışma özeti

- **Ayrışma var, v1'de bugün etkin:** 1 (TTL), 2 (tahliye), 3/3b (kopya kaydı), 5 (ESI, ters yön), 7–8 (teyit
  sonrası kalan kayıt; motor tarafı 05b'de kapandı). → CEVAP 13 §3 gereği uyarlama **05b'de**.
- **Ayrışma vardı, yalnız 05b tabanında:** 4 (K_MAP; `7f81f184`'te etkindi, aday tabanında `4fec0556` ile kapalı — 05c'de uyarlanmış runtime'la açılır).
- **Ayrışma yok:** 6, 8b (erişilemez), 9, 10, 11, 12, 13.
- **Veri kaybı:** geç-ACK satırlarında (1–4) YOK — ACK, alıcının o datagramı aldığını kanıtlar; kayıp israf
  (gövdesiz yeniden gönderim) ve harita sızıntısıdır. Satır 5'te (main) PRE_CONVERGE'de **ulaşmama riski**
  var (kopyaları iptal edilen birimin kuruluşa dek kurtarma yolu yok) — akıl yürütme; motor P0 testleri
  ayrışmayı gösterir, runtime düzeneği main'de ulaşmamayı ÜRETMEDİ (aşağıda "Testler").

## Testler — ölçülmüş sonuçlar (run1, `/home/ryuzaki/kama-k4g-work/k4g05b/runtime/run1/`)

`crates/client-core/tests/k4g05b_runtime_ownership.rs`. Taban kolu: main `1b1425d2` + taban kopyası
(`taban/k4g05b_runtime_ownership.rs`; yalnız iddia 5'in gövdesi boş). Aday: `k4g/05b-int` `db3b13bd`
(motor ucu `730b69c1`, K_MAP kapalı). Tahminlerim iki testte ölçümle TUTMADI; tablo ölçülmüş hâldir.

| Test | Envanter satırı | Taban (main) — ölçüm | Önceki tahminim | Aday `db3b13bd` — ölçüm |
|---|---|---|---|---|
| `late_ack_after_ttl_keeps_payload` | 1 | **Kırmızı** — iddia (1): geç ACK sonrası `acked()` olayları `[4059, 1, 4060]` (hedef 1) | kırmızı (1) | Yeşil |
| `late_ack_after_cap_keeps_payload` | 2 | **Kırmızı** — iddia (1): `[1, 4208]` | kırmızı (1) | **Kırmızı (fikstür)** — geç ACK öncesi trafik boşaltması 10 s'de olmadı (`outstanding=231`); 300 s bütçeyle teşhis koşusunda adım 6,2 s, test yeşil. Tahliye edilen kayıtların geç ACK'i teyit etmez, ek yeniden gönderim turu gerekir (11-EK §4'ün beklenen bedeli). Bütçe yol başına yapıldı (`20f1df53`: cap → `CAP_WAIT`); düzeltme sonrası KOŞULMADI |
| `late_ack_after_dup_loss_keeps_payload` | 3 | **Kırmızı** — iddia (1): `[0, 44]` (hedef 0) | kırmızı (1) | Yeşil |
| `parity_ack_keeps_payload_and_copies_of_esi_namesake` | 5 | **Yeşil** | ~~kırmızı (faz 2)~~ — TUTMADI | Yeşil |
| `store_miss_guard_loss_and_reorder` | 7 | **Yeşil** | ~~büyük olasılıkla kırmızı~~ — TUTMADI | Yeşil |
| `store_miss_guard_fec_recovered` | 8 | **Kırmızı** — `stale_decisions = 3` | kırmızı | Yeşil |

Taban: 4 kırmızı / 2 yeşil. Aday: 5 yeşil / 1 kırmızı (fikstür bütçesi).

- **ESI ters yönü (satır 5):** main'deki ayrışmanın taban-kırmızı kanıtı motor düzeyindeki P0 testleridir
  (`crates/adaptive-transport/tests/k4g_p0_engine.rs` `esi_inflight_arm` / `esi_tombstone_arm`). Runtime
  düzeneği main'de bu ayrışmayı yeniden üretmedi; nedeni bu turda incelenmedi. Runtime testi taban-kırmızı
  kanıtı DEĞİLDİR; bekçi olarak kalır (05b + uyarlamada parite ACK'i payload'ı silmez, teyit üretmez, esi
  adaşı birimler hizmet görür).
- **Kayıp + yeniden sıralama bekçisi (satır 7):** main'de bu kanalda store-miss oluşmadı (tek koşu, tohum
  sabit, zamanlama değişken). Satır 7'nin main'deki kanıtı bu koşuda RECOVERED bekçisidir (satır 8 →
  satır 7 sınıfı). Test bekçi olarak kalır.

## Uyarlamanın yeri

`crates/client-core/src/runtime.rs`, `client_task`:
- `drive!`: her `session.on_event` dönüşünde `take_confirmed_units()` drenajı — payload silme, `unit_acked!`,
  `outstanding` düşümü, `stats.acked` / `stats.fec_recovered_ctrl_units` ve `unit_seqs`/`seq_to_unit`
  temizliği YALNIZ burada; karar işlemeden ÖNCE (teyitli birime aynı olayda karar gelirse store-miss sayılır).
- Karar döngüsü: birim store'da yoksa (teyitli) `seq_to_unit`/`unit_seqs`'e kayıt eklenmez.
- ACK yolu ve RECOVERED yolundaki doğrudan `store.remove` kalkar; `RecoveredControl` motora aynen gider.
- Teşhis: `SenderRecoveryDiag` iki salt-gözlem alanı (`seq_to_unit_len`, `unit_seqs_len`) — sızıntı bekçisi
  (CEVAP 10 DUR-23 (b) "runtime tablo sayacı"; yalnız `live_diag` açıkken).
