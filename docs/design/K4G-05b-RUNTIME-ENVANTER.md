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
bekleyen işini (kayıp kuyruğu, kopya kuyruğu, uçuştaki diğer kopyaları) iptal eder — teyitli birime
karar üretmez (#140 madde 2'nin amacı, `UnitOwnership` modeli OLMADAN). Bu motor değişikliği
KARAR-NÖTR DEĞİLDİR; değişen kararların tam listesi "Motor: teyitli birime karar yok" bölümündedir.

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

**Karar-nötrlük (drenaj).** Teyit çıktısı kodu yalnız iki çıktı alanına yazar (`confirmed_units_out`,
`confirmed_units_overflow`); hiçbir karar yolu onları okumaz (çağrı girişindeki budama da yalnız
bu iki alana dokunur): drenaj yapan ve yapmayan motor aynı kararı verir. Teyit OLAYI ise artık
motorda birimin işini iptal eder (aşağıda "Motor: teyitli birime karar yok"; `acked_units` kalktı,
yerine aynı noktada `cancel_confirmed_units`). Pinler: `sender.rs` `tests::k4g05br_teyit` (aşağıda) ve değişmeden yeşil kalan
`tv_*`, `sim_determinism`, `cc_port_sim`, `sender_diag_contract`, `e1_goster`, `e2a_goster`,
kama-engine `n8_v2`, client-core `ctx_first_dispatch`.

## Motor: teyitli birime karar yok (sahip kararı 2026-11-11)

Karar metni (gece yetkisi 2026-11-11 §2.4 ve öncesi): "geçmezse kökü motorda kapat (teyitli birimin
bekleyen retx/prob kayıtlarını ACK anında iptal), iki kurucuyla test, yeniden koş, MERGE." ve
"teyit edilen birimin bekleyen yeniden iletim / kopya kayıtları ACK anında iptal edilmeli (dup_queue
için acked_units iptali zaten var; pending_lost ve tail probe için aynı iptal). İki kurucuda da test.
Bu motor değişikliği karar-nötr değil ama 'teyitli birime karar üretme' #140'ın asıl amacı."

**Bu motor değişikliği KARAR-NÖTR DEĞİLDİR (sahip kabulü).** Teyit çıktısının drenajı
(`take_confirmed_units`) karar-nötr kalır (drenaj yapan ve yapmayan ikiz testleri değişmeden yeşil);
karar değişikliği teyit OLAYININ kendisindedir.

**Mekanizma** (`sender.rs` `Sender::cancel_confirmed_units`). Teyit olayları teyit çıktısını
besleyenlerle aynıdır: kökeni `FecParity` olmayan canlı kaydın ACK'i (uçuş, tombstone, un-loss) ve
`RecoveredControl`. Olay anında, artan sıralı tekil birim listesiyle:

- `pending_lost` kayıtları silinir (ARQ yeniden gönderimi, RecoverFwd devri ve ARQ tavanı devri
  buradan beslenir);
- `dup_queue` kayıtları silinir (önceki `acked_units` iptalinin yerine, aynı noktada);
- uçuştaki DİĞER kopyalar (köken `FecParity` değil) kayıp İLAN EDİLMEDEN uçuştan düşer:
  `inflight_bytes` (port motorunun uçuş baytı) ve `bw_sampler` uçuş sayısı (`diag_pif`) bırakılır,
  tıkanıklık olayı üretilmez. `queue_losses` yalnız uçuştaki kaydı işlediğinden düşen kopya sonradan
  kayıp ilan edilse de yeniden kuyruğa giremez; kuyruk probu hedefi uçuştan seçildiğinden ve bekleyen
  prob emisyonda yeniden doğrulandığından prob hedefi olamaz (bekleyen prob iptal sayılır).
- Dokunulmayanlar: tombstone kayıtları (karar doğurmazlar; geç ACK'leri yol kanıtıdır — örnek
  kurtarma, spurious — ve TTL ya da `TOMBSTONE_MAX` ile düşerler; her tombstone gibi `diag_pif`'i
  TTL'e dek tutarlar) ve kayıp dedektörünün kaydı (ilan ve ledger öncekiyle aynı). Kuyruk
  kurtarmasının "ACK ilerlemesi" kuralı değişmedi: teyitle düşen kopyalar ilerleme sayılmaz, uçuş
  boşalırsa kurtarma söner.
- Sınır (SR-AUDIT-001): teyit KÜMESİ tutulmaz. Yeni iletim yalnız `tx_queue`, `pending_lost`,
  `dup_queue` ve uçuş kaydından doğar; teyitli birim `tx_queue`'da olamaz. Geçici seq listesi en çok
  uçuş haritası kadardır; kalıcı ek bellek yalnız üç doygun sayaçtır (`ConfirmedCancelDiag`,
  `Sender::diag_confirmed_cancels`, salt gözlem). Kopya kuyruğunun hayalet birim sayısıyla
  büyümesi (önceki açık kalem) kapanır: teyitli birime kopya kaydı kurulmaz.

Aynı seride, aynı yoldaki üç inceleme bulgusu (ayrı commit'ler):

1. **ESI ad-uzayı sızıntısı.** Parite kaydının ACK'i (uçuş ve tombstone dalı) `acked_units`'e ESI'yi
   itiyordu; ESI ile aynı sayısal kimlikli KAYNAK birimin kopyaları iptal oluyordu. Parite artık kopya
   iptaline girmez (envanter satır 5'in "ters yönü" kapandı).
2. **`emit_tail_probe` `retx_count + 1` (u8).** Port motorunun yeniden iletim tavanı 255 olduğundan
   255'teki kayıt prob hedefi olabilir: debug'da taşma paniği, release'de 0'a sarma (0, kopyanın geç
   ACK'ini kol-1 örneği yapardı). Artık doygun.
3. **Kuyruk probu parite kaydını hedefliyordu** (hedef en eski uçuş kaydı; prob onu `unit_id = ESI`
   taşıyan `Retransmit` olarak yeniden gönderirdi). Hedef artık en eski KAYNAK birim kaydıdır
   (`oldest_probe_target`; E2a ikinci prob yuvası dahil). Uçuşta yalnız parite varsa prob yoktur
   (kuyruk kurtarması söner).

### Değişen kararlar (tam liste)

| # | Ne | Ne zaman | Önce | Şimdi |
|---|---|---|---|---|
| D1 | Teyitli birime ARQ `Retransmit` | Birimin `pending_lost` kaydı varken başka bir kaydı ACK'lenir ya da RECOVERED gelir (satır 7) | Kayıt ARQ'ya girer, karar üretilir (port motorunda tavan 255) | Kayıt teyitte silinir, karar yok |
| D2 | Teyitli birime RecoverFwd ileri kopyası / ARQ tavanı devri | Aynı (kopyalar `pending_lost`'tan doğar) | Üretilir | Yok |
| D3 | Teyitli birimin uçuştaki diğer kopyasından doğan kararlar | Birimin bir kopyası teyit eder, diğeri uçuştadır (ör. kopya ACK'lenir, orijinal uçuşta; geç orijinal teyit eder, yeniden gönderim uçuşta) | Kopya sonradan kayıp ilan edilir → `pending_lost` / kopya kaydı (Duplicate kökenli kayıpta geometrik kopya yeniden kurulur) / saf-sessizlik canlılık kopyası / kuyruk probu hedefi | Kopya teyitte uçuştan düşer; bu kararların hiçbiri yok |
| D4 | Port motoru tıkanıklık olayı | D3'teki kopya kayıp ilan edildiğinde | `on_congestion_event` (BBR `recovery_window`) | Yok (uçuş kaydı yok → kayıp baytı yok) |
| D5 | Uçuş baytı ve uçuş sayısı | Teyit anı | Kopyalar ACK ya da kayıp ilanına dek uçuşta | Teyitte düşer: port motorunda `cc_fits` penceresi daha erken açılır, `on_end_acks` uçuş baytı küçük, iş kalmadıysa uygulama-sınırlı damgası daha erken; eski motorda EAI'nin ortalama birim boyu, `ack_confidence` etkinliği ve `Idle` geçişi (`update_state`) buna bağlıdır; `bw_sampler` uçuş sayısı 0'a inince gönderim çapası yeniden kurulur |
| D6 | Düşen kopyanın sonraki ACK'i | Kopya tele çıkmıştı ve sonra ACK'lenir | Uçuş kaydı ACK'i: BW örneği, CC `on_ack`, ikinci teyit | Bilinmeyen seq: örnek, CC `on_ack` ve teyit yok (port motorunda RTT v1.1 kuralı çerçevenin en büyük seq'i buysa o çerçeveden örnek almaz) |
| D7 | Faz | Eski motor, PreConverge, kuruluş kopyası birimi teyit eder, orijinal kayıp | PreConverge'de ölü orijinalle bekler | İş kalmadıysa `Idle`; sonraki veri yine PreConverge'e girer (ölçüm yok) |
| D8 | Parite ACK'i, ESI adaşı kaynak birimin kopyaları | FEC açık, kaynak birim kimliği = ESI | Kopyalar iptal | Kopyalar sürer |
| D9 | Kuyruk probu hedefi | En eski uçuş kaydı parite | Parite `Retransmit` (`unit_id = ESI`) olarak yeniden gönderilir | En eski kaynak birim kaydı; yalnız parite uçuştaysa prob yok |
| D10 | Prob kopyasının `retx_count`'u | Hedefin sayacı 255 | Debug panik / release 0 | 255 |

Teyit çıktısı da etkilenir (karar değil): düşen kopyanın geç ACK'i ikinci kez teyit üretmez; birimin
tombstone kaydının geç ACK'i yine üretebilir (olaylar arası tekilleştirme tüketicinindir).

### Hayalet ölçümü — önce ↔ sonra

`tests::k4g05br_teyit::k4g05br_ghost_after_skipped_stale_decision` (kurulum ve kanal modeli aşağıda
"Atlama politikası"nda; 120 s sahte saat, belirlenimli, tek koşu, `--nocapture`). "Önce" = dal
`2c77970a` (motor iptali yok), "sonra" = bu seri. Sonra sütunlarında "atla" ve "başlık" kolları
birebir aynıdır (teyitli birime karar olmadığından iki kol ayrışmaz).

| Motor | Kol | Teyitten sonra birime karar (önce → sonra) | Birimin bırakılması (bütün kayıtlar) | Motor `Idle`'a döner mi | Faz geçişi (120 s) | BBR (pencere B, tıkanıklık olayı) | Diğer birimler: teyit / ort. / en büyük gecikme |
|---|---|---|---|---|---|---|---|
| legacy + `validated_v1` (`TransportEngine::Legacy`) | atla | 14 (2 Retransmit + 12 Duplicate) → **0** | Hiç (kopya vadesi ≈ 173 s) → **26 s** | Hayır → **evet, teyitten 5 ms sonra** | 49 → 3549 | — | 2389 / 143 / 1400 ms → **2399 / 40 / 40 ms** |
| aynı | başlık | 1 → **0** | 26 s → 26 s | Evet → evet | 3551 → 3549 | — | 2399 / 40 / 40 → aynı |
| legacy (`Sender::new`) | atla | 14 → **0** | Hiç → **26 s** | Hayır → **evet** | 52 → 3549 | — | 2388 / 116 / 720 ms → **2399 / 40 / 40 ms** |
| aynı | başlık | 1 → **0** | 26 s → 26 s | Evet → evet | 3551 → 3549 | — | 2399 / 40 / 40 → aynı |
| cc_port (`Sender::with_cc_port`, ürün varsayılanı) | atla | 266 (254 Retransmit + 12 Duplicate) → **0** | Hiç → **1 s** | Hayır → **evet** | 550 → 3549 | (4800, 267) → **(254 600, 2)** | 2399 / 40 / 40 → aynı |
| aynı | başlık | 1 → **0** | 1 s → 1 s | Evet → evet | 3551 → 3549 | (254 700, 2) → (254 600, 2) | 2399 / 40 / 40 → aynı |

Teyit olayında iptal edilen iş her kolda `pending_lost` 1, kopya 0, uçuş kopyası 0 (satır-7 kurulumunda
birimin yeniden gönderimi kayıp kuyruğundadır). "Bırakma" birimin SON kaydının düştüğü ilk 1 s'lik
örnektir: kalan kayıt yeniden gönderimin tombstone'udur ve TTL'le düşer (eski motorda TTL tabanı
cadence, ≈ 26 s; port motorunda RFC 9002 biçimi, ≈ 1 s). Başlık kolunda port motorunun penceresi
100 B küçüktür: önceki başlık-only hayalet `Retransmit` (100 B) ACK'lenip pencereye ekleniyordu.
İddia edilenler (test): teyitte kayıp kaydı iptali = 1, teyitten sonra birime 0 karar, birim ufukta
bırakılır, motor `Idle`'a döner, BBR iki kolda aynı, diğer birimlerin gecikmesi iki kolda birim birim
aynı. Önceki inceleme gözlemleri ("ghost records consume the tail-probe budget", "dup_queue grows
without cap", "shutdown drain runs to drain_cap because pif > 0") hayalet kayıt kalmadığı için bu
yoldan oluşmaz; kapanış drenajında `diag_pif`'i yalnız tombstone'lar (her kayıp gibi) TTL'e dek tutar.

### Pinlenmiş beklentilerde değişenler (her biri gerekçeli)

- `tests/e2a_goster.rs` çıplak iz tabanı `0xa9f2_a6bc_da15_8482` → `0x9e4f_8cca_eac8_6803` (ürün
  benzeri iz `0x1e49_5f75_283c_9675` DEĞİŞMEDİ). Karar dökümü (iki ağaç, ortam değişkeniyle geçici
  döküm; commit edilmedi): birim 0'ın orijinali (seq 0) kaybolur, kuruluş kopyası (seq 1) ulaşır ve
  birimi teyit eder; önceki motor 10.635 s'de birim 0'a `Retransmit` (seq 5) veriyordu, şimdi
  vermez. Sonraki seq'ler kaydığından seq'e bağlı kayıp karması izin geri kalanını değiştirir (154 →
  153 karar). Eski değer dosyada yorum olarak duruyor.
- `tests/dr_cs1_acceptance.rs` DI-1, DI-1C, DI-2 (yalnız `--all-features`): sabit `acked_seqs: [1]` /
  `[1, 2]` u2/u3'ün değil u1'in PreConverge kuruluş kopyalarının seq'leriydi; o ACK u1'i teyit
  ettiğinden testler teyitli birime yeniden gönderim bekliyordu. Şimdi u2/u3'ün gerçek DATA seq'i
  çıktıdan alınır (DI-1A zaten böyleydi). DI-1C'de u1 gerçekten kayıp kalır; ikinci kurtarma
  emisyonu ≈ 60 s ACK sessizliğinden sonra RecoverFwd'da ileri kopyadır (Duplicate), yardımcı tür
  listesi alır (`expect_emission_via_wakeup`). İddia edilen özellik (yeni ağ olayı olmadan açık
  zamanlayıcıyla emisyon, busy-loop yok, nihai ACK'ten sonra vadesi geçmiş uyanma yok) değişmedi.
- `tests/sender_state_matrix.rs` (yalnız `--all-features` kırmızıydı): `experimental-cold-start`
  derlemesinde `on_event` = `on_event_batch`; faz-3 vadesi gelen iç Pto'yu AYNI çağrıda ikinci boru
  hattı turuyla işler. Bir tohumda bir çağrı iki geçiş yaptı ve Startup → Steady göründü
  (ayrıştırma: Startup → Drain → Steady; iki adım da tabloda izinli). Yürüyüş artık aktivasyon
  sayacı iki geçiş gösteren çağrının yasak görünen çiftini iki izinli tek turluk adıma ayrıştırır
  (Recover* girişi sayıldıysa ara durum o Recover*); ayrıştırılamayan ya da tek geçişli yasak çift
  yine kırmızıdır. 8 tohum × 20 000 adımda 1 bileşke.
- `sender.rs` `e2a_m8_b_duplicate_origin_ack_exits_preconverge_only_when_on` kapalı kolu (D7): kopya
  ACK'inden sonra durum `PreConverge` yerine `Idle`; sonraki AppData yine `PreConverge`'e girer.
- `k4g05br_decision_neutral_after_confirmation_with_pending_work` artık iptali iddia eder (önce
  gözlemdi); drenaj ikizi karşılaştırması aynen sürer.
- Runtime: FEC bekçisinin vakum ön koşulu (`stale_skipped ≥ 1`) ters çevrildi; her runtime testinde
  yeni iddia 7: `stale_decisions == 0`.

### `cc_port_sim` — önce ↔ sonra

`cargo test -p adaptive-transport --test cc_port_sim -- --nocapture --test-threads=1`, aynı makine,
tek koşu. 24 sonuç satırının 11'i değişti:

| Hücre | Motor | Önce (`2c77970a`) | Sonra |
|---|---|---|---|
| kesinti 60 s, seed 5 | legacy | tamamlandı 119.88 s, 125/125, retx 494 | **tamamlanmadı (180 s), 124/125**, retx 487 |
| kesinti özet | legacy | teslim 2/5, p50 119.9 s | **1/5**, p50 118.4 s |
| kesinti 60 s, seed 1 / 2 / 3 / 4 / 5 | cc_port | 95.88 / 87.90 / 85.13 / 84.80 / 80.73 s; retx 86 / 93 / 60 / 61 / 85 | 77.17 / 81.47 / 85.15 / 84.81 / 80.62 s; retx 78 / 90 / 60 / 60 / 84 |
| kesinti özet | cc_port | 5/5, p50 85.1 s | 5/5, p50 81.5 s |
| q1000 l20, 10 seed | legacy | 7/10, p50 17.37 s, en büyük 60.50 s | **6/10**, p50 17.37 s, en büyük 54.31 s |
| q6 l20, 10 seed | legacy | 7/10, p50 16.36 s, en büyük 59.98 s | **6/10**, p50 16.36 s, en büyük 54.11 s |
| q6 l20, 10 seed | cc_port | 10/10, p50 15.89 s, en büyük 17.12 s | 10/10, p50 15.89 s, en büyük 17.28 s |

Değişmeyen satırlar: q6 l0 (iki motor), q1000 l20 cc_port (10/10, p50 15.71 s, en büyük 17.34 s),
PTT demeti (0.64 / 0.34 s), FEC parite hücresi, kesinti legacy seed 1–4. Port kolu (bağlayıcı) bütün
eşiklerini geçer (7/7 test yeşil).

Okuma sınırı: kanal modeli kayıp kararını her gönderimde LCG'den çeker; hayalet emisyonların kalkması
sonraki çekimleri kaydırır, tohum başına önce ↔ sonra eşli bir kıyas DEĞİLDİR. Tek tohum teşhisi
(q6 l20 legacy seed 10, iki ağaçta ortam değişkeniyle geçici karar dökümü; commit edilmedi): önce
59.98 s'de tamamlanıyordu — birim 0 kuruluş kopyasıyla teyitliydi ama 39.94 s'de ona hayalet
`Retransmit` gitti ve sonraki kayıp çekimleri bu emisyonla kaydı. Sonra: son emisyon 56.2 s'de birim
89'un `Retransmit`'i; ondan sonra hiçbir emisyon yok, 120 s'ye dek uçuşta 1200 B, durum RecoverFwd.
Kod okuması: sim'in eski kolu `Sender::new()` ile kuyruk kurtarması KAPALI koşar; kaybolan son
`Retransmit`'ten sonra seq gönderilmediği için kayıt kayıp ilan edilmez ve RecoverFwd'un saf-sessizlik
canlılık kolu (`plan_forward_duplication`) yalnız `Data` kökenli uçuş kayıtlarına kopya kurar. Aynı
takılma tabanda seed 5'te zaten vardır (iki ağaçta karar dökümü birebir aynı, 124/125). Kesinti seed 5
ve q1000 legacy değişimi ayrıca teşhis edilmedi. Ürün geri alma yolu (`TransportEngine::Legacy`)
`validated_v1` kuyruk kurtarmasıyla koşar (prob hedefi her köken); sim kolu onu kurmaz. Bu canlılık
deliği bu PR'da düzeltilmedi (açık kalem).

### Runtime testleri (`k4g05b_runtime_ownership`, `-- --ignored`)

PR `test-runtime` komutu (`--workspace --all-features --exclude kama-integration --test
k4g05b_runtime_ownership -- --ignored --nocapture`), tek koşu, yerel loopback: **11/11 yeşil**, testler
119.5 s. Her testte gövdesiz datagram 0 ve `stale_skipped = stale_decisions = fec_repair_missing = 0`.
TTL ve tahliye fikstürleri (`Legacy`'ye sabit) ön koşulları ve iddialarıyla geçti; aynı yollar port
motorunda motor düzeyinde (`tests::k4g05br_teyit`, iki motor) sınanır. Gözlem; kontrollü kıyas değil.

| Test | Motor | İstemci datagramı (gövdesiz) | `stale_skipped` önce → sonra | Gözlem (sonra) |
|---|---|---|---|---|
| `late_ack_after_ttl_keeps_payload` | Legacy | 5399 (0) | 0 → 0 | faz B 8.3 s |
| `late_ack_after_cap_keeps_payload` | Legacy | 9274 (0) | 68 → **0** | kayıt 17.5 s'de düştü (tablo 4096); faz B 0.11 s |
| `late_ack_after_dup_loss_keeps_payload` | Legacy | 209 (0) | 0 → 0 | faz B 0.86 s |
| `late_ack_after_dup_loss_keeps_payload_cc_port` | CcPort | 80 (0) | 0 → 0 | faz B 0.01 s |
| `parity_ack_no_confirmation_payload_kept` | Legacy | 18 (0) | 0 → 0 | faz 2 3.1 s |
| `parity_ack_no_confirmation_payload_kept_cc_port` | CcPort | 20 (0) | 8 → **0** | faz 2 0.56 s |
| `store_miss_guard_loss_and_reorder` | Legacy | 545 (0) | 0 → 0 | — |
| `store_miss_guard_loss_and_reorder_cc_port` | CcPort | 542 (0) | 12 → **0** | — |
| `store_miss_guard_fec_recovered` | Legacy | 15 (0) | 3 → **0** | teyitten sonra 10 s: motor (uçuş, kayıp, kopya) her saniye (0, 0, 0) |
| `store_miss_guard_fec_recovered_cc_port` | CcPort | 15 (0) | 16 → **0** | aynı, (0, 0, 0) |
| `store_miss_guard_fec_recovered_frag_cc_port` | CcPort (DATA_FRAG) | 15 (0) | 8 → **0** | aynı, (0, 0, 0) |

"Önce" sütunu yukarıdaki "main birleştirmesinden sonraki koşu" tablosudur (dal `d05cc48f` kodu, tek
koşu); iki koşu aynı makinede ama farklı derleme ve zamanda, kontrollü kıyas değildir. İptal öncesi
FEC bekçilerinde teyitten 10 s sonra motorda 2–3 uçuş kaydı kalıyordu; şimdi 0.

YAMA ADAYI (specs/ düzenlenmedi): `adaptive-transport-engine-spec` §3.3-B "stop koşulu" yalnız kopya
iptalini anar; teyit olayında `pending_lost` ve uçuş kopyalarının iptali, ve kuyruk probunun onarım
sembolünü hedeflememesi için spec satırı adayı. Kimlik sorusu (§9.5) aşağıda açıktır.


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
| 7 | **Teyit sonrası kalan kayıt** (ör. geç orijinal teyit eder, kaybolan yeniden gönderim `pending_lost`'ta) | Tabanda motor teyitli birimin `pending_lost` kaydını budamaz. Bu PR: teyitte kayıp, kopya ve uçuş kaydı iptal (`cancel_confirmed_units`) | Evet | Kayıt iptal; karar YOK | İlk ACK'te `store.remove`; kalan kararlar başlık-only (`stale_decisions`) VE harita kaydı açar (budanmaz) | Karar yok (`stale_decisions == 0`, iddia 7); gelirse atlanır (tele datagram çıkmaz, harita kaydı açılmaz) | `store_miss_guard_loss_and_reorder` (+ `_cc_port`); motor `k4g05br_decision_neutral_after_confirmation_with_pending_work`, `k4g05br_confirmed_unit_*` |
| 8 | **FEC RECOVERED** | `on_recovered_control`: `pending_lost`/`dup_queue` iptali; tabanda uçuştaki kayıt kalır (satır 7'ye düşerdi). Bu PR: uçuştaki kayıtlar da kayıp ilan edilmeden düşer | Evet | Listelenen her birim teyit edilir | `store.remove` + `acked()` motordan ÖNCE | Drenajda (motordan SONRA); store'da varsa düşer — sonuç tabanla aynı; teyitten sonra karar yok | `store_miss_guard_fec_recovered` (+ `_cc_port`, `_frag_cc_port`); motor `k4g05br_recovered_control_retires_inflight_copies` |
| 8b | RECOVERED, motorun hiç göndermediği birim | — | #140 envanterine göre erişilemez (bu PR'da yeniden incelenmedi) | Bu PR: yankılanır (#140: teyit etmez) | Store'da varsa düşer | Aynı (tabanla) | **#140 ayrımı**; motor testi yalnız motorun bildiği birimi sınar |
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
(iddia 6). Vakum: FEC bekçisinde en az bir atlanan karar ön koşuldur (motor kısmı birimin işini
budadığında ters çevrilir). Mutasyon (yerel, commit edilmedi, yedekten geri alındı): (a) yolunda
başlık-only emisyon geri konunca FEC bekçisi kırmızı — 18 istemci datagramının 3'ü boş gövdeli DATA
(PN 15, 16, 17). Mutasyon yalnız parçalamasız DATA dalını sınadı. Parçalamalı dal (eski davranış:
11 B sıfır önekli, 22 B'lık DATA_FRAG) `store_miss_guard_fec_recovered_frag_cc_port` ile koşar:
`max_message_bytes` kurulu, atlanan karar ≥ 1 ön koşul, gövdesiz 0 (ilk koşuda `stale_skipped` 8);
o dal için mutasyon koşusu yapılmadı. (b) yolunu tetikleyen runtime testi yok (repair-store
tahliyesi ya da latch-0 bu fikstürlerde kurulmuyor); o yol kod okuması ve sayaç kimliğiyle
kapsanır.

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
kabulü; tam liste "Motor: teyitli birime karar yok" → "Değişen kararlar" D1–D10). Tel biçimi, golden
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
  makinede ön koşul kırmızısı (iddia değil) beklenebilir.
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
- Motor iptali (sahip kararı 2026-11-11), iki motor: `k4g05br_confirmed_unit_inflight_copy_never_requeued`
  (uçuş kopyası kayıp ilanı olmadan düşer; uçuş baytı ve `diag_pif` bırakılır; daha yeni seq ACK'i +
  30 s sonra karar, kayıp kuyruğu ve port motorunda tıkanıklık olayı yok),
  `k4g05br_confirmed_unit_dup_queue_cancelled` (PreConverge geometrik kopya; kopya ACK'iyle teyitte
  uçuştaki orijinal düşer), `k4g05br_recovered_control_retires_inflight_copies` (RECOVERED; BtlBw
  değişmez — N8); kuyruk kurtarmalı iki motor (`legacy+tail_v1`, `cc_port`):
  `k4g05br_confirmed_unit_tail_probe_cancelled` (jeton bekleyen prob iptal).
- İnceleme bulguları: `k4g05br_parity_ack_keeps_esi_namesake_copies` (iki motor),
  `k4g05br_tail_probe_retx_count_saturates` ve `k4g05br_tail_probe_never_targets_parity` (kuyruk
  kurtarmalı iki motor).
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

Mutasyon denetimi (yerel, commit edilmedi, yedekten geri alındı; `216206bc` üzerinde): eski çağrı-içi
düşürme geri konunca `k4g05br_confirmed_units_call_output_never_dropped` kırmızı (eski motor
kolunda tek çağrının ilk 37 teyidi düştü; test ilk ayrışmada durur). Sıralama + tekilleştirme
kaldırılınca üç test kırmızı: sıra (`k4g05br_inflight_ack_confirms_unit_ids_not_seqs`,
`k4g05br_recovered_control_confirms_listed_units`) ve olay içi tekil teyit
(`k4g05br_tombstone_late_ack_confirms_once_per_event`).

## #140'ta kalanlar

- Motor: `UnitOwnership` dizini, teyit kümesi ("birim oturumda en çok bir kez"), RTO_MAX tavanı
  (madde 3), sahiplenme adayları (madde 4), K_MAP, parite kaydı sahibi ve süre dolumu (madde 1).
  Satır 4, 6, 8b. Teyitli birime karar üretmeme (madde 2) bu PR'da `UnitOwnership` OLMADAN olay
  anında iptalle kapandı; satır 5'in kopya iptali bu PR'da kapandı.
- Kimlik değişikliği (`transport_v1_freeze.rs` düzenlemesi) — bu PR kimlik değiştirmez.
- Runtime testinin #140'taki parite faz 2'si ("esi adaşı kopyaları sürer, bölünme (r, k−r)") ve
  bekçilerdeki `stale_decisions == 0` iddiası: ikisi de motor kısmına bağlı.
- Aralık birleşmesinde ele alınacak iki ayrım: bu PR'ın teyit çıktısı olay içinde artan birim
  kimliği sırasıyla ve çağrı girişinde budanan birikimle çalışır (motor testleri bunu pinler);
  #140'ın `confirmed_out`'u kendi sırası ve tavanıyla çalışır. Runtime (`drain_confirmed!`) iki
  biçime de bağlı değildir.
