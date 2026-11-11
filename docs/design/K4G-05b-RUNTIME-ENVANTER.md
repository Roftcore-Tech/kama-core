# K4G-05b-R — Kayıt-unutma envanteri: aynı geç ACK'te motor ve runtime

Tarih 2026-11-11. Dal `k4g/05b-r-runtime-sahiplik`. Taban: main `4a9e97f1` (CC-PORT #170 dahil).
Kaynak: #140 (`49864510`) içindeki aynı adlı envanterin bu PR'a uyarlanmış hâli.

**main birleştirmesi (2026-11-11).** Dala main `399b573f` (#173, #175, #176), ardından `1cfd4da9`
(#178 INV-RATE-1 2026-11-11 metni, #179 CC-PORT U9) birleştirildi (rebase yok, metin çatışması
yok). #176'dan beri ürün varsayılan motoru `TransportEngine::CcPort`'tur
(`RuntimeConfig::default()`); bu PR'ın runtime değişikliği motordan bağımsızdır ve iki motorda
da koşar. Aşağıdaki satır atıfları birleştirme ÖNCESİ dal içindir.

**Yetki.** Sahip kararı 2026-11-11: "#140 (K4G-05b) BEKLER. CC-PORT motorun retx/token/HOL
politikasını değiştirdi; #140'ın motor kısmı Aralık'ta KARAR-KAYDI ile kapanır. İçindeki runtime
payload sahipliği (client-core) kısmını ayrı küçük PR olarak çıkar." Bu karar henüz
`docs/decisions/` altında kayıtlı DEĞİLDİR (KARAR-KAYDI'na girmesi bekleniyor).

**Kapsam.** #140'ın motor kısmı (birim sahiplik dizini `UnitOwnership`, teyit kümesi, madde 1–4:
parite kaydı sahibi, teyitli birime karar yok, RTO_MAX tavanı, sahiplenme adayları, K_MAP) bu PR'da
YOKTUR. Motor KARAR-NÖTR bir teyit çıktısı verir; runtime payload'ı ve app teyidini yalnız o
çıktıdan alır. **Ek (sahip kararı 2026-11-11, gece yetkisi §2.4):** motor teyit olayında birimin
bekleyen işini (kayıp kuyruğu, kopya kuyruğu) iptal eder, uçuştaki diğer kopyalarını kararsız kılar
(uçuşta kalırlar, muhasebeleri aynı, karar doğurmazlar) — teyitli birime karar üretmez (#140 madde
2'nin amacı, `UnitOwnership` modeli OLMADAN). Bu motor değişikliği KARAR-NÖTR DEĞİLDİR; değişen
kararların tam listesi "Motor: teyitli birime karar yok" bölümündedir.

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
emekliye ayırdığı kopya kaydı, sessiz kanal sınırında düşmüş kararsız kopya, bilinmeyen seq); kaydı
çoktan düşmüş seq'in yinelenen ACK'i; zamanlayıcılar.

**Karar-nötrlük (drenaj).** Teyit çıktısı kodu yalnız iki çıktı alanına yazar (`confirmed_units_out`,
`confirmed_units_overflow`); hiçbir karar yolu onları okumaz (çağrı girişindeki budama da yalnız
bu iki alana dokunur): drenaj yapan ve yapmayan motor aynı kararı verir. Teyit OLAYI ise artık
motorda birimin işini iptal eder (aşağıda "Motor: teyitli birime karar yok"; `acked_units` kalktı,
yerine aynı noktada `cancel_confirmed_units`). Drenaj nötrlüğünün pinleri: `sender.rs`
`tests::k4g05br_teyit` (iki karar-nötrlük testi ve satır-7 ikiz testi). Motor iptalinden ÖNCE
(`2c77970a`'ya dek) `tv_*`, `sim_determinism`, `cc_port_sim`, `sender_diag_contract`, `e1_goster`,
`e2a_goster`, kama-engine `n8_v2` ve client-core `ctx_first_dispatch` değişmeden yeşildi; motor
iptaliyle `e2a_goster` çıplak iz tabanı yeniden kaydedildi ve `cc_port_sim` satırlarının bir kısmı
değişti (aşağıda "Pinlenmiş beklentilerde değişenler" ve "`cc_port_sim`").

## Motor: teyitli birime karar yok (sahip kararı 2026-11-11)

Karar metni (gece yetkisi 2026-11-11 §2.4 ve öncesi): "geçmezse kökü motorda kapat (teyitli birimin
bekleyen retx/prob kayıtlarını ACK anında iptal), iki kurucuyla test, yeniden koş, MERGE." ve
"teyit edilen birimin bekleyen yeniden iletim / kopya kayıtları ACK anında iptal edilmeli (dup_queue
için acked_units iptali zaten var; pending_lost ve tail probe için aynı iptal). İki kurucuda da test.
Bu motor değişikliği karar-nötr değil ama 'teyitli birime karar üretme' #140'ın asıl amacı."

**Bu motor değişikliği KARAR-NÖTR DEĞİLDİR (sahip kabulü).** Teyit çıktısının drenajı
(`take_confirmed_units`) karar-nötr kalır (drenaj yapan ve yapmayan ikiz testleri değişmeden yeşil);
karar değişikliği teyit OLAYININ kendisindedir. "Önce" = dal `2c77970a` (motor iptali yok).

**Mekanizmanın iki sürümü.** İlk sürüm (`e01a6862`) teyitli birimin uçuştaki diğer kopyalarını teyit
anında uçuştan SİLİYORDU. İnceleme turu bunun karar yolu dışındaki girdileri de sildiğini gösterdi:
silinen kopyanın geç ACK'i teslim kredisi (BtlBw → pacing; port motorunda `cc.set_external_bw`) ve
CC `on_ack` vermiyordu, gerçek kaybı tıkanıklık olayı olmuyordu; ayrıca kader tahmincisi (FEC yapı
kapısının girdisi), ProbeUp kayıp atfı, reorder toleransı (`note_spurious`) ve kuyruk kurtarmasının
ilerleme sıfırlaması o kopyaları görmüyordu. Bu, `CC-PORT-ESLEME.md` (b) ile çelişiyordu ve ilk
sürümün "quinn discard semantiği" gerekçesi yanlıştı: quinn-proto 0.11 verisi başka paketle
onaylanmış paketi `sent_packets`'ta tutar, ACK'te `on_packet_acked` → `congestion.on_ack`, kayıpta
`detect_lost_packets` → `on_congestion_event` işler; sinyalsiz düşürme yalnız `discard_space`,
Retry ve 0-RTT reddinde vardır. İnceleme ölçümü (sim, cc_port kesinti tohum 1–5, ilk sürüm): koşu
başına 9–14 kopya teyitte silindi, bunların 6–10'u sonradan ACK'lendi (7.2–12.0 kB, ACK'lenen
baytın %4.6–7.5'i; BBR bu ACK'leri görmedi), 1–7'si yolda gerçekten kayboldu (tıkanıklık olayı
yok). İkinci sürüm (bu metin) kopyaları uçuşta tutar ve yalnız KARAR kollarını kapatır.

**Mekanizma** (`sender.rs` `Sender::cancel_confirmed_units`). Teyit olayları teyit çıktısını
besleyenlerle aynıdır: kökeni `FecParity` olmayan canlı kaydın ACK'i (uçuş, tombstone, un-loss) ve
`RecoveredControl`. Olay anında, artan sıralı tekil birim listesiyle:

- `pending_lost` kayıtları silinir (ARQ yeniden gönderimi, RecoverFwd devri ve ARQ tavanı devri
  buradan beslenir);
- `dup_queue` kayıtları silinir (önceki `acked_units` iptalinin yerine, aynı noktada);
- uçuştaki DİĞER kopyalar (köken `FecParity` değil) **kararsız işaretlenir**
  (`InflightPacket::confirmed_release_ns`) ve uçuşta kalır. Kopya gerçekten tele çıkmıştır (runtime
  payload'ı yalnız teyitte siler). ACK'i ve kayıp ilanı tabandaki muhasebeyi AYNEN besler: ACK'te BW
  örneği ya da teslim kredisi, CC `on_ack`, RTT kuralları, kader düzeltmesi; kayıpta uçuş baytı, CC
  kayıp baytı (`on_congestion_event`), kader kanıtı, ProbeUp atfı, tombstone (geç ACK'i kol-1/kol-2,
  spurious → reorder). Spurious kanıtı kol-2'de dalın kendisinde, kol-1'de (kayıp ilan edilmiş
  `Data` kökenli kayıt, tek gönderim) un-loss adımındadır ve `pending_lost` kaydı olmadan da işler
  (D12; ilk düzeltme turunda kararsız `Data` kaydının ve teyitte iptal edilen kaydın geç ACK'i reorder'a
  girmiyordu — inceleme bulgusu). Kapalı olan yalnız karar kollarıdır: `queue_losses`'ta `pending_lost` ve
  Duplicate kökenli kayıpta geometrik kopya; `plan_forward_duplication` canlılık kopyası; kuyruk
  probu hedefi (`oldest_probe_target`); jeton bekleyen prob ve E2a ikinci prob emisyonda yeniden
  doğrulanır (kararsızsa iptal).
- **Sessiz kanal sınırı** (`release_confirmed_copies`, Pto'da zaman eşiği kayıp tespiti ve tombstone
  TTL budamasından sonra; yeni zamanlayıcı değil, bırakma anı EK4-A2 uyanma kümesindedir — D14).
  Teyitten `K_CONFIRMED_RELEASE × T_loss` (= `K_TOMB × T_loss`, tombstone TTL saati) sonra hâlâ uçuşta
  olan kararsız kopya kayıp İLAN EDİLMEDEN düşer: uçuş baytı ve `diag_pif` bırakılır; tıkanıklık olayı,
  kayıp sayacı ve tombstone yok (sessizlik kayıp değildir — TUR-1a; sonuç TTL'ini doldurmuş tombstone
  ile aynı). Kapsam: (1) kopyadan daha yeni ACK'lenmiş seq yok (sessiz kanal); (2) PreConverge
  (§3.3-C zaman eşiği yasağı); (3) dar pencere: daha yeni seq ACK'lenmiş ama paket eşiği dolmamış
  (`la − seq` < reorder eşiği) ve zaman eşiği teyitten bu yana en az `K_CONFIRMED_RELEASE` kat
  büyümüş (kopya teyitten önce gönderildiği için bırakma anında geçen süre en az
  `K_CONFIRMED_RELEASE × T_loss(teyit anı)`'dır; eşik bu kadar ancak kuyruk birikimiyle, port
  motorunda tek bir geç `latest_rtt` örneğiyle ya da ACK aralığının büyümesiyle büyür). (3)'te kopya
  gerçekten kaybolmuşsa tıkanıklık olayı oluşmaz; dedektör seq'i sonra yine ilan edebilir (yalnız
  kayıp metriği). Bu kapsamların dışında dedektör kopyayı bırakma anından önce ilan eder. quinn'den
  beyanlı sapma (`CC-PORT-ESLEME.md` (b)): quinn'de PTO probu (PING) sonraki ACK'le kaybı ilan
  ettirirdi; burada teyitli birime prob kararı üretilmez.
- **RECOVERED** aynı ortak iptali kullanır (`pending_lost`/`dup_queue` sayaçları artık RECOVERED'da
  da yazılır) ve gönderim kuyruğunda bekleyen, hiç gönderilmemiş birimi kuyruktan çıkarır (envanter
  satır 8b; `experimental-fec-budget` bütçe kapısı veriyi bekletirken parite kendi bloğunun verisini
  geçebilir — inceleme bulgusu). N8: RECOVERED uçuş baytına, `bw_sampler`'a, BtlBw'ye ve pacer'a
  dokunmaz (ilk sürüm `bw_sampler` uçuş sayısını düşürüyordu).
- Kuyruk kurtarmasının "ACK ilerlemesi" kuralı tabanla birebir aynıdır (teyitte uçuştan kayıt
  düşmediği için ilk sürümdeki düzeltme kalktı).
- Sınır (SR-AUDIT-001): teyit KÜMESİ tutulmaz; işaret uçuş kaydının içindedir ve kayıtla düşer.
  Yeni iletim yalnız `tx_queue`, `pending_lost`, `dup_queue` ve uçuş kaydından (prob) doğar; ACK ile
  teyitli birim gönderilmiştir, RECOVERED'ın gönderilmemiş birimi kuyruktan çıkar. Kalıcı ek
  durum: kayıt başına `Option<u64>`, tarama kapısı `confirmed_release_next_ns` ve doygun gözlem
  sayaçları `ConfirmedCancelDiag {pending_lost, dup_queue, tx_queue, inflight_marked,
  lost_not_requeued, released}` (`Sender::diag_confirmed_cancels`; karar okumaz). Uçuş haritası
  teyit olayında bir kez taranır (işaretleme) ve bırakma anı geldiğinde (geçici liste ≤ uçuş
  haritası). Kopya kuyruğunun hayalet birim sayısıyla büyümesi kapanır: teyitli birime kopya
  kaydı kurulmaz.
- Teyit çıktısı: kararsız kopyanın ACK'i canlı kaydın ACK'idir ve teyidi yineler (tabandaki gibi;
  tüketici store varlığıyla tekilleştirir).

Aynı seride, aynı yoldaki üç inceleme bulgusu (ayrı commit'ler):

1. **ESI ad-uzayı sızıntısı.** Parite kaydının ACK'i (uçuş ve tombstone dalı) `acked_units`'e ESI'yi
   itiyordu; ESI ile aynı sayısal kimlikli KAYNAK birimin kopyaları iptal oluyordu. Parite artık kopya
   iptaline girmez (envanter satır 5'in "ters yönü" kapandı).
2. **`emit_tail_probe` `retx_count + 1` (u8).** Port motorunun yeniden iletim tavanı 255 olduğundan
   255'teki kayıt prob hedefi olabilir: debug'da taşma paniği, release'de 0'a sarma (0, kopyanın geç
   ACK'ini kol-1 örneği yapardı). Artık doygun.
3. **Kuyruk probu parite kaydını hedefliyordu** (hedef en eski uçuş kaydı; prob onu `unit_id = ESI`
   taşıyan `Retransmit` olarak yeniden gönderirdi). Hedef artık en eski teyitsiz KAYNAK birim
   kaydıdır (`oldest_probe_target`; E2a ikinci prob yuvası dahil). Uygun kayıt yoksa prob yoktur
   (kuyruk kurtarması söner).

### Değişen kararlar (tam liste; taban `2c77970a`)

| # | Ne | Ne zaman | Önce | Şimdi |
|---|---|---|---|---|
| D1 | Teyitli birime ARQ `Retransmit` | Birimin `pending_lost` kaydı varken başka bir kaydı ACK'lenir (satır 7) | Kayıt ARQ'ya girer, karar üretilir (port motorunda tavan 255) | Kayıt teyitte silinir, karar yok |
| D2 | Teyitli birime RecoverFwd ileri kopyası / ARQ tavanı devri | Aynı (kopyalar `pending_lost`'tan doğar) | Üretilir | Yok |
| D3 | Kararsız kopyanın kayıp ilanından doğan kararlar | Birimin bir kaydı teyit eder, diğeri uçuştadır (kopya ACK'lenir, orijinal uçuşta; geç orijinal teyit eder, yeniden gönderim uçuşta; RECOVERED, orijinal uçuşta) ve sonra kayıp ilan edilir | `pending_lost`'a girer (→ D1/D2 kararları); Duplicate kökenlide geometrik kopya yeniden kurulur | Kayıp muhasebesi aynı (uçuş baytı, port motorunda tıkanıklık olayı, tombstone, kader kanıtı); yeniden kuyruk ve geometrik kopya yok |
| D4 | Saf-sessizlik canlılık kopyası (RecoverFwd, `pending_lost` boş) | Kararsız kopya `Data` kökenli | Kopya kurulur | Kurulmaz |
| D5 | Kuyruk probu hedefi | Kararsız kopya en eski uçuş kaydı | Prob onu `Retransmit` olarak gönderir | Atlanır; en eski teyitsiz kaynak kayıt hedeflenir, yoksa prob yok (kuyruk kurtarması söner). Jeton bekleyen prob ve E2a ikinci prob emisyonda iptal |
| D6 | Sessiz kanal sınırı | Kararsız kopyanın ACK'i ve kayıp ilanı teyitten `K_TOMB × T_loss` içinde gelmez (kapsam yukarıda: sessiz kanal, PreConverge, dar pencere) | Kopya uçuşta kalır (ve D3–D5'in kaynağıdır) | Kayıp ilanı olmadan uçuştan düşer: uçuş baytı ve `diag_pif` bırakılır, tıkanıklık olayı yok; sonrasında `Idle` dönüşü, kapanış drenajı ve port motorunda `cc_fits` kopyayı saymaz |
| D7 | RECOVERED, gönderilmemiş birim (satır 8b) | Birim gönderim kuyruğunda | DATA olarak gönderilir (runtime'da gövdesiz → atlanır, kaydı ACK'lenmez) | Gönderim kuyruğundan çıkar; DATA kararı yok |
| D8 | Dolaylı etkiler | D1–D7'de kalkan emisyonlar | Emisyonlar ve kayıpları vardı | Yok: uçuş baytı, port motorunda tıkanıklık olayları ve pencere, kayıp sayaçları, EAI / `ack_confidence`, pacer jetonları ve sonraki seq numaraları buna göre değişir (aşağıdaki sim ölçümleri). Teyit iş kuyruklarını boşalttığında çerçeve sonu `idle_now` (CC uygulama-sınırlı) ve `update_state`'in `Idle` denetimi (iki motorda ortak) daha erken doğru olabilir |
| D9 | Parite ACK'i, ESI adaşı kaynak birimin kopyaları | FEC açık, kaynak birim kimliği = ESI | Kopyalar iptal | Kopyalar sürer |
| D10 | Kuyruk probu hedefi parite | En eski uçuş kaydı parite | Parite `Retransmit` (`unit_id = ESI`) olarak yeniden gönderilir | En eski teyitsiz kaynak kayıt; yalnız parite uçuştaysa prob yok |
| D11 | Prob kopyasının `retx_count`'u | Hedefin sayacı 255 | Debug panik / release 0 | 255 |
| D12 | Kayıp ilan edilmiş orijinalin (kol-1 tombstone, `Data`) geç ACK'i, birimin `pending_lost` kaydı YOKKEN | (a) kararsız `Data` kaydı kayıp ilan edilip yeniden kuyruğa girmedi; (b) kaydı başka kaydın ACK'iyle teyitte iptal edildi; (c) kaydı RECOVERED'la iptal edildi | (a)/(b) tabanda kayıt kuyruktaydı: geç ACK un-loss eşleşmesiyle ya da (yeniden gönderim önce çıktıysa) kol-2'de reorder'a girerdi; (c) tabanda da iptaldi, reorder'a GİRMİYORDU | Üçünde de `note_spurious(la − seq)` (un-loss adımında; kaydı eşleşen seq için aynı değer). (a)/(b) tabanın öğrenmesini geri getirir; (c) tabandan farklıdır (eski boşluk kapandı): reorder eşiği (`set_packet_threshold`) iki motorda sonraki kayıp ilanlarını etkiler |
| D13 | D12 (a)/(b)'deki geç ACK'in kolu | Taban o birime yeniden gönderimi (D1/D2, hayalet karar) geç ACK'ten önce çıkarırdı | Tombstone superseded → kol-2: yalnız teslim kredisi | Yeniden gönderim yok → tombstone kol-1 kalır: BW örneği (eski motorda ayrıca Karn RTT adayı; port motorunda RTT v1.1 kuralıyla iki kolda aynı). Geç ACK yeniden gönderimden önce gelseydi tabanda da kol-1'di |
| D14 | Uyanma kümesi (EK4-A2) | Uçuşta kararsız kopya var | Kararsız kopya yoktu; aynı kayıt kuyruk probu hedefi ya da canlılık kopyasının kaynağıydı ve uyanma onlardan gelirdi | Bırakma anı (`confirmed_release_next_ns`, alt sınır) `next_wakeup_ns` adayı; bayat değer en çok bir uyanma doğurur. Karar değildir: yalnız uyanma-güdümlü sürücüde Pto'nun zamanını belirler (ürün runtime'ı Pto'yu 1 ms'de bir pompalar) |

RECOVERED'da `pending_lost` ve `dup_queue` iptali tabanda da vardı (`on_recovered_control`,
`2c77970a`): D1/D2 RECOVERED'ı kapsamaz. RECOVERED'daki gerçek değişiklikler D3 (uçuştaki kayıt
kararsız), D7 (gönderilmemiş birim) ve D12 (c)'dir; iptal sayaçları artık ortak yoldan yazılır.

İlk sürümden farklı olarak DEĞİŞMEYENLER: teyit anında uçuş baytı ve uçuş sayısı; kopyanın ACK'i
(BW örneği, teslim kredisi, CC `on_ack`) ve kaybı (tıkanıklık olayı); kader tahmincisi (silinen
orijinal UNKNOWN kalmıyor), ProbeUp kayıp atfı, reorder toleransı (D12 (c) dışında), kuyruk
kurtarmasının ilerleme kuralı; PreConverge'de kuruluş kopyası birimi teyit edip orijinal kaybolduğunda eski motor
tabandaki gibi PreConverge'de bekler (`e2a_m8_b` taban beklentisine döndü) — yalnız D6 sınırında
orijinal düşer ve iş kalmadıysa motor `Idle`'a döner. Tel biçimi, golden vektörler, profil
kimlikleri ve `specs/` değişmedi.

### Hayalet ölçümü — önce ↔ sonra

`tests::k4g05br_teyit::k4g05br_ghost_after_skipped_stale_decision` (kurulum ve kanal modeli aşağıda
"Atlama politikası"nda; 120 s sahte saat, belirlenimli, tek koşu, `--nocapture`). Sonra sütunlarında
"atla" ve "başlık" kolları birebir aynıdır (teyitli birime karar olmadığından iki kol ayrışmaz).
İkinci sürümün çıktısı ilk sürümünkiyle (sayaç yapısı dışında) bayt düzeyinde aynıdır: satır-7
kurulumunda teyit anında birimin uçuşta kaydı yoktur (iptal: `pending_lost` 1, kopya 0, işaret 0).

| Motor | Kol | Teyitten sonra birime karar (önce → sonra) | Birimin bırakılması (bütün kayıtlar) | Motor `Idle`'a döner mi | Faz geçişi (120 s) | BBR (pencere B, tıkanıklık olayı) | Diğer birimler: teyit / ort. / en büyük gecikme |
|---|---|---|---|---|---|---|---|
| legacy + `validated_v1` (`TransportEngine::Legacy`) | atla | 14 (2 Retransmit + 12 Duplicate) → **0** | Hiç (kopya vadesi ≈ 173 s) → **26 s** | Hayır → **evet, teyitten 5 ms sonra** | 49 → 3549 | — | 2389 / 143 / 1400 ms → **2399 / 40 / 40 ms** |
| aynı | başlık | 1 → **0** | 26 s → 26 s | Evet → evet | 3551 → 3549 | — | 2399 / 40 / 40 → aynı |
| legacy (`Sender::new`) | atla | 14 → **0** | Hiç → **26 s** | Hayır → **evet** | 52 → 3549 | — | 2388 / 116 / 720 ms → **2399 / 40 / 40 ms** |
| aynı | başlık | 1 → **0** | 26 s → 26 s | Evet → evet | 3551 → 3549 | — | 2399 / 40 / 40 → aynı |
| cc_port (`Sender::with_cc_port`, ürün varsayılanı) | atla | 266 (254 Retransmit + 12 Duplicate) → **0** | Hiç → **1 s** | Hayır → **evet** | 550 → 3549 | (4800, 267) → **(254 600, 2)** | 2399 / 40 / 40 → aynı |
| aynı | başlık | 1 → **0** | 1 s → 1 s | Evet → evet | 3551 → 3549 | (254 700, 2) → (254 600, 2) | 2399 / 40 / 40 → aynı |

"Bırakma" birimin SON kaydının düştüğü ilk 1 s'lik örnektir: kalan kayıt yeniden gönderimin
tombstone'udur ve TTL'le düşer (eski motorda TTL tabanı cadence, ≈ 26 s; port motorunda RFC 9002
biçimi, ≈ 1 s). Başlık kolunda port motorunun penceresi 100 B küçüktür: önceki başlık-only hayalet
`Retransmit` (100 B) ACK'lenip pencereye ekleniyordu. İddia edilenler (test): teyitte kayıp kaydı
iptali = 1, teyitten sonra birime 0 karar, birim ufukta bırakılır, motor `Idle`'a döner, BBR iki
kolda aynı, diğer birimlerin gecikmesi iki kolda birim birim aynı. Önceki inceleme gözlemleri
("ghost records consume the tail-probe budget", "dup_queue grows without cap", "shutdown drain runs
to drain_cap because pif > 0") hayalet karar kalmadığı için bu yoldan oluşmaz; kapanış drenajında
`diag_pif`'i tombstone'lar (her kayıp gibi) TTL'e dek, kararsız kopyalar en çok D6 sınırına dek
tutar.

### Pinlenmiş beklentilerde değişenler (her biri gerekçeli)

- `tests/e2a_goster.rs` çıplak iz tabanı `0xa9f2_a6bc_da15_8482` → `0x9e4f_8cca_eac8_6803` (ürün
  benzeri iz `0x1e49_5f75_283c_9675` DEĞİŞMEDİ; iki sürümde aynı yeni değer). Karar dökümü (iki
  ağaç, ortam değişkeniyle geçici döküm; commit edilmedi): birim 0'ın orijinali (seq 0) kaybolur,
  kuruluş kopyası (seq 1) ulaşır ve birimi teyit eder; önceki motorda orijinalin kayıp ilanı birimi
  yeniden kuyruğa sokuyordu ve 10.635 s'de birim 0'a `Retransmit` (seq 5) gidiyordu, şimdi gitmez.
  Sonraki seq'ler kaydığından seq'e bağlı kayıp karması izin geri kalanını değiştirir (154 → 153
  karar). Eski değer dosyada yorum olarak duruyor.
- `tests/dr_cs1_acceptance.rs` DI-1, DI-1C, DI-2 (yalnız `--all-features`): sabit `acked_seqs: [1]` /
  `[1, 2]` u2/u3'ün değil u1'in PreConverge kuruluş kopyalarının seq'leriydi; o ACK u1'i teyit
  ettiğinden testler teyitli birime yeniden gönderim bekliyordu. Şimdi u2/u3'ün gerçek DATA seq'i
  çıktıdan alınır (DI-1A zaten böyleydi). DI-1C'de u1 gerçekten kayıp kalır; re-armed deadline'daki
  ikinci kurtarma emisyonu ≈ 60 s ACK sessizliğinden sonra RecoverFwd'da ileri kopyadır (Duplicate).
  İnceleme bulgusu üzerine test belgesi bunu söyler ve test ayrıca emisyonun o deadline'daki
  zaman-eşiği kayıp ilanından geldiğini iddia eder (`diag_loss` zaman sayacı artar); test adı
  geçmiş raporlarda atıflı olduğundan değişmedi. İddia edilen yaşam döngüsü (yeni ağ olayı olmadan
  expose edilen deadline'da emisyon, busy-loop yok, nihai ACK'ten sonra vadesi geçmiş uyanma yok)
  aynen sınanır.
- `tests/sender_state_matrix.rs`: ilk sürümdeki yürüyüş değişikliği (iki geçişli çağrının ara durumu
  çıkarımla kaydediliyordu — inceleme bulgusu: gözlenmemiş geçişleri gözlenmiş sayar, yasak
  PreConverge girişini gizleyebilirdi) GERİ ALINDI. İkinci sürümle dosya tabandaki hâliyle
  `--all-features` altında yeşil (7/7; ilk sürümdeki iki geçişli bileşke bu ağaçta oluşmuyor).
- `sender.rs` `e2a_m8_b_duplicate_origin_ack_exits_preconverge_only_when_on`: ilk sürümdeki değişiklik
  geri alındı, taban beklentisi (kapalı kolda `PreConverge`) yeşil.
- `k4g05br_decision_neutral_after_confirmation_with_pending_work` artık iptali iddia eder (önce
  gözlemdi); drenaj ikizi karşılaştırması aynen sürer.
- Runtime: FEC bekçisinin vakum ön koşulu (`stale_skipped ≥ 1`) ters çevrildi; her runtime testinde
  yeni iddia 7: `stale_decisions == 0`. Atlama dalları doğal trafikle artık koşmadığından dalın
  kendisi `runtime.rs` `skip_tests`'te sınanır (aşağıda "Atlama politikası").

### `cc_port_sim` — önce ↔ ilk sürüm ↔ bu sürüm

`cargo test -p adaptive-transport --test cc_port_sim -- --nocapture --test-threads=1`, aynı makine,
her ağaçta tek koşu (sim belirlenimli; ilk sürüm iki koşuda aynı çıktı verdi). 24 sonuç satırından
bu sürümde 9'u tabandan farklı (ilk sürümde 11):

| Hücre | Motor | Önce (`2c77970a`) | İlk sürüm (`cb80671b`) | Bu sürüm |
|---|---|---|---|---|
| kesinti 60 s, seed 1 / 2 / 3 / 4 / 5 — tamamlanma | cc_port | 95.88 / 87.90 / 85.13 / 84.80 / 80.73 s | 77.17 / 81.47 / 85.15 / 84.81 / 80.62 s | 95.88 / 81.47 / 85.13 / 84.80 / 80.73 s |
| aynı — retx | cc_port | 86 / 93 / 60 / 61 / 85 | 78 / 90 / 60 / 60 / 84 | 85 / 90 / 60 / 60 / 83 |
| aynı — bayt | cc_port | 253 200 / 261 600 / 222 000 / 223 200 / 252 000 | 243 600 / 258 000 / 222 000 / 222 000 / 250 800 | 252 000 / 258 000 / 222 000 / 222 000 / 249 600 |
| kesinti özet | cc_port | 5/5, p50 85.1 s | 5/5, p50 81.5 s | 5/5, p50 84.8 s |
| q6 l20, 10 seed | cc_port | 10/10, p50 15.89 s, en büyük 17.12 s | 10/10, p50 15.89 s, en büyük 17.28 s | 10/10, p50 15.89 s, en büyük 17.12 s |
| kesinti 60 s, seed 5 | legacy | tamamlandı 119.88 s, 125/125, retx 494 | tamamlanmadı (180 s), 124/125, retx 487 | tamamlanmadı (180 s), 124/125, retx 487 |
| kesinti özet | legacy | 2/5, p50 119.9 s | 1/5, p50 118.4 s | 1/5, p50 118.4 s |
| q1000 l20, 10 seed | legacy | 7/10, p50 17.37 s, en büyük 60.50 s | 6/10, p50 17.37 s, en büyük 54.31 s | 6/10, p50 17.37 s, en büyük 54.30 s |
| q6 l20, 10 seed | legacy | 7/10, p50 16.36 s, en büyük 59.98 s | 6/10, p50 16.36 s, en büyük 54.11 s | 6/10, p50 16.36 s, en büyük 54.14 s |

Değişmeyen satırlar (bu sürüm = taban): q6 l0 (iki motor), q1000 l20 cc_port (10/10, p50 15.71 s,
en büyük 17.34 s), PTT demeti (0.64 / 0.34 s), FEC parite hücresi (legacy 8/30, cc_port 30/30),
kesinti legacy seed 1–4, kesinti cc_port seed 3. Port kolu (bağlayıcı) bütün eşiklerini geçer (7/7
test yeşil).

Okuma sınırı: kanal modeli kayıp kararını her gönderimde LCG'den çeker; kalkan emisyonlar sonraki
çekimleri kaydırır, tohum başına eşli bir kıyas DEĞİLDİR. İlk sürümün kesinti p50'si (81.5 s) bu
sürümde 84.8 s'dir; ikisi arasındaki fark, silinen kopyaların CC muhasebesinin geri gelmesiyle
tutarlıdır ama ayrıca teşhis edilmedi ve bir iyileşme ya da gerileme iddiası yapılmaz. Tek tohum
teşhisi (ilk sürümde, q6 l20 legacy seed 10, iki ağaçta ortam değişkeniyle geçici karar dökümü;
commit edilmedi): önce 59.98 s'de tamamlanıyordu — birim 0 kuruluş kopyasıyla teyitliydi ama 39.94
s'de ona hayalet `Retransmit` gitti ve sonraki kayıp çekimleri bu emisyonla kaydı. Sonra: son emisyon
56.2 s'de birim 89'un `Retransmit`'i; ondan sonra hiçbir emisyon yok, 120 s'ye dek uçuşta 1200 B,
durum RecoverFwd. Kod okuması: sim'in eski kolu `Sender::new()` ile kuyruk kurtarması KAPALI koşar;
kaybolan son `Retransmit`'ten sonra seq gönderilmediği için kayıt kayıp ilan edilmez ve RecoverFwd'un
saf-sessizlik canlılık kolu (`plan_forward_duplication`) yalnız `Data` kökenli uçuş kayıtlarına kopya
kurar. Aynı takılma tabanda seed 5'te zaten vardır. Bu sürümde eski kolun satırları ilk sürümle aynı
kaldı (en büyük süreler 10–30 ms farklı); ayrıca teşhis edilmedi. Ürün geri alma yolu
(`TransportEngine::Legacy`) `validated_v1` kuyruk kurtarmasıyla koşar; sim kolu onu kurmaz. Bu
canlılık deliği bu PR'da düzeltilmedi (açık kalem).

### Runtime testleri (`k4g05b_runtime_ownership`, `-- --ignored`)

PR `test-runtime` komutu (`--workspace --all-features --exclude kama-integration --test
k4g05b_runtime_ownership -- --ignored --nocapture`), ikinci sürümün son ağacında tek koşu, yerel
loopback: **11/11 yeşil**, testler 120.6 s. Her testte gövdesiz datagram 0 ve `stale_skipped =
stale_decisions = fec_repair_missing = 0`. TTL ve tahliye fikstürleri (`Legacy`'ye sabit) ön
koşulları ve iddialarıyla geçti; aynı yollar port motorunda motor düzeyinde (`tests::k4g05br_teyit`,
iki motor) sınanır. Gözlem; kontrollü kıyas değil.

| Test | Motor | İstemci datagramı (gövdesiz) | `stale_skipped`: iptal öncesi → ilk sürüm → bu sürüm | Gözlem (bu sürüm) |
|---|---|---|---|---|
| `late_ack_after_ttl_keeps_payload` | Legacy | 6019 (0) | 0 → 0 → 0 | faz B 12.0 s |
| `late_ack_after_cap_keeps_payload` | Legacy | 8908 (0) | 68 → 0 → **0** | kayıt 20.2 s'de düştü (tablo 4096); faz B 0.17 s |
| `late_ack_after_dup_loss_keeps_payload` | Legacy | 209 (0) | 0 → 0 → 0 | faz B 0.87 s |
| `late_ack_after_dup_loss_keeps_payload_cc_port` | CcPort | 82 (0) | 0 → 0 → 0 | faz B 0.009 s |
| `parity_ack_no_confirmation_payload_kept` | Legacy | 18 (0) | 0 → 0 → 0 | faz 2 2.9 s |
| `parity_ack_no_confirmation_payload_kept_cc_port` | CcPort | 18 (0) | 8 → 0 → **0** | faz 2 0.17 s |
| `store_miss_guard_loss_and_reorder` | Legacy | 541 (0) | 0 → 0 → 0 | — |
| `store_miss_guard_loss_and_reorder_cc_port` | CcPort | 551 (0) | 12 → 0 → **0** | — |
| `store_miss_guard_fec_recovered` | Legacy | 15 (0) | 3 → 0 → **0** | teyitten sonra 10 s, saniyede bir: motor (uçuş, kayıp, kopya) = (2, 0, 0) |
| `store_miss_guard_fec_recovered_cc_port` | CcPort | 15 (0) | 16 → 0 → **0** | aynı, (0, 0, 0) |
| `store_miss_guard_fec_recovered_frag_cc_port` | CcPort (DATA_FRAG) | 15 (0) | 8 → 0 → **0** | aynı, (0, 0, 0) |

"İptal öncesi" sütunu yukarıdaki "main birleştirmesinden sonraki koşu" tablosudur (dal `d05cc48f`
kodu); "ilk sürüm" `cb80671b`'nin koşusudur. Koşular aynı makinede ama farklı derleme ve zamanda,
kontrollü kıyas değildir. FEC bekçisinde eski motorla teyitten sonra 10 s boyunca uçuşta 2 kayıt
kalır: kurtarılan birimlerin kararsız kopyalarıdır (karar doğurmazlar; yeni trafik olmadığından
kayıp ilan edilmezler ve eski motorda D6 sınırı ≈ 26 s, gözlem penceresinin dışında). Port
motorunda sınır ≈ 1 s olduğundan pencere içinde (0, 0, 0). İlk sürümde (kopyalar teyitte siliniyordu)
iki motorda da (0, 0, 0) idi; iptal öncesi 2–3 uçuş kaydı kalıyor ve yeniden karar doğuruyordu.

YAMA ADAYI (specs/ düzenlenmedi): `adaptive-transport-engine-spec` §3.3-B "stop koşulu" yalnız kopya
iptalini anar; teyit olayında `pending_lost` iptali, uçuştaki kopyaların kararsız işaretlenmesi
(muhasebe aynı, karar yok) ve sessiz kanal sınırı, RECOVERED'da gönderilmemiş birimin kuyruktan
çıkması ve kuyruk probunun onarım sembolünü ve kararsız kopyayı hedeflememesi için spec satırı
adayı. Kimlik sorusu (§9.5) aşağıda açıktır.


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
  kaynakta da karar tele ÇIKMAZ (aşağıda "Atlama politikası"). Motor iptalinden sonra birinci kaynak
  beklenmez; runtime testleri her testte `stale_decisions == 0` iddia eder (iddia 7).
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
| 5 | **FEC parite ACK'i — ESI ad çakışması** | Parite kaydı `unit.id = esi`; uçuş dalı 3284 `acked_units.push(esi)` → 3443-3445 esi kimlikli KAYNAK birimin kopyaları iptal | Evet (FEC ürün profilinde açık) | Teyit YOK (`FecParity` süzgeci) | Parite seq'i haritaya girmez (`carries_unit` 2021-2025) → payload kalır | Aynı; teyit çıktısı süzgeci olmasaydı runtime 0..R birimlerini silerdi | `parity_ack_no_confirmation_payload_kept` (faz 1); motor `k4g05br_parity_ack_never_confirms_esi_namesake`. Kopya iptali (ters yön) KAPANDI: parite ACK'i kopya iptaline girmez (motor `k4g05br_parity_ack_keeps_esi_namesake_copies`) |
| 6 | Parite kaydının süre dolumu | Tabanda ve bu PR'da YOK | — | — | — | — | **Yalnız #140** |
| 7 | **Teyit sonrası kalan kayıt** (ör. geç orijinal teyit eder, kaybolan yeniden gönderim `pending_lost`'ta) | Tabanda motor teyitli birimin `pending_lost` kaydını budamaz. Bu PR: teyitte kayıp ve kopya kaydı iptal, uçuş kopyaları kararsız (`cancel_confirmed_units`) | Evet | Kayıt iptal; karar YOK | İlk ACK'te `store.remove`; kalan kararlar başlık-only (`stale_decisions`) VE harita kaydı açar (budanmaz) | Karar yok (`stale_decisions == 0`, iddia 7); gelirse atlanır (tele datagram çıkmaz, harita kaydı açılmaz) | `store_miss_guard_loss_and_reorder` (+ `_cc_port`); motor `k4g05br_decision_neutral_after_confirmation_with_pending_work`, `k4g05br_confirmed_unit_*` |
| 8 | **FEC RECOVERED** | `on_recovered_control`: `pending_lost`/`dup_queue` iptali; tabanda uçuştaki kayıt kalır ve kaybında yeniden kuyruğa girer (satır 7'ye düşerdi). Bu PR: uçuştaki kayıtlar kararsız (uçuşta kalır, kaybında yeniden kuyruk yok; sessiz kanalda D6 sınırında düşer) | Evet | Listelenen her birim teyit edilir | `store.remove` + `acked()` motordan ÖNCE | Drenajda (motordan SONRA); store'da varsa düşer — sonuç tabanla aynı; teyitten sonra karar yok | `store_miss_guard_fec_recovered` (+ `_cc_port`, `_frag_cc_port`); motor `k4g05br_recovered_control_marks_inflight_copies` |
| 8b | RECOVERED, motorun hiç göndermediği birim | — | Varsayılan zamanlamada erişilemez (parite telde kendi bloğunun DATA'sından sonra planlanır); `experimental-fec-budget` bütçe kapısı veriyi bekletirken parite verinin önüne geçebilir (inceleme bulgusu) | Bu PR: yankılanır (#140: teyit etmez); gönderim kuyruğundaysa kuyruktan çıkar (D7), DATA kararı yok | Store'da varsa düşer | Aynı (tabanla) | motor `k4g05br_recovered_control_removes_unsent_unit` (iki motor); **#140 ayrımı** (teyit edip etmeme) açık |
| 9 | Runtime `PN_SEQ_WINDOW` (65 536 PN) | runtime 78 | Evet | Çevrilmez → motora gitmez | Silme yok | Değişmez | — |
| 10 | Runtime store sınırı (`max_inflight_units` 4096) | teklif kapısı runtime 2458 (dal 2563) | Evet | — | Doluyken teklif alınmaz | Değişmez; teyit kaybı bu sınırdan bağımsız (motor çağrının kendi teyitlerini düşürmez; parçalama açıkken store bu sınırı aşabilir, sonuç değişmez) | motor `k4g05br_confirmed_units_call_output_never_dropped`; runtime testlerinde `confirmed_units_overflow == 0` |
| 11 | Oturum kapanışı (`drain_cap`) | runtime 2675-2693 | Evet | Motor düşer | Store düşer, `undelivered` sayılır | Değişmez | — |
| 12 | Runtime `repair_store` tahliyesi (64 blok) | runtime 2565 | Evet | — | Parite gövdesi, birim payload'ı değil | Değişmez | — |
| 13 | Bench / harness (ürün dışı; `crates/bench/src/kama.rs`, sim, `fs_harness`, `udp_runner`) | — | — | Drenaj yapmazlar: birikim her çağrı girişinde tavana iner (≈ 32 KiB + son çağrının teyitleri / Sender), taşma sayacı büyür | Payload deposu yok | Değişmez; karar etkisi yok (nötrlük testi taşan ikizle) | — |

Satır 7 ve 8'de bu PR, tabandaki harita sızıntısını (kod okuması) kapatır, teyitli birime verilen
kararı tele çıkarmaz (atlar) ve — sahip kararıyla — kararın kendisini motorda kapatır (teyitte
bekleyen iş iptal; bütün runtime testlerinde `stale_decisions == 0`).

## Atlama politikası (sahip kararı 2026-11-11, #174 madde 2)

Karar metni: "gövdesiz gönderim DATAGRAM ÜRETMEMELİ — runtime o kararı atlamalı (sayaç:
stale_skipped), tele boş/eksik çerçeve çıkmamalı."

PR için tek cümle: **Gövdesi olmayan motor kararı (teyitli birime karar ya da repair-store'da
sembolü olmayan parite) artık datagram üretmez; runtime kararı atlar ve `stale_skipped`'te sayar;
bu kararlar için tele boş gövdeli DATA, yalnız önekli DATA_FRAG ya da boş sembollü FEC çıkmaz
(röle sayımıyla her runtime testinde 0).** Kapsam: motor kararı. Geçerli bir BOŞ uygulama mesajı
(`send` boş payload'ı kabul eder) tam başlık boyunda DATA ya da yalnız önekli DATA_FRAG olarak
çıkar; o bir atlama konusu değildir (testlerde boş mesaj gönderilmez, tel bekçisinin
sınıflandırması bu varsayıma dayanır).

| Yol (`runtime.rs` `drive!`) | Önceki (`36820b37`) | Bu PR | Neden sayacı |
|---|---|---|---|
| (a) Data / Retransmit / Duplicate, birim store'da yok (teyitli birim; satır 7, 8 → 7) | Başlık-only: boş gövdeli DATA; parçalama açıkken 11 B sıfır önekli DATA_FRAG | Atlanır (`continue`; seal yok, PN harcanmaz, harita kaydı yok) | `stale_decisions` |
| (b) FecParity, FEC açık, sembol repair-store'da yok (blok tahliyesi / latch-0) | Boş sembollü FEC çerçevesi | Atlanır | `fec_repair_missing` |
| (c) FecParity, FEC kapalı ya da gönderim kaydı yok | Zaten tele çıkmıyordu | Aynı | `stale_decisions` |

`stale_skipped` = (a) + (b) + (c) = `stale_decisions + fec_repair_missing` (tanım; her runtime testinde
iddia edilir). Kurtarma kapalıyken bastırılan karar ayrı sayaçtadır (`recovery_suppressed`). Sıfır-önek
gövdesi (`stale_body`) kaldırıldı. Tel biçimi, golden vektörler ve profil kimliği değişmedi.

**Ardıl (motorda) — İPTAL ÖNCESİ (dal `2c77970a`'ya dek; tarihsel kayıt).** Bu alt bölüm motor
iptali gelmeden önceki durumu ve ölçümlerini anlatır; motor artık teyitli birime karar üretmez
("Motor: teyitli birime karar yok", önce ↔ sonra tablosu orada). Motor KODU değişmedi; motorun kapalı-döngü karar akışı değişir. Motor kararı
işlemiştir: atlanan kararın kaydı uçuştadır ve ACK'i hiç gelmez. Önceki başlık-only emisyonda alıcı
seq'i ACK'liyor ve motor kaydı kapatıyordu. Atlama bu kapanışı kaldırır. Kaydın akıbeti (kod
okuması, `sender.rs` / `loss.rs` / `tail_recovery.rs`):

- Daha yeni bir seq ACK'lenirse kayıt kayıp ilan edilir (`seq < largest_acked` koşulu, `loss.rs`
  698) ve birim yeniden `pending_lost`'a girer: motor aynı birime yeni karar verir, o da atlanır.
  Döngü ARQ tavanına dek sürer (eski motorda `RetxConfig::fixture` `arq_retx_cap = 3`; port
  motorunda `RetxConfig::rfc9002_cc_port` `arq_retx_cap = u8::MAX` = 255, kaynak yorumu "fiilen
  sınırsız"), sonra birim geometrik kopya kuyruğuna devredilir (`CapExceedPolicy::HandToForward`).
- Kopya kuyruğu kaydı yalnız birimin bir kaydının ACK'iyle ya da RECOVERED ile kalkar; atlanan
  kayıtlar ACK'lenmez, kopya kökenli kayıp kaydı yeniden kurar. Hayalet birim bu yüzden oturum
  boyunca kopya kuyruğunda kalır ve kuyruğun sayı tavanı yoktur: hayalet birim sayısıyla doğrusal
  büyür (SR-AUDIT-001 açısından açık kalem; her `Pto` turunda bütün kuyruk taranır). Kaynakta
  kapatma #140 madde 2'dir.
- Sessiz kanalda (daha yeni seq ACK'lenmiyorsa) kayıt kayıp ilan edilmez, uçuşta kalır. Kuyruk
  kurtarması en eski uçuş kaydını hedefler (`inflight.keys().next()`); hedef hayaletse prob da
  atlanır, ACK gelmez ve prob sayacı sıfırlanmaz. Ürün profilinin eski motorunda (`validated_v1`,
  tavan 3) bütçe hayaletlere harcanabilir; port motorunda (`rfc9002_cc_port`, prob tavanı yok, en
  çok 30 s geri çekilme) problar hayalet üzerinde sürer. Gerçek kaybolmuş bir kuyruk biriminin
  hayaletlerin arkasında ne kadar beklediği ÖLÇÜLMEDİ.
- Motor `Idle`'a ancak uçuş, kayıp ve kopya kuyrukları boşken döner (`sender.rs` 4303-4306) ve
  `Idle → Startup` yalnız `Idle`'dan geçer: hayaletten sonra eski motor oturum boyunca `Idle`'a
  dönmez, sonraki toplu gönderim seyrek trafiğin hızından başlar. Toplu gönderimdeki etkinin
  büyüklüğü bu PR'da ölçülmedi (inceleme, depo dışı tek bir sim koşusunda iki motorda da yavaşlama
  bildirdi; yeniden üretilmedi).
- Port motorunda (BBR portu) her hayalet kayıp bir tıkanıklık olayıdır: BBR `recovery_window`
  uçuş tavanını (`window()`) sınırlar — INV-RATE-1'in 2026-11-11 metninin (#178) izin verdiği yol —
  ve kurtarmadayken `Startup`'tan çıkış tetiklenir (`cc/bbr.rs` `check_if_full_bw_reached`;
  `pacing_gain` düşer). Metin ağdaki gerçek kaybı öngörür; buradaki kayıplar yapaydır (birim
  teslim edilmişti, kayıt yalnız atlandığı için ACK'lenmedi). Simde pencere 4800 ↔ 254 700 B, pacing 3125 ↔
  1 200 000 B/s (aşağıda).
- Kapanış drenajı `store.is_empty() && diag_pif() == 0` ya da `drain_cap` (varsayılan 3 s) ile biter;
  uçuşta kalan hayalet kayıt `diag_pif() > 0` tutar, drenaj `drain_cap`'e dek sürer (kod okuması;
  süre ölçülmedi).

Bu ardıl iki yerde ölçüldü:

*Motor simi* (`sender.rs` `tests::k4g05br_teyit::k4g05br_ghost_after_skipped_stale_decision`; motor
kodu değişmedi). Satır-7 durumu (geç orijinal birimi teyit eder, yeniden gönderim kaydı
`pending_lost`'ta) kurulur; ardından 120 s sahte saat: runtime pompası taklidi (`Pto` 5 ms,
`RtpropProbe` 200 ms), kayıpsız sıralı kanal (ACK gönderimden 40 ms sonra), diğer birimler 50 ms
aralıkla (100 B). "Atla" kolunda teyitli birime verilen kararlar ACK'lenmez; "başlık" kolunda ACK'lenir.
Tek koşu, belirlenimli; sayılar `--nocapture` çıktısıdır.

| Motor | Kol | Hayalet karar (120 s) | Tür ve anlar (teyitten, ms) | 120 s'de motor birimi tutuyor mu | Hedefin uçuş baytı tepesi | Faz / hız sonu | Diğer birimler (2400 sunulan) |
|---|---|---|---|---|---|---|---|
| legacy + `validated_v1` kuyruk kurtarması (#176'ya dek runtime motoru; şimdi `TransportEngine::Legacy` geri alma) | atla | 14 | 2 Retransmit (5, 60) + 12 Duplicate (1590, 3130, …, 24715, 45195, 91240); 10 s kovaları `[10,1,1,0,1,0,0,0,0,1,0,0]` | Evet: kopya kuyruğunda, aralık 81.9 s (sonraki ≈ 173 s) | 400 B | 49 faz geçişi, hiç `Idle` yok; son `DrainRtprop`, btlbw 2500, pacing 1875 | 2389 teyit; ortalama 143 ms, en büyük 1400 ms |
| aynı | başlık | 1 (5 ms, Retransmit) | — | Hayır (26 s'de bıraktı) | 100 B | 3551 faz geçişi (`Idle` ↔ `Startup`/`Drain`); son `Startup` | 2399 teyit; ortalama 40 ms, en büyük 40 ms |
| legacy (`Sender::new`, kuyruk kurtarması kapalı) | atla | 14 | 2 Retransmit (5, 1540) + 12 Duplicate (3040 … 96200) | Evet (aralık 81.9 s) | 400 B | 52 geçiş; son `DrainRtprop`, pacing 1875 | 2388 teyit; ortalama 116 ms, en büyük 720 ms |
| aynı | başlık | 1 | — | Hayır (26 s) | 100 B | son `Startup` | 2399; 40 / 40 ms |
| cc_port (`Sender::with_cc_port`; #176'dan beri ürün varsayılanı — runtime ayrıca ilk pencereyi 4·MTU'ya kurar, sim kurmaz) | atla | 266 | 254 Retransmit (ilk ≈ 13 s, ≈ 50 ms aralıkla) + 12 Duplicate; kovalar `[200,62,1,1,0,1,0,0,0,1,0,0]` | Evet (aralık 81.9 s) | 100 B | 253 zaman eşiği kaybı, 267 BBR tıkanıklık olayı; pencere 4800 B | 2399; 40 / 40 ms |
| aynı | başlık | 1 | — | Hayır (1 s) | 100 B | 2 tıkanıklık olayı; pencere 254 700 B | 2399; 40 / 40 ms |

Atla ↔ başlık, birim birim teyit gecikmesi: eski runtime motorunda 2389 ortak birimin 629'unda değişti
(en büyük fark 1360 ms); kuyruk kurtarmasız legacy'de 2388'in 565'inde (680 ms); cc_port'ta 2399'un
0'ında. Okuma: legacy motorda hayalet birim kopya kuyruğunda tavansız geometrik aralıkla kalır
(yalnız birimin herhangi bir kaydının ACK'i iptal eder); kuyruk boş kalmadığı için motor `Idle`'a
dönmez ve bu seyrek, app-limited kanalda `Steady`/`DrainRtprop`/`ProbeUp` döngüsüne girer
(`DrainRtprop`'ta pacing 1875 < sunulan yük 2000 B/s). cc_port'ta yeniden iletim tavanı 255'tir
(`u8::MAX`): 1 + 254 Retransmit ile tavana ulaşılır, sonra kopya kuyruğu; 50 ms'de bir yeni ACK
geldiğinden atlanan her yeniden iletim zaman eşiğiyle kayıp sayılır (253 kez) ve BBR penceresi
tabana iner. Fırtınanın süresi 255 × kayıp ilanı aralığıdır, RTT ile büyür. Bu yükte diğer
birimlerin gecikmesi değişmedi, daha yüksek yükte etkisi bu simde ölçülmedi. Etkinin büyüklüğü bu
kanal modeline özgüdür; netem ya da ürün koşusunda ölçülmedi. Bellek bu tek hayalet için sınırlı
kaldı (kopya kuyruğunda tek kayıt; uçuş, kayıp, kopya kuyruk tepeleri ≤ 5 / 0 / 1; tombstone ≤ 8),
panik yok; birden çok hayalette kopya kuyruğu hayalet sayısıyla büyür (yukarıda). Kaynakta kapatma
#140 madde 2'dir (motor teyitli birimin bekleyen işini budar, karar üretmez).

*Runtime* (legacy + `validated_v1`, gerçek UDP loopback; `store_miss_guard_fec_recovered`): blok
teyidinden 0.36 s sonra ilk atlama; `stale_skipped` 1. s'de 2, 3. s'de 3, 10. s'ye dek 3 (yeni trafik
yok); motorun uçuş kuyruğunda 2 kayıt 10 s boyunca kaldı, kopya yok. İkinci koşu: ilk atlama 13 ms,
1. s'de 3, 10. s'ye dek 3, uçuşta 2 kayıt.

*Runtime, port motoru* (main birleştirmesinden sonra, `RuntimeConfig::default()` = `CcPort`; tek
koşu, yerel loopback, `-- --ignored --nocapture`). Eski motora kalibre fikstürlerden ikisi port
motorunda ÖN KOŞULDA kırmızıydı (iddia değil): TTL kolunda hedefin kaydı geçişten 0.156 s sonra
yenilendi (TTL alt sınırı 25.2 s), tahliye kolunda tombstone tablosu en çok 105'e ulaştı (tavan 4096).
Bu iki test `Legacy`'ye sabitlendi; port motorunda aynı yollar motor düzeyinde
(`tests::k4g05br_teyit`, iki motor) sınanır. Diğer yollar port motoruyla da koşar (yeni
`*_cc_port` testleri). Gözlem (`-p client-core --all-features`, ilk koşu): kayıp/yeniden sıralama
bekçisinde `stale_skipped` 16 (eski motorla aynı kanalda 0), ESI kolunda 6, FEC bekçisinde teyitten
0.05 s sonra ilk atlama ve trafik yokken 10 s içinde 10 → 16 (2., 4., 8. s'de +2; aralıklar
büyüyor), uçuşta 2 kayıt 10 s boyunca. Gövdesiz datagram her testte 0.

*Runtime, tahliye kolu* (`late_ack_after_cap_keeps_payload`; 4096+ dolgu birimi, her birinin orijinali
düşer). Atlanan karar sayısı atlama kolunda 123, 227, 198, 495, 500 (beş koşu); aynı test kodunda
başlık-only emisyon geri konunca (yerel mutasyon, commit edilmedi) geçerli tek koşuda 49 başlık-only
karar (iddia 6 o kolda beklendiği gibi kırmızı). Atlanan karar aynı birime yeni karar doğurur
(legacy: ARQ tavanı 3'e dek yeniden gönderim, sonra tavansız geometrik kopya); sayının artışı bu
döngüyle tutarlıdır. Faz B (geç ACK'ten sonra hedefin yeniden gönderiminin ACK'i), bekleme trafiği
tavansızken (5 ms'de bir birim): atlama kolunda 23.8 s ve 0.41 s (geçti) ve iki koşuda türetilmiş
tavan (24.2 s, 22.2 s) aşıldı — o anda ≈ 1100 teyitsiz birim, gönderim kuyruğunda 1032 birim ve
hedefin vadesi ≈ 10 s geçmiş kopyası; başlık-only kolda 0.18 s (geçerli tek koşu). Örnek küçüktür
(atla 4, başlık 1 geçerli koşu); fark, atlamanın bu yük altındaki etkisine işaret eder, büyüklüğü
ölçülmedi. Fikstür düzeltmesi: bekleme trafiği teyitsiz birim 64'e ulaşınca durur
(`TRICKLE_OUTSTANDING_MAX`; trafik yalnız ACK akışı içindir). Sonra: faz B 9.3 / 12.5 / 21.3 s
(tavanlar 18.9 / 21.4 / 26.8 s). İki koşuda (biri başlık-only kolda) tahliye kolu ön koşulda kırmızı
oldu (TTL tahliyeden önce geldi; yukarıda "Fikstür geçmişi"nde beyanlı, bu PR'dan bağımsız).
Sonuç: bekleme trafiği tavanıyla `test-runtime` atlamanın bu yükteki yavaşlamasını ARTIK
YAKALAMAZ (faz B tavanı motorun kendi kopya vadesinden türer; en dar pay ≈ 5.5 s). Yavaşlama
yukarıda gözlem olarak kayıtlıdır; kapı değildir.

**Tel bekçisi ve mutasyon.** Runtime testlerinin röle düğümü istemciden gelen her datagramı kurallardan
önce sayar; boş gövdeli DATA, yalnız önekli DATA_FRAG ve boş sembollü FEC sayısı her testte 0 olmalı
(iddia 6). Vakum (iptal öncesi): FEC bekçisinde en az bir atlanan karar ön koşuldu; motor iptaliyle
ters çevrildi (iddia 7: `stale_decisions == 0`), bu yüzden atlama dalları runtime testlerinde artık
doğal trafikle KOŞMAZ. Testte sınanan yalnız yardımcılardır (`runtime.rs` `skip_tests`:
`store_miss_builds_no_frame`, `store_hit_frame_carries_whole_body`,
`missing_repair_symbol_builds_no_frame`): `unit_frame` / `repair_symbol` store'da gövde ya da
repair-store'da sembol yoksa `None` döner, varsa çerçeve gövdenin tamamını taşır. Atlama dalının
kendisi — `drive!` içindeki `let … else` kolu: sayaçlar (`stale_decisions` / `fec_repair_missing` +
`stale_skipped`), seal ve PN yokluğu, `continue` — hiçbir testte KOŞMAZ: on bir runtime testi
`stale_decisions == 0` iddia eder ve hepsi `stale_skipped = stale_decisions = fec_repair_missing = 0`
ile biter, iddia 6'daki `stale_skipped == stale_decisions + fec_repair_missing` kimliği bugün 0 == 0
olarak sağlanır. Dalın içine konan bir başlık-only gerileme (ör. `None` yerine boş gövdeli
`encode_data`) `skip_tests`'i ve runtime testlerini yeşil bırakır; dal kod okumasıyla kapsanır.
İptal öncesi geçmiş (`2c77970a`'ya dek; motor o sürümde teyitli birime karar üretiyordu):
mutasyon (yerel, commit edilmedi, yedekten geri alındı) — (a) yolunda başlık-only emisyon geri
konunca FEC bekçisi kırmızıydı, 18 istemci datagramının 3'ü boş gövdeli DATA (PN 15, 16, 17);
mutasyon yalnız parçalamasız DATA dalını sınadı. Parçalamalı dal (eski davranış: 11 B sıfır önekli,
22 B'lık DATA_FRAG) `store_miss_guard_fec_recovered_frag_cc_port` ile koşuyordu: `max_message_bytes`
kurulu, atlanan karar ≥ 1 ön koşuldu, gövdesiz 0 (ilk koşuda `stale_skipped` 8); o dal için
mutasyon koşusu yapılmadı. Motor iptalinden sonra bu ön koşul kalktı (test atlanan karar için
yalnız gözlem serisini yazdırır; `assert_no_bodyless` `stale_decisions == 0` iddia eder) ve aynı
mutasyon bekçiyi kırmızıya çevirmez (motor o kararı üretmez). (b) yolunu tetikleyen runtime testi yok (repair-store tahliyesi ya da latch-0 bu
fikstürlerde kurulmuyor); o yol da kod okumasıyla kapsanır.

## CI (sahip kararı 2026-11-11, #174 madde 1)

`k4g05b_runtime_ownership` testleri (gerçek zamanlı, sırayla) `#[ignore]` ile hızlı kümeden
(`test-fast`) çıktı. Dosyada 11 test var: eski motorla 6 (TTL ve tahliye kolları `Legacy`'ye
sabit), port motoruyla 5 (kopya kökenli kayıp, ESI, iki bekçi, parçalamalı FEC bekçisi).

- PR (`core-ci`): test-fast'a paralel `test-runtime` işi
  `cargo test --workspace --all-features --exclude kama-integration --test k4g05b_runtime_ownership -- --ignored`
  koşar. Özellik kümesi test-fast'ınkiyle aynıdır (bölünmeden önceki PR koşusu bu birleşik
  derlemedeydi). Vakum bekçisi "`test result: ok. 11 passed; 0 failed`" birebir. İş `ci-verdict`
  `needs` listesindedir.
- main (`core-main`): ayrı `runtime-ownership` işi, üç paralel bacak: ürün varsayılanı
  (`-p client-core`), aynı birleşik all-features, release. Ayrı iş olduğundan kırmızısı
  `product-profile`'ın deterministik adımlarını atlatmaz; `main-verdict` `needs` listesindedir.
  Hiçbir workspace crate'inin varsayılan özelliği olmadığından `--no-default-features` bacağı yoktur.
- Önbellek iş başınadır (varsayılan anahtar); test-fast'ın önbellek ayarı değişmedi. `core-ci` yalnız
  PR'da koştuğu için main kapsamlı önbellek yoktur: bir PR'ın ilk koşusu soğuk derler (hosted
  test-fast günlüklerinde #173, #174, #175 "No cache found").
- Duvar süresi: hosted #174 test-fast (`36820b37`, soğuk önbellek) bu dosyada 83.5 s harcadı; toplam
  4 dk 42 s → 8 dk 6 s artışın tamamı bu dosya değildir (aynı PR'ın diğer testleri, soğuk derleme,
  runner değişkenliği). test-fast'ın ≈ 6.5 dk civarına inmesi beklenir; test-runtime soğuk derleme +
  ≈ 2–2.5 dk testle test-fast'ın altında kalmalıdır. Hosted ölçüm bekliyor.
- Yerel kapı (`scripts/ci/local-gate.sh` `test_runtime`) aynı komutu ve sayıyı kullanır; `ci.yml`
  özeti güncellendi. Hayalet simi (`k4g05br_ghost_after_skipped_stale_decision`, belirlenimli, yerel
  ≈ 6 s) hızlı kümede kalır.

## Davranış farkı ve açık soru (EVOLUTION §9.5)

**Ek (motor iptali):** motor KODU artık değişir ve motor kararları değişir (KARAR-NÖTR DEĞİL, sahip
kabulü; tam liste "Motor: teyitli birime karar yok" → "Değişen kararlar" D1–D11). Tel biçimi, golden
vektörler ve profil kimliği değişmez. Aşağıdaki paragraf iptal öncesi metindir; kimlik sorusu
(§9.5) AÇIK kalır ve artık motor kararı değişikliğini de kapsar.

Motor KODU ve tel biçimi değişmez; motorun kapalı-döngü karar akışı değişir. Tel TRAFİĞİ değişir:
gövdesi olmayan karar artık datagram üretmez (önceki başlık-only emisyon kalktı; "Atlama
politikası"), motor o kaydın ACK'ini almaz ve aynı birime yeni kararlar, kuyruk kurtarması, `Idle`
dönüşü ve (port motorunda) tıkanıklık penceresi bundan etkilenir (ölçümler "Atlama politikası").
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
  makinede ön koşul kırmızısı (iddia değil) beklenebilir. (Önceki fikstür; 2026-11-11 fikstürü
  için bir sonraki madde.)
- **Tahliye fikstürü: ön koşulun TTL payı büyütüldü (2026-11-11).** Ön koşul hâlâ bir duvar saati
  karşılaştırmasıdır (tahliye süresi ↔ TTL alt sınırı 25,16 s); yeni fikstür payı büyütür, yarışı
  kaldırmaz. Yavaşlık yanlış yeşil değil, ön koşul kırmızısı üretir. Yukarıdaki tablonun tahliye
  satırı önceki fikstürün sayılarıdır; yeni fikstürle taban (`4a9e97f1`) koşusu yapılmadı
  (ayırt edicilik ön koşullarla korunur: `mapped_seqs > 0`, tahliye ön koşulu, tablo ≥ 4096; tabanda
  kırmızı yeniden ölçülmedi). main `6716eb8d`'de testin tek
  başına yerel koşusu (`--exact late_ack_after_cap_keeps_payload`; en çok 15 koşu, ilk kırmızıda dur)
  4. koşuda ön koşulda kırmızıydı: kayıt geçişten 25,30 s sonra, tablo 3390'dayken düştü (TTL alt
  sınırı 25,16 s); yeşil üç koşuda tahliye 9,95 / 21,96 / 21,99 s'de. O fikstürde (birim başına tek
  düşüş, 4232 dolgu) teşhis koşularında uçuşta ≈ 1400 yeniden gönderim kaydı ≈ 3 s bekleyip zaman
  eşiğiyle topluca kayıp ilan edildi, teklif alımı ilk ≈ 0,6 s'den sonra ≈ 100 birim/s'ye düştü ve
  süreç CPU'su koşu boyunca ≈ %120'de kaldı (bir koşu, `/proc` örneklemesi): tavanın süresi makine
  yüküne bağlıydı. Yeni fikstür: dolgu biriminin ilk üç datagramı düşer (orijinal + iki yeniden
  gönderim → üç tombstone; eski motorda `arq_retx_cap` = 3), her sekizinci tahliye birimi
  taşıyıcıdır (düşmez, ACK ilerlemesini taşır); 1720 tahliye birimi (1505 dolgu + 215 taşıyıcı).
  Faz A öncesi boşaltma bekleme trafiğiyle (`wait_trickle`) yapılır: ilk 20'lik serinin 10.
  koşusunda son dolguların düşen yeniden gönderimlerinden sonra tele geçen datagram kalmadı ve 21
  dolgu 60 s teyitsiz kaldı (kuyruk probu 3/3; ön koşul o koşuda da tuttu, 2,81 s). Bu düzeltmeden
  sonra 20 ardışık yerel koşu yeşil: tahliye 2,80–4,87 s (ortanca 3,42 s), tablo her koşuda 4096,
  gövdesiz datagram 0, `stale_skipped` 0; dosyanın tamamı bir kez 11/11. İddialar, test adı ve
  sayısı değişmedi; ürün kodu değişmedi.
- **Tahliye boşaltmasının bütçesi (inceleme turu, 2026-11-11).** Bağımsız inceleme, CPU kısıtında
  faz A öncesi tahliye boşaltmasının (bekleme trafiğiyle) 60 s bütçenin büyük kısmını harcadığını
  ölçtü: tek çekirdeğe sabitli koşuda 42,1 s, aynı çekirdekte meşgul döngüyle 38,9 s; sabitlenmemiş
  beş koşuda 0,7–1,2 s. Bu dalda aynı koşullarda 40,6 s ve 26,1 s ölçüldü; tek çekirdekte boşaltma
  başında 1208 birim teyitsizdi (aşağıdaki 20'lik seride 126–565). Üç kısaltma denemesi ölçüldü ve
  bırakıldı (her biri tek çekirdek + yarım çekirdek, birer koşu): (i) bekleme kapısı yalnız kendi
  birimlerini sayar — 38,5 s; yarım çekirdekte 60 s'de bitmedi (289 teyitsiz); (ii) buna ek olarak
  tahliyeden sonra düşüş kalkar — 49,8 s; yarım çekirdekte 60 s'de bitmedi (458 teyitsiz); (iii)
  tahliye evresinde teyitsiz birim tavanı 256 ve tahliyeden sonra yeni birim yok — iki koşuda da ön
  koşul kırmızı (kayıt 25,25 / 25,23 s'de düştü, tablo en çok 396 / 467; 25 s'de 291 birim
  sunulabildi). Süreyi CPU kısıtında eski motorun kurtarma hızı belirliyor, fikstür değil; bu
  oynaklık kaynağı giderilmedi. Uygulanan: bu adımın bütçesi `CAP_DRAIN_WAIT` = 120 s (`TTL_WAIT`).
  Adım ön koşul değil kurulumdur: tahliye ön koşulu ondan önce denetlenir, iddialar değişmez.
  Boşaltma başında istemcinin teyitsiz birim sayısı yazdırılır. CI'da (`--nocapture` yok) süre
  satırları yalnız kırmızıda görünür. Bu değişiklikten sonraki ilk 20'lik seri 16. koşuda başka bir
  adımda kırmızıydı: ön koşul ve boşaltma geçti (kayıt 1,78 s, tablo 4096; boşaltma 6,39 s), ortak
  yoldaki boşluk bekçisi "geç ACK çerçevesi istemcide işlenmeli" 10 s'de düştü (makinenin yük
  ortalaması 5,8–6,5). Bu adım bütçe değişikliğinden etkilenmez; kök belirlenemedi: kırmızı mesajı
  geç datagramın istemciye hiç ulaşmadığını, ACK kayıt halkasından (32 kayıt) düştüğünü ya da başka
  bir nedeni ayırmıyordu. Mesaja enjeksiyon sayacı (röle tarafı) ve halka özeti (index, en büyük
  PN, `mapped_seqs`) eklendi; sayaç sıfırlandı. Ardından 20 ardışık yerel koşu yeşil (tek test,
  her koşu ayrı çağrı): tahliye 2,39–5,86 s (ortanca 3,32 s), tablo her koşuda 4096; boşaltma
  başında 126–565 birim teyitsiz, boşaltma 0,55–11,97 s (bütçe 120 s); gövdesiz datagram 0,
  `stale_skipped` 0. Dosyanın tamamı bir kez 11/11 (110,9 s).
- **Boşluk bekçisi:** her geç-ACK kolunda geç ACK çerçevesinin istemcide işlendiği ve motor seq'ine
  çevrildiği (`mapped_seqs > 0`) iki kolda da doğrulandı; kollar yalnız `new_units`'te ayrışır.

**Atlama politikasından sonraki koşu** (dal `d9f5372e`, bekleme trafiği tavansız; `-- --ignored
--nocapture`, tek koşu, yerel loopback, 6/6 yeşil, 133.9 s). Yukarıdaki tabloyla kontrollü kıyas
DEĞİLDİR (farklı derleme ve koşu; süreler gözlem). Bekleme trafiği tavanından sonraki tam koşu
(`e16b0192` ağacı): 6/6 yeşil, 106.4 s; gövdesiz 0 her testte; `stale_skipped` tahliye 198, FEC bekçisi
3, diğerleri 0.

| Test | İstemci datagramı (gövdesiz) | `stale_skipped` (= `stale_decisions` + `fec_repair_missing`) | Gözlem |
|---|---|---|---|
| `late_ack_after_ttl_keeps_payload` | 7118 (0) | 0 | faz B 19.2 s |
| `late_ack_after_cap_keeps_payload` | 13 820 (0) | 123 (123 + 0) | kayıt 17.6 s'de düştü (tablo 4096); faz B 23.8 s |
| `late_ack_after_dup_loss_keeps_payload` | 209 (0) | 0 | faz B 0.87 s |
| `parity_ack_no_confirmation_payload_kept` | 17 (0) | 0 | faz 2 3.1 s |
| `store_miss_guard_loss_and_reorder` | 543 (0) | 0 | — |
| `store_miss_guard_fec_recovered` | 15 (0) | 3 (3 + 0) | ilk atlama teyitten 0.36 s sonra |

**main birleştirmesinden sonraki koşu** (dal `d05cc48f` kodu, birleşik all-features — PR
`test-runtime` komutu; `-- --ignored --nocapture`, tek koşu, yerel loopback): 11/11 yeşil, testler
121.2 s (derleme dahil 2 dk 34 s). Gözlem; kontrollü kıyas değildir.

| Test | Motor | İstemci datagramı (gövdesiz) | `stale_skipped` | Gözlem |
|---|---|---|---|---|
| `late_ack_after_ttl_keeps_payload` | Legacy | 4406 (0) | 0 | faz B 1.8 s |
| `late_ack_after_cap_keeps_payload` | Legacy | 8703 (0) | 68 | kayıt 24.18 s'de düştü (TTL alt sınırı 25.16 s; pay ≈ 1 s), faz B 0.09 s |
| `late_ack_after_dup_loss_keeps_payload` | Legacy | 209 (0) | 0 | faz B 0.86 s |
| `late_ack_after_dup_loss_keeps_payload_cc_port` | CcPort | 78 (0) | 0 | faz B 0.01 s |
| `parity_ack_no_confirmation_payload_kept` | Legacy | 17 (0) | 0 | — |
| `parity_ack_no_confirmation_payload_kept_cc_port` | CcPort | 18 (0) | 8 | — |
| `store_miss_guard_loss_and_reorder` | Legacy | 539 (0) | 0 | — |
| `store_miss_guard_loss_and_reorder_cc_port` | CcPort | 547 (0) | 12 | — |
| `store_miss_guard_fec_recovered` | Legacy | 15 (0) | 3 | ilk atlama 0.03 s; 10 s boyunca 3, uçuşta 2 kayıt |
| `store_miss_guard_fec_recovered_cc_port` | CcPort | 15 (0) | 16 | ilk atlama 0.03 s; trafik yokken 1. s'de 10, 7. s'de 16 (2., 4., 7. s'de +2); uçuşta 3 kayıt |
| `store_miss_guard_fec_recovered_frag_cc_port` | CcPort (DATA_FRAG) | 15 (0) | 8 | ilk atlama 0.42 s; trafik yokken 1. s'de 4, 6. s'de 8 (3., 6. s'de +2); uçuşta 2 kayıt |

Aynı kanal ve aynı fikstürde port motorunda atlanan karar sayısı eski motordan yüksektir (kayıp
bekçisi 12 ↔ 0, ESI 8 ↔ 0, FEC bekçisi 16 ↔ 3): port motoru hayalete daha çok karar üretir (ARQ
tavanı 255, sınırsız PTO probu). Tahliye kolunun TTL alt sınırına payı bu koşuda ≈ 1 s'dir; yavaş
runner'da bu kolun ön koşul kırmızısı ("Fikstür geçmişi") beklenebilir.

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
- `k4g05br_decision_neutral_after_confirmation_with_pending_work` — satır 7 durumunda teyit kayıp
  kaydını iptal eder, birime sonra karar yok; drenajlı motor ile hiç drenajsız ikizi her olayda
  aynı karar ve uyanma.
- `k4g05br_tombstone_late_ack_confirms_once_per_event` (kol-2) ayrıca: teyitte uçuştaki yeniden
  gönderim kararsız ama uçuşta; onun sonraki ACK'i teslim kredisidir (`delivered` += boy), uçuş
  baytını düşürür, karar üretmez ve teyidi yineler.
- Motor iptali (sahip kararı 2026-11-11), iki motor (`Sender::new`, `Sender::with_cc_port`):
  - `k4g05br_confirmed_unit_inflight_copy_never_requeued` — kol-2: teyitte yeniden gönderim
    kararsız, uçuş baytı aynı; daha yeni seq'ler ACK'lenince kayıp ilan edilir: tombstone var,
    `pending_lost` yok, port motorunda tam bir tıkanıklık olayı (gerçek kayıp); geç ACK'i kol-2 teslim
    kredisi ve reorder mesafesi; birime karar yok.
  - `k4g05br_confirmed_copy_released_in_silence` (dört kurucu: kuyruk kurtarmalı iki motor + iki
    temel kurucu) — sessiz kanal sınırı: bırakma anından önce uçuşta, anında kayıp ilanı olmadan
    düşer (uçuş baytı, `diag_pif`, sayaç), tıkanıklık olayı ve tombstone yok, prob yok, `Idle`;
    sonraki çok geç ACK teyit ve kredi vermez.
  - `k4g05br_confirmed_unit_dup_queue_cancelled` — PreConverge geometrik kopya: (a) orijinalin
    ACK'i kopya kaydını iptal eder; (b) kopyanın ACK'i birimi teyit eder, uçuştaki orijinal
    kararsız; RTT örneği yoksa (eski motor) PreConverge'de bekler ve bırakma anında düşer, örnek
    varsa (port motoru) kayıp ilan edilir — ikisinde de yeniden kuyruk yok, sonunda `Idle`.
  - `k4g05br_confirmed_unit_lost_duplicate_not_recreated` — Duplicate kökenli kararsız kopyanın
    kaybı geometrik kopyayı yeniden kurmaz.
  - `k4g05br_recovered_control_marks_inflight_copies` — RECOVERED: kopya kaydı iptal ve işaret
    sayaçları RECOVERED'da da yazılır; uçuş baytı, `diag_pif`, teslim kredisi ve BtlBw değişmez (N8).
  - `k4g05br_recovered_control_removes_unsent_unit` — gönderim kuyruğundaki birim çıkar, hiç
    gönderilmez; listelenmeyen birim gönderilir.
- Kuyruk kurtarmalı iki motor (`legacy+tail_v1`, `cc_port`): `k4g05br_confirmed_unit_tail_probe_cancelled`
  (jeton bekleyen prob iptal), `k4g05br_tail_probe_skips_marked_copy` (prob kararsız kopyayı atlar,
  arkasındaki teyitsiz birimi hedefler, prob turu boşa gitmez).
- İnceleme bulguları: `k4g05br_parity_ack_keeps_esi_namesake_copies` (iki motor),
  `k4g05br_tail_probe_retx_count_saturates` ve `k4g05br_tail_probe_never_targets_parity` (kuyruk
  kurtarmalı iki motor).
- İkinci inceleme turu:
  - `k4g05br_marked_data_record_spurious_loss_feeds_reorder` (iki motor; D12) — teyitli birimin
    `Data` kökenli kaydı kayıp ilan edilip geç ACK'lenir, `pending_lost` kaydı yok: (a) RECOVERED'la
    kararsız işaretli uçuş kaydı yeniden kuyruksuz kayıp; (b) kayıp kaydı birimin Duplicate kopyasının
    ACK'iyle teyitte iptal (kopya beyaz kutu `track_sent`); (c) kayıp kaydı RECOVERED'la iptal. Üçünde
    geç ACK kol-1 (BW örneği) ve `recent_max_reorder = la − seq`; birime karar yok.
  - `k4g05br_release_deadline_wakes_wakeup_only_driver` (dört kurucu; D14) — uçuşta yalnız kararsız
    kopya varken yalnız `next_wakeup_ns`'e uyanan sürücü (her uyanmada `Pto`) kopyayı bırakma anında
    bırakır; kopya uçuştayken uyanma `None` değil, sonunda `None` ve `Idle`.
  - `k4g05br_confirmed_unit_second_probe_cancelled` (port motoru, `pto_probes = 2`) — ikinci prob
    yuvasının hedefi kararsız: geç orijinal ACK'i ile teyitte emisyon yolu yuvayı boşaltır
    (`pto2_cleared` +1, `pto2_emitted` aynı); RECOVERED teyidinden sonra yuva uyanma doğurmaz
    (`e2a_probe2_ready_ns` `None`), sonraki emisyon yolu aynı biçimde boşaltır. Yuva beyaz kutu (o
    turun kimliğiyle).
- `k4g05br_ghost_after_skipped_stale_decision` — artık iddia eder: teyitten sonra birime 0 karar,
  birim ufukta bırakılır, motor `Idle`'a döner, BBR ve diğer birimlerin gecikmesi iki kolda aynı.
- `k4g05br_confirmation_output_is_decision_neutral_{legacy,cc_port}` — LCG izli kanal modeli (kayıp,
  geç teslim, kesinti, RECOVERED, parite); drenajlı motor ile taşan ikizi her olayda aynı karar ve
  uyanma, son durumda aynı tanı yüzeyleri.

Mutasyon denetimi (yerel, commit edilmedi; `fcd8efb6` üzerinde, tekilleştirme o sürümde ilk görülme
sırasını koruyan kümeyle yapılıyordu): parite süzgecini, un-loss itişini, tombstone itişini ve olay
içi tekilleştirmeyi kaldırmak ilgili testi kırmızıya çevirdi. Drenaja `pending_lost` budaması
eklemek (#140 madde 2'ye benzer yan etki) satır-7 senaryo testini (ilk ayrışma eski motor kolunda) ve
port motorunun LCG izini kırmızıya çevirdi; eski motorun LCG izi bu mutasyonda YEŞİL kaldı (iz, eski
motorda o durumu nadiren kurar) — eski motor için duyarlılık senaryo testine dayanır.

Mutasyon denetimi (yerel, commit edilmedi, yedekten geri alındı ve `cmp` ile doğrulandı; motor
iptalinin ikinci sürümü üzerinde, `--lib k4g05br`): yedi karar kolu tek tek açıldı, her biri en az bir
testi kırmızıya çevirdi — kararsız kopyanın kaybında `pending_lost` itişi (2 test), prob hedef
seçiminde işaret süzgeci (1), sessiz kanal sınırı (2), canlılık kopyası süzgeci (1), Duplicate
kökenli kayıpta geometrik kopya (1), RECOVERED'da gönderim kuyruğu çıkarımı (1), bekleyen probun
emisyonda yeniden doğrulanması (1).

İkinci inceleme turu (yerel, commit edilmedi, yedekten geri alındı ve `cmp` ile doğrulandı): üç yeni
testten ikisi düzeltmeden ÖNCE kırmızıydı (D12 testi eski motor kolunda (a)'da
`recent_max_reorder` 0 ≠ 5; uyanma testi `legacy+tail_v1`'de kararsız kopya uçuştayken
`next_wakeup_ns = None`); ikinci prob testi kapsama testidir (kod zaten doğruydu; ilk sürümü yalnız
kendi ön koşulunda kırmızıydı). D12 kuralı kapalıyken test (a)'nın geç ACK adımı atlanınca (b)
(0 ≠ 11), (b)'ninki de atlanınca (c) (0 ≠ 4) kırmızı (eski motor kolu). E2a ikinci prob emisyonundaki işaret süzgeci kaldırılınca ikinci
prob testi kırmızı (ikinci prob çıktı, kopya tombstone'a emekli edildi); `e2a_probe2_ready_ns`'deki
süzgeç kaldırılınca kırmızı (kararsız hedefli yuva uyanma verdi).

Mutasyon denetimi (yerel, commit edilmedi, yedekten geri alındı; `216206bc` üzerinde): eski çağrı-içi
düşürme geri konunca `k4g05br_confirmed_units_call_output_never_dropped` kırmızı (eski motor
kolunda tek çağrının ilk 37 teyidi düştü; test ilk ayrışmada durur). Sıralama + tekilleştirme
kaldırılınca üç test kırmızı: sıra (`k4g05br_inflight_ack_confirms_unit_ids_not_seqs`,
`k4g05br_recovered_control_confirms_listed_units`) ve olay içi tekil teyit
(`k4g05br_tombstone_late_ack_confirms_once_per_event`).

## #140'ta kalanlar

- Motor: `UnitOwnership` dizini, teyit kümesi ("birim oturumda en çok bir kez"), RTO_MAX tavanı
  (madde 3), sahiplenme adayları (madde 4), K_MAP, parite kaydı sahibi ve süre dolumu (madde 1).
  Satır 4, 6 ve satır 8b'nin teyit ayrımı. Teyitli birime karar üretmeme (madde 2) bu PR'da
  `UnitOwnership` OLMADAN olay anında iptal ve kararsız kopya işaretiyle kapandı; satır 5'in kopya
  iptali ve satır 8b'nin gönderim kuyruğu tarafı bu PR'da kapandı.
- Kimlik değişikliği (`transport_v1_freeze.rs` düzenlemesi) — bu PR kimlik değiştirmez.
- Runtime testinin #140'taki parite faz 2'si ("esi adaşı kopyaları sürer, bölünme (r, k−r)") #140
  madde 1'e (parite kaydı sahibi) bağlı kalır; motor tarafında ESI adaşının kopyalarının sürmesi bu
  PR'da sınanır (`k4g05br_parity_ack_keeps_esi_namesake_copies`). Bekçilerdeki `stale_decisions == 0`
  iddiası bu PR'a girdi (her runtime testinde iddia 7).
- Aralık birleşmesinde ele alınacak iki ayrım: bu PR'ın teyit çıktısı olay içinde artan birim
  kimliği sırasıyla ve çağrı girişinde budanan birikimle çalışır (motor testleri bunu pinler);
  #140'ın `confirmed_out`'u kendi sırası ve tavanıyla çalışır. Runtime (`drain_confirmed!`) iki
  biçime de bağlı değildir.
