# K4G-05g — sahte `Ready` — kısa not (DUR-05g-1 (a) uygulaması + DUR-05g-2 + DUR-05g-3 (B))

## 0. Künye

**K4G-05g** (CEVAP 14 §5; CEVAP 15 §3; CEVAP 16 §2). Dal `k4g/05g-sahte-ready`, taban main `1b1425d2`. Kod: `9c0273e4`, `0049f5d9`, `978180a5` (DUR-05g-3 (B)) · testler: `0b216f07`, `9fee9b77`, `ee7fabb4`, `0e54e31e` · S1 metni (Ek A): `b0eed05f`. Ek A'daki `<ONAY-TARİHİ>` sahibin onay turu-2 mesajının tarihidir; o mesaj gelmeden Ek A merge edilmez. Satır atıfları bu dalın uçundandır (Ek B). Bu şeritte cargo koşulmadı; sayılar orkestratörün ölçümünden gelir.

**Onay koşulu (CEVAP 16 §2).** "K4G-05g ONAYLANDI" ancak #140 merge'ünden SONRA main üzerine yeniden tabanlanmış GERÇEK merge adayında, tam yerel kapı (1.96.0, `--locked`) yeşil olduğunda ve sonuçlar o SHA'ya bağlandığında gelir. Ek A hash'inin değişmemesi kodun doğrulandığı anlamına GELMEZ: Ek A yalnız S1 K7 metnidir. (B) ile Ek A DEĞİŞMEDİ; K7 yaması `b0eed05f`'teki hâliyle aynıdır. Yeniden tabanlamayı orkestratör yapar.

## 1. Kol başına davranış (DUR-05g-1 (a) sonrası)

| Kol | Application'ın bildiği | S1 olayı | Crypto oturumu |
|---|---|---|---|
| bekleyen niyet (başlatan) | bilet | `ConnectAbandoned(bilet, LocalPermanent)` | yeni `Close` |
| yanıtlayan, glare kaybı | bilet | `ConnectAbandoned(bilet, LocalPermanent)` | yeni `Close` (+ kayıp bekleyen `Close`, olay yok) |
| yanıtlayan, niyet yok | hiçbir şey | yok (S1 K2/K7) | yeni `Close` |
| yenileme cutover'ı (yanıt tutuldu) | eski tutamaç | yok; uç `AcceptResponse`'tan ÖNCE açılır, açılamazsa yanıt tutulur, eski oturum tam çalışır | yenileme sürer |
| yenileme yedek kolu (uç önceden açılmadı) | eski tutamaç | `Closed(eski, Superseded)`, `Ready` yok | yeni + eski `Close` |
| provisional onayı | eski tutamaç | `Closed(eski, Superseded)`, `Ready` yok | yeni + eski `Close` |

Ek olarak `connect` (ve kalıcı niyetin taze INIT'i) INIT'ten ÖNCE yerel soketi sınar (§3, DUR-05g-3 (B)). Sınama düşerse INIT yola çıkmaz; niyet `ConnectAbandoned(bilet, LocalPermanent)` ile biter. Sınama geçer ama açılış düşerse yukarıdaki kollar aynen çalışır.

Son iki kol S1 K7'nin TEK istisnasıdır (Ek A). Kapanış `close` ile aynı yoldan geçer: uç bırakılır, sıralama durumu silinir, eski oturumun bekleyen yenilemesi kapatılır, Crypto'nun eski oturum için vereceği tek `Closed` yutulur. S1'e tam BİR `Closed` gider (`abandon_successor`). `activate_without_endpoint` kalktı. Uçsuz `Ready` üreten kod yolu yok: `activate` açılmış ucu parametre olarak alır. Enum'lar değişmedi. Application tarafı nedenden bağımsızdır: kama-app `Messenger::retire` eşin oturumunu kaldırır, kama-node `note_down` eş için `NodeEvent::Closed { reason: Session(Superseded) }` yayımlar.

## 2. Karşı ucun durumu — gerçek durum makinesinden (DUR-05g-2 (1))

**Kendi bakışında kurulu mu?** Başlatan, glare, yanıtlayan ve provisional kollarında evet: karşı uç el sıkışmayı tamamlamış ve yeni oturum için `Ready` vermiştir.
- Karşı uç yanıtlayansa `cmd_accept_init` `EstablishedAndReply` ile `Ready`'yi INIT'i kabul ettiği anda yayımlar (`session_control.rs:849`).
- Karşı uç başlatansa `cmd_accept_response` `Ready` yayımlar (`:920`).

Yenileme yedek kolunda ise karşı uçtaki yeni oturum onaysız bir provisional'dır ve Application'a görünmez; `Ready` almış olan, eski oturumdur.

Karşı uçtaki kayıt Crypto'nun "yarım açık" kaydı DEĞİLDİR. O kayıt bekleyen bir INIT'tir (`HALF_OPEN_TTL`); burada ise kurulu bir `MetaState::Active` oturum vardır. Bu notta "yarım açık oturum", karşılığı bizde kapanmış kurulu oturum anlamına gelir. Telde kapanış mesajı yok, karşı uç bunu bilmez. Karşı ucun bu oturumda yolladığı her datagram bizde `open` → `None` ile sessizce düşer (karşılık gelen Crypto oturumu yok), ACK almaz. ARQ teyide dek sürer (`transport.rs` modül belgesi, "Teslim sırası").

| Kol | Karşı uçtaki kayıt |
|---|---|
| başlatan, glare kaybı, adressiz yanıtlayan | bir `Active` (ölü) |
| provisional onayı | karşı ucun yeni oturumu `Active` (ölü); kendi eskisi yenilemeyse kendi drain'inde kapanır |
| yenileme yedek kolu | eski `Active` (artık ölü) + bir ONAYSIZ provisional |
| başarısız yeniden bağlanma (karşı uçta zaten `Active` varken) | bir ONAYSIZ provisional |

**Bitiş mekanizması ve sınırı.**
- **Ölü `Active`.** Hiçbir zamanlayıcı bitirmez. INV-NO-IDLE-TEARDOWN (`specs/transport-layer-tdd.md:53-58`) der ki: "Bir oturum ya da bekleyen niyet, ZAMAN GEÇTİĞİ İÇİN sonlandırılmaz". Bitiren olaylar şunlardır:
  - (i) **Bizim sıradaki INIT'imiz (yeniden `connect`).** Karşı uçta provisional doğar. Bizim ilk epoch-1 paketimiz onu onaylar (`:418-422`). Ölü oturum drain'e düşer ve `Closed(_, Superseded)` ile biter. Sınır: `GRACE_N_G` = 64 paket ya da `GRACE_T_G` = 30 s, hangisi önce (`epoch_machine.rs:57,61`). `T_g` onay anında başlar (`start_drain`, `:524`; denetim `:447`).
  - (ii) **Karşı ucun yaş yenilemesi.** `SESSION_MAX_AGE` = 7 gün (`:87`), oturumun kurulduğu andan (`created_at`) sayılır: yanıtlayanda AcceptInit (`:806`), başlatanda AcceptResponse (`:907`). Süre dolunca bir kez `RenewRequired` üretilir (`:457`). Sonrası:
    - Biz erişilebilirsek karşı ucun INIT'ine bizim taraf taze oturumla yanıt verir. Karşı uç ölü oturumun teyitsiz birimleri için `renew_drain_cap` kadar bekler; bu süre ürün `drain_cap` değeri, 3 s (`runtime.rs:269`). Ardından cutover olur ve eski oturum `Closed(_, Superseded)` ile biter.
    - Erişilemezsek yenileme INIT'i `HALF_OPEN_TTL` = 7 gün sonra düşer (`:476`). `age_renew_signalled` tek seferliktir, ölü oturum kalır.

    `SESSION_MAX_AGE` bir yaş, yani anahtar hijyeni tetikleyicisidir; boşta kalma süresi değildir. Sonucu ölüm değil yenilemedir. Bu yüzden INV-NO-IDLE-TEARDOWN ile çelişmez.
  - (iii) Karşı ucun uygulamasının `close`'u ya da yeniden başlaması.
  - (iv) `SEND_PN_CAP` = 2^31 (`protector.rs:42`) → tek `RenewRequired`; pratikte ulaşılmaz.

  Sonuç: ölü oturumun BİZDEN BAĞIMSIZ bir üst süre sınırı yoktur. Biz erişilebilir oldukça en geç `SESSION_MAX_AGE` + bir el sıkışması + `renew_drain_cap`/`GRACE` içinde biter.
- **Onaysız provisional.** `PROVISIONAL_TTL` = 7 gün (`:85`, `HALF_OPEN_TTL` ile aynı değer). Süre INIT'in kabul edildiği andan (`created_at`, `:806`) başlar. Dolunca kayıt SESSİZ düşer (`drop_silently`, `:451-453`; olay yok). Provisional bir S1 oturumu değildir, Application'a görünmez; bu yüzden TTL'si INV-NO-IDLE-TEARDOWN'ın kapsamına girmez.
- **Karşı ucun maliyeti.** Oturum başına en çok `max_inflight_units` = 4096 teyitsiz birim ve `offer_queue` = 1024 (`runtime.rs:259-260`). Dolunca `send` `Backpressure` döner (S1 K8: Application bekler). RECEIPT gelmez; kama-app kaydı outbox'ta tutar ve eşin sıradaki oturumunda aynı `msg_id`'yle yollar. kama-node testi (a) bunu uçtan uca gösterir.

## 3. Kaynak sınırı (DUR-05g-2 (2))

- **Başarısız aktivasyon başına karşı uçta EN ÇOK BİR yeni kayıt.** Her aktivasyon tek bir el sıkışmasıdır: bir INIT, bir RESPONSE. Karşı uçta etkin oturum yoksa bu bir `Active`, varsa bir onaysız provisional doğurur (`:769`). Yenileme yedek kolunda karşı uçtaki eski `Active` zaten vardı; yeni kayıt yalnız provisional'dır.
- **Bizim tarafımızda kayıt kalmaz.**
  - Crypto oturumu kapatılır (zombi yok; testlerde `session_epoch(..) == None`).
  - Uç ve sıralama durumu yoktur.
  - Niyet `ConnectAbandoned` ile biter.
  - client-core kendiliğinden yeniden DENEMEZ. Yeni el sıkışması yalnız Application'ın `connect`'iyle ya da karşı ucun INIT'iyle başlar.
  - Yenilemenin uç açma denemesi her `drive`'da tekrarlanır ama YENİ INIT üretmez: Crypto'ya hep aynı tutulan RESPONSE verilir (`release_held_renewals`).
- **Eş başına üst sınır (karşı uçta).**
  - `Active`: eş başına en çok bir (`active_session_with`, `:246`). Yeni INIT, etkin oturum varken provisional olur.
  - Onaysız provisional: Crypto'da SABİT TAVAN YOK. `sessions` haritası sınırsızdır; yalnız önbellekli yanıtlar eş başına `HALF_OPEN_CAP` = 64 ile sınırlı (`:80`, `:824`). Sayı, bizim `PROVISIONAL_TTL` (7 gün) içinde başarısız olan yeniden bağlanma denemelerimizin sayısıdır. Üretim hızı Application'ın `connect` hızına bağlıdır.
- **Temizlenme koşulu.**
  - Ölü `Active`: bizim sıradaki BAŞARILI bağlanmamız (onay → drain → kapanış, `GRACE` içinde) ya da karşı ucun yenilemesi.
  - Onaysız provisional: yalnız `PROVISIONAL_TTL`. Başka bir provisional'ın onayı onu temizlemez.
- **DUR-05g-3.** Kalıcı bir yerel uç hatası altında Application'ın her yeniden bağlanma denemesi karşı uçta bir provisional bırakır; eş başına tavan yoktur. Sahip kararı (CEVAP 16 §2):
  - **(B) bu kartta, yardımcı önlem olarak uygulandı** (`978180a5`). `connect` ve `reinitiate`, INIT'ten ÖNCE ucun açılışındaki AYNI soket kurulumunu sınar: ortak yardımcı `runtime::bind_peer_socket` (bind + connect), uç görevi açılmadan; kopya yok. Sınama düşerse INIT yola çıkmaz, Crypto `Initiate` çağrılmaz, karşı uçta kayıt oluşmaz. Niyet `ConnectAbandoned(_, LocalPermanent)` ile biter; sayaç `local_probe_failures`. S1, tel ve enum değişmedi.
  - **(B)'nin sınırı: (B) başarısız girişimleri azaltır ama kaynak sınırı DEĞİLDİR.** Sınama ile gerçek açılış arasında koşul değişebilir (o zaman §1'deki açılış hatası kolları aynen çalışır; sınama onların yerine geçmez). Ayrıca (B) gelen INIT yollarını (yanıtlayan, provisional) ve yenileme INIT'ini kapsamaz. Karşı uçtaki provisional sayısına üst sınır koymaz.
  - **(A) ayrı kart:** kama-noise-wire'da eş başına onaysız provisional tavanı ve temizleme politikası (`HALF_OPEN_CAP` kalıbı; KS-DOS-1 sınıfı). "INT-02 GEÇTİ"den ÖNCE kapanır. Taslak: dal `k4g/05g-a-provisional-tavan`, not `docs/design/K4G-05g-A-PROVISIONAL-TAVAN.md`; kart adı sahibin.
  - **(C) tek başına REDDEDİLDİ:** INT-02 ölçümü ek olarak kalır (§5).

## 4. Testler (DUR-05g-1 (a) ve DUR-05g-2 (3))

client-core (`crates/client-core/tests/k4g05g_endpoint_open_failure.rs`, 6 test; ilk 4'ü `0b216f07`'de, değişmedi):
- **(5) Provisional onayı:**
  - `Ready(b1)` yok; tam bir `Closed(b0, Superseded)` gelir, `T_g` geçse de ikincisi gelmez.
  - `b1` ve `b0` Crypto'da kapalı; sayaç 1.
  - Bob `connect` → `Pending` → `Ready(b2)`; erken metin teslim edilir.
- **(6) Yenileme yedek kolu:**
  - Düzenek: ham eş RESPONSE'u yabancı bir hub tutamacıyla yollar; adaptör yanıtı tutmaz, uç önceden açılmaz.
  - `Ready(h′)` yok; tam bir `Closed(a0, Superseded)`; tek deneme.
  - `connect` → `Pending` → `Ready(a2)`; yeni uç Bob'un yeni oturumu bağlamında mühürler.

kama-node (`crates/kama-node/src/node/tests/endpoint_failure.rs`; iki elle sürülen ürün düğümü):
- **(a) Provisional kolu:**
  - Alice yeniden başlar ve bağlanır. Bob'un onaydaki ucu açılamaz → Bob `NodeEvent::Closed { alice, Session(Superseded) }` görür, `Connected` görmez.
  - Alice'te yarım açık oturum dururken Bob'un yeniden `connect`'i BAŞARIR: iki yönde metin ve alındı geliyor. Alice'in yarım açık oturumda kaybolan metni yeni oturumda teslim ediliyor.
- **(b) Başlatan kolu:**
  - Alice `Closed { bob, Abandoned(LocalPermanent) }` görür.
  - Bob'daki yarım açık oturum dururken Alice'in yeniden `connect`'i BAŞARIR; Bob'da `Closed` yok.

**DUR-05g-3 (B)** (aynı client-core dosyası, 4 yeni test; mevcut 6 test değişmedi):
- **(7) Kalıcı yerel soket hatası:**
  - INIT yok: röle ve bütün kutular boş, karşı uca hiçbir zarf gitmedi, Bob'da olay yok.
  - `ConnectAbandoned(t1, LocalPermanent)`; `local_probe_failures` 1, `endpoint_open_failures` 0; Crypto oturumu hiç açılmadı.
  - Hata kalkınca `connect` → `Pending` → `Ready`; erken metin tek kez teslim edilir.
- **(8) Sınama geçer, açılış düşer:** INIT yola çıkar (röle 1). `Ready` yok; tam bir `ConnectAbandoned`. Sınama sayacı 0, açılış sayacı 1; Crypto oturumu kapalı.
- **(9) Normal yol:** `Measuring` → `Ready`; iki sayaç 0; iki yönde veri.
- **(10) Kalıcı niyetin taze INIT'i:** `HALF_OPEN_TTL` sonrası sınama düşer. Taze INIT yok (rölede yalnız ilk INIT); `ConnectAbandoned`.

**2×2 beklentisi (DUR-8 biçimi).** Satırlar kod, sütunlar test. "Yeni kod" = `0e54e31e`. "Eski kod" = (B)'siz `c9c2ea22`; yeni testler orada derlenmez (`set_socket_probe_failures_for_test` / `local_probe_failures` yok). Bu yüzden eski davranış yeni kodun üstüne mutasyonla geri getirilir.

| Kod \ test | (7) | (8) | (9) | (10) | mevcut 6 test + kama-node 2 |
|---|---|---|---|---|---|
| yeni kod | Y | Y | Y | Y | Y (değişmedi) |
| M8: `connect` sınamayı atlar | **K** — INIT gider, ikinci `next_event` WAIT içinde gelmez | Y | Y | Y | Y |
| M9: `reinitiate` sınamayı atlar | Y | Y | Y | **K** — taze INIT gider, `ConnectAbandoned` gelmez | Y |
| M10: `connect`'te sınama düşünce olay yok | **K** — ikinci `next_event` gelmez | Y | Y | Y | Y |
| M1: açılış kolunda uçsuz `Ready` | Y | **K** — ikinci olay `Ready` | Y | Y | initiator, responder, glare, kama-node (b) K |

(K = kırmızı, Y = yeşil.) (9) mutasyonlarla kırmızıya dönmez; normal yolun değişmediğinin kontrol kolu odur. Mutasyon dosyaları `~/kama-k4g-work/k4g05g/mutasyon/M8…M10`.

## 5. INT-02 (DUR-05g-2 (4))

Ölçüm ek olarak kalır (hüküm değil, raporlanır):
- `endpoint_open_failures` ve `local_probe_failures`;
- karşı uçta ölü `Active` ve onaysız provisional sayısı ile ömrü;
- yeniden bağlanmadan karşı ucun ölü oturumunun kapanışına kadar geçen süre (beklenen ≤ `GRACE_T_G`).

## 6. Beyanlar

- **`LocalPermanent`:** soket kurulum hatası geçici olabilir. Kapalı listede uyan başka neden yoktur ve niyeti açık tutmak kalıcı hatada sınırsız el sıkışma döngüsü kurar. Adres eksikliğinde `connect` bu nedeni zaten kullanıyordu. (B)'nin sınama hatası da aynı nedenle biter: `connect`'in belgelenmiş hata kümesinde (`EarlyUnitConflict`, `UnitTooLarge`, `Backpressure`) yerel soket hatasının karşılığı yoktur; yeni hata ya da olay eklenmedi.
- **(B) kapsamı:** `connect` ve `reinitiate` (kalıcı niyetin `HALF_OPEN_TTL` sonrası taze INIT'i). Yenileme INIT'i sınanmaz: yenilemenin uç hatası, uç `AcceptResponse`'tan önce açıldığı için eski oturumu bozmaz ve yenileme başına karşı uçta en çok bir provisional bırakır.
- **(B) ve runtime.rs:** yalnız `connect_session_counted`'ın soket kurulumu `bind_peer_socket`'a taşındı (aynı iki çağrı). 05b'nin (#140) runtime.rs hunk'larıyla örtüşmüyor.
- **Yenileme yeniden denemesi:** uç açma denemesi her `drive`'da bir kez sayılır, yani sayaç hızlı artabilir.
- **Testsiz savunma dalı:** taze ucun erken birimi reddetmesi (uç görevi açılır açılmaz ölürse).
- **Bayat belge cümleleri (davranış değişmedi):**
  - `crates/kama-contracts/src/handles.rs` `CloseReason::Superseded` belgesi ("yeni oturum eskisinin yerini aldı") FROZEN'dır ve dokunulmadı; istisna S1 K7 metnindedir (Ek A).
  - `crates/kama-app/src/ptt.rs:89` belgesindeki "güncel oturumu kalmadıysa (`UserClosed`, `PeerClosed` ya da `KeyHygiene`)" cümlesi istisnadan sonra `Superseded`'i de kapsamalıdır. Davranış oturum tabanlıdır (`session_of(..).is_none()`), değişmez.

## Ek A — S1 metin yaması (DUR-05g-1 (a); `b0eed05f`)

Yalnız ekleme; üstü çizili metin yok. Sahip kararı CEVAP 15 §3 donmuş `s1.rs` belge metninin ve S1 sözleşme belgesinin değiştirilmesine yazılı izindir. `s1.rs`'te kod, enum ve imza değişmez; trait bloğu, `TransportError`, `CloseReason` ve `AbandonReason` kümeleri aynıdır. `<ONAY-TARİHİ>` onay turu-2 mesajının tarihidir. Yerel kapılar (`b0eed05f`): `scripts/ci/word_coverage.py --base 1b1425d2` YEŞİL (kaybolan kelime 0, yerinde korunmayan satır 0); `scripts/ci/seam-api-drift.py --seam s1` YEŞİL. Kapsam sınırı: istisna yalnız bu hata kolu içindir; K4G-06 akış izolasyonunun S1 değişikliklerinin yerine geçmez.

Sahibin metni (iki dosyada aynen):

> Yerine geçen oturumun (provisional onayı ya da yenileme cutover'ı) yerel ucu açılamazsa: yeni oturum için `Ready` YAYIMLANMAZ; yeni Crypto oturumu kapatılır; eski oturum (Crypto'da zaten drain'de, geri alınamaz) `Closed(eski, Superseded)` ile biter. K7'nin "`Ready(h′)` → … → `Closed(h, Superseded)`" sırasına TEK istisna budur: `Closed(h, Superseded)` etkin bir ardıl olmadan gelebilir. Application `Closed(_, Superseded)` aldığında etkin ardıl yoksa eşi bağlantısız sayar ve yeniden `connect` eder.

Uygulanan fark:

```diff
diff --git a/crates/kama-contracts/src/s1.rs b/crates/kama-contracts/src/s1.rs
index 0066ff77..a06f0577 100644
--- a/crates/kama-contracts/src/s1.rs
+++ b/crates/kama-contracts/src/s1.rs
@@ -75,6 +75,15 @@ pub struct Delivered {
 }
 
 /// §1 K7 — olaylar typed'dır; Transport yorumlamadan iletir.
+///
+/// **[YAMA KAYDI — <ONAY-TARİHİ> · K4G-05g DUR-05g-1 (a), sahip kararı CEVAP PAKETİ 15 §3 — K7'nin
+/// TEK istisnası]** Yerine geçen oturumun (provisional onayı ya da yenileme cutover'ı) yerel ucu
+/// açılamazsa: yeni oturum için `Ready` YAYIMLANMAZ; yeni Crypto oturumu kapatılır; eski oturum
+/// (Crypto'da zaten drain'de, geri alınamaz) `Closed(eski, Superseded)` ile biter. K7'nin
+/// "`Ready(h′)` → … → `Closed(h, Superseded)`" sırasına TEK istisna budur: `Closed(h, Superseded)`
+/// etkin bir ardıl olmadan gelebilir. Application `Closed(_, Superseded)` aldığında etkin ardıl
+/// yoksa eşi bağlantısız sayar ve yeniden `connect` eder. Kapsam yalnız bu hata koludur; enum'lar
+/// ve imzalar değişmedi.
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum SessionEvent {
     Ready(SessionHandle, PeerRef),
diff --git a/docs/seams/KM-SEAM-AT-001.md b/docs/seams/KM-SEAM-AT-001.md
index 037ceeab..a6ed0a20 100644
--- a/docs/seams/KM-SEAM-AT-001.md
+++ b/docs/seams/KM-SEAM-AT-001.md
@@ -6,6 +6,8 @@
 **Türetildiği kaynaklar (icat yok):** transport-tdd §0 INV-NEIGHBOR-1 / INV-PLAINTEXT-NOT-SEMANTICS · KM-SEAM-TC-001 (S3) §1.2 SessionControl/SessionEvent, K7, K8, K9 · app-rpc §2 (üç düzlem), AMENDMENT-7 (envelope_type) · application-message-protocol.md §1–§2, §5 · engine-spec §0.2 · ccr §35.6 BatchPayload (KARAR-1) · identity (multi-device limit=1)
 **Yazım:** her karar **K** · **G** (kaynak atfıyla) · **R** (reddedilen alternatif) · **S** (sınır).
 
+> **YAMA KAYDI — <ONAY-TARİHİ> [K4G-05g — DUR-05g-1 (a), Abkarada kararı CEVAP PAKETİ 15 §3 (2026-10-25) — S1 K7'nin TEK istisnası; `docs/design/K4G-05g-SAHTE-READY.md` Ek A]:** §1 K7'ye, provisional supersession maddesinin ardına istisna maddesi eklendi: yerine geçen oturumun (provisional onayı ya da yenileme cutover'ı) yerel ucu açılamazsa yeni oturum için `Ready` yayımlanmaz, yeni Crypto oturumu kapatılır, eski oturum `Closed(eski, Superseded)` ile biter — etkin bir ardıl olmadan; Application eşi bağlantısız sayar ve yeniden `connect` eder. Kapsam yalnız bu hata kolu; K4G-06 akış izolasyonunun gerektireceği S1 değişikliklerinin yerine geçmez. Trait bloğu, `TransportError`, `CloseReason` ve `AbandonReason` kümeleri DEĞİŞMEDİ; `crates/kama-contracts/src/s1.rs`'te yalnız `SessionEvent` belge metnine aynı istisna yazıldı. Üstü çizili metin yok.
+
 > **YAMA KAYDI — 2026-10-08 [S1 — K6 `early_unit` taşıması, Abkarada kararı "S1 early_unit: iş emri geçerli … + S1 K6 YAMA"; L-T1]:** K6'ya bu sprintin taşıma kuralı: Transport `early_unit`'i S3 `Initiate`'e VERMEZ (`early_unit: None`), oturum `Ready` olunca **ilk normal `send()`** olarak yollar; `ConnectAbandoned`'da birim yollanmamıştır. `max_early_unit_bytes` bu sprint INIT zarfından değil birim tavanından türer (birim normal veri birimi olarak gider). INIT'e gömülü 0-RTT taşıma sonraki sprint. Trait imzaları ve `s1.rs` DEĞİŞMEDİ. Üstü çizili metin silinmedi.
 
 > **YAMA KAYDI — 2026-10-06c [L-F07 artıklar-2 F07.1/F07.2/F07.5 — Abkarada kararı]:** F07.1: `close` artık `Result<(), TransportError>` döner — §1 trait satırı (eski satır fence içinde üstü çizili yorum olarak korundu), §3 F0.2(i) paragrafına `Err(SessionSuperseded)` eklemesi, §4 F0.2(iii) maddesinde "hata dönmez" / "hata vermez" üstü çizildi: `Ok(())` ⟺ tam BİR `Closed(h, UserClosed)`; bilinmeyen ya da zaten kapatılmış tutamaç → `UnknownSession`, supersede penceresindeki tutamaç → `SessionSuperseded`, ikisinde de olay yok; §6 H-EVENTS ve H-CUTOVER hücrelerine tarihli ekler. F07.2: §1 `PeerRef` yorumunda "(bugün CI kapısı yok)" üstü çizildi — kama-app kaynağında `as_bytes` çağrısı layer_check C6 ile yasak. F07.5: §8 ⬛ İMZA satırı — "imza bekliyor (Hayri)" üstü çizildi; imza: Abkarada, 2026-10-06 (Hayri'nin kalemleri bu sprint Abkarada'ya devredildi; iş emri 2026-10-06 §0). `TransportError` kümesi ve diğer trait metotları DEĞİŞMEDİ; aynı PR'da `crates/kama-contracts/src/s1.rs` aynı imzayı taşır (drift kapısı). **G-S1: bu değişiklik main'e girince FROZEN (iş emri 2026-10-06 §8 ön-yetki); kama-contracts s1 ve handles.rs kilitlenir.** Üstü çizili metin silinmedi.
@@ -113,6 +115,7 @@ Cutover sonrası `send(h)` → **`SessionSuperseded`** (yapısal hata; oracle de
 **S:** Olay sırası bir session için garanti: `Ready` → (`RenewRequired`)* → `Closed`. `Ready` gelmeden `send(h)` → `UnknownSession`.
 ~~**S — Peer-restart supersession [DR-CONNECT-1 H6]:** Mevcut oturumu olan bilinen bir eşten doğrulanmış YENİ bir INIT gelirse (eş durumunu kaybetmiş; identity §16) olay sırası: **`Ready(h', peer)` → `Closed(h, Superseded)`**. Sonrasında `send(h)` → `SessionSuperseded`. `h` üzerinde teslim edilmemiş birimleri Transport yeni oturuma TAŞIMAZ; Application `msg_id` ile `h'` üzerinden yeniden gönderir (bu retry değil, yeni oturumda yeni gönderimdir — K8'in "app retry yapmaz" kuralı aynı oturum içindir).~~ *[2026-10-04: AŞILDI — aşağıdaki provisional kuralla değişti.]*
 **S — Supersession, PROVISIONAL [DR-CONNECT-1 H6, 2026-10-04]:** Mevcut oturumu (`h`) olan bilinen bir eşten doğrulanmış, yeni `init_counter`'lı bir INIT gelirse yeni oturum provisional kurulur; **provisional dönem Application'a GÖRÜNMEZ** ve `h` tam çalışır (`send(h)` kabul). Olay sırası: **`Ready(h', peer)`** (ONAYDA = `h'` üzerinde ilk AEAD-geçerli epoch-1 paketi) **→ `h` yalnız-alım drain** (KS-GRACE-1: `N_g = 64` / `T_g = 30 s`, hangisi önce; bu sürede `recv()` `h`'den teslim edebilir) **→ `Closed(h, Superseded)`**. `Ready(h')` anından itibaren `send(h)` → `SessionSuperseded`. `h` üzerinde teslim edilmemiş birimleri Transport yeni oturuma TAŞIMAZ; Application `msg_id` ile `h'` üzerinden yeniden gönderir (alıcıda `msg_id` dedup güvenli kılar; bu aynı-oturum retry'ı değildir, K8 korunur). Onaylanmayan provisional oturum hiçbir olay üretmeden düşer; `h`'ye dokunulmaz. Alıcı Renew ile peer-restart'ı ayırt etmez — ikisi bu yoldan geçer (identity §16).
+**S — K7'nin TEK istisnası [<ONAY-TARİHİ> — K4G-05g, DUR-05g-1 (a), CEVAP PAKETİ 15 §3]:** Yerine geçen oturumun (provisional onayı ya da yenileme cutover'ı) yerel ucu açılamazsa: yeni oturum için `Ready` YAYIMLANMAZ; yeni Crypto oturumu kapatılır; eski oturum (Crypto'da zaten drain'de, geri alınamaz) `Closed(eski, Superseded)` ile biter. K7'nin "`Ready(h′)` → … → `Closed(h, Superseded)`" sırasına TEK istisna budur: `Closed(h, Superseded)` etkin bir ardıl olmadan gelebilir. Application `Closed(_, Superseded)` aldığında etkin ardıl yoksa eşi bağlantısız sayar ve yeniden `connect` eder. Kapsam sınırı: yalnız bu hata kolu (K4G-06 akış izolasyonunun S1 değişikliklerinin yerine geçmez).
 **S — Glare [DR-CONNECT-1, 2026-10-04]:** iki taraf aynı anda `connect` ederse (identity §16 tie-break: `dest_hash`'i küçük olan kazanır) tek oturum doğar ve her iki Application da TEK `Ready(h, peer)` görür; kaybeden tarafın `Pending(ticket)`'i geçerli kalır ve o `Ready` ile çözülür.
 
 **K8 — Application teslimi Transport'tan öğrenmez; receipt AMP semantiğidir.**
```

## Ek B — atıflar (dal ucu)

- `crates/client-core/src/transport.rs`: `on_incoming_init` :814 · `release_held_renewals` :874 · `on_crypto_ready` :925 · `on_confirmed` :990 · `on_crypto_closed` :1050 · `reinitiate` :1109 (sınama :1112) · `activate` :1153 · `abort_activation` :1202 · `abandon_successor` :1220 · `local_socket_ok` :1251 · `connect` :1476 (sınama :1524). `crates/client-core/src/runtime.rs`: `bind_peer_socket` :1544.
- `crates/kama-noise-wire/src/session_control.rs`: `HALF_OPEN_CAP` :80 · `HALF_OPEN_TTL` :83 · `PROVISIONAL_TTL` :85 · `SESSION_MAX_AGE` :87 · `active_session_with` :246 · onay `Confirmed` :418-422 · `tick_meta` :442 (drain `T_g` :447, provisional TTL :451-453, yaş :457) · bekleyen INIT TTL :476 · `start_drain` :524 · `end_session` :543 · provisional mi etkin mi :769 · yanıtlayan `created_at` :806 · önbellek tavanı :824 · yanıtlayan `Ready` :849 · başlatan `created_at` :907, `Ready` :920, eski drain :922 · `cmd_close` :991.
- `crates/kama-noise-wire/src/epoch_machine.rs`: `GRACE_N_G` :57 · `GRACE_T_G` :61. `crates/kama-noise-wire/src/protector.rs`: `SEND_PN_CAP` :42.
- `crates/client-core/src/runtime.rs` ürün varsayılanı: `max_inflight_units` 4096 :259 · `offer_queue` 1024 :260 · `drain_cap` 3 s :269.
- `crates/kama-node/src/node.rs`: `note_down` :602. `crates/kama-app/src/messaging.rs`: `on_session_event` `Closed` → `retire` (nedenden bağımsız). `specs/transport-layer-tdd.md` INV-NO-IDLE-TEARDOWN :53-58.
