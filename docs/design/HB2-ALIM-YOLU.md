# HB2-ALIM-YOLU — hub kipinde gelen oturum datagramının yolu (KK-H52 tasarım notu)

Durum: **öneri**, sahip onayı bekliyor (KK-H52 sahip kararı 2026-11-10, EK-A §10). Kapsam: yalnız
hub kipi (`DeviceConfig.hub` dolu); doğrudan kip bit-eşit kalır. Atıflar `origin/main @ dbea1ef3`.

## Problem (değişmez şart)

Hub kipinde bir oturumun iki rolü (istemci ucu: ACK/RECOVERED/PARAM_ACK/CS_ECHO; sunucu ucu:
DATA/FEC/PARAM/DATA_FRAG/CS_PROBE) aynı `h` ile tek soketten gelir. Rol yalnız AEAD'den sonra,
çerçeve tipiyle bilinir; açma replay penceresini işler (`protector.rs:968`), yani datagram **tam bir
kez** açılır ve açılmış kaydın tek sahibi olmalıdır. Şart: **açılmış kayıt tüketici kabulüne kadar
sahipliğini korur; kanal dolması, tüketici durması, oturum kapanması yollarında sessizce kaybolmaz.**
Görevlerin (`client_task` `runtime.rs:2162`, `server_task` `:3438`) tek girdisi kendi UDP soketidir
(`recv` → `seam.open`); `runtime.rs` değişmeden göreve başka bir giriş yolu yoktur.

## Seçenek (a) — tek-açma önbelleği, sahiplik süreç-içi kanalda, yerel UDP yalnız "zil"

`HubCrypto<C>` (`hub/shared_crypto.rs`) S3 trait'lerini devreder; `Hub` kipinde içinde **sınırlı,
sahipli bir kayıt kanalı** (`OpenedStash`) vardır. Akış:

1. **Önce yer ayır, sonra aç.** Hub soketinden `h ≠ 0` datagram gelir. `HubEndpoint`, `h` için ve
   küresel olarak boş yuva var mı bakar (`≤ 32 / h`, `≤ 256` toplam). Yoksa datagram **açılmadan**
   düşer (`hub_rx_backpressure`): kripto durumu değişmez (parmak izi testiyle), kayıp ağ kaybıyla
   eşdeğerdir, ARQ toparlar. Böylece "açılmış ama sahipsiz" kayıt hiç oluşmaz.
2. Yer varsa **tek açma** (`inner.open`), bağlam `Session(h)` → tip → rol; kayıt yuvaya **taşınır**
   (`Opened`, `Clone` değil). Açma hatası / bağlam uyumsuz / tip bilinmez → yuva serbest + sayaç.
3. **Zil:** aynı baytlar yerel geri döngüden tüketicinin soketine yollanır. Zil yalnız uyandırır,
   **sahiplik taşımaz**: zil kaybolursa (alıcı soket tamponu dolu — çekirdek bunu göndericiye
   bildirmez) kayıt yuvada kalır ve `HubEndpoint` tüketilmemiş kaydı **yeniden çalar** (20 ms
   aralıkla, kayıt yaşadıkça).
4. **Kabul = tüketim.** Görevin `seam.open(bytes)` çağrısı `HubCrypto::open`'a gelir; tam bayt
   eşleşmesiyle kayıt yuvadan **alınır** ve döner. Kripto ikinci kez çağrılmaz. Eşleşme yoksa
   (yinelenen zil, yabancı yerel datagram) `None` + `hub_doorbell_dup`; `inner.open` **çağrılmaz**.
5. **Yuvadan tüketimsiz çıkış yalnız iki yolla ve sayılır** (`hub_stash_miss` = toplam):
   - `Closed(h, _)` olayı `HubCrypto::poll_event`'ten geçerken `h`'nin kayıtları düşer
     (`stash_dropped_closed`) — oturum yok, kaydın tüketicisi de yok;
   - tüketici ölü (oturum kapanmadan 10 s tüketim yok): `stash_dropped_expired`.
   Kanal dolması kaybı **yoktur** (1. adım açmadan önce reddeder).

Birim testleri (ayrı ayrı; dumanda sayaç 0 yeterli sayılmaz): **taşma** (dolu yuvada açma çağrısı
yok, kripto parmak izi aynı, sayaç +1) · **duran tüketici** (zil düşürülür, kayıt yeniden çalınır,
tüketici dönünce kayıp 0) · **kapanan oturum** (kayıtlar düşer, `stash_dropped_closed` = kayıt sayısı)
· **ölü tüketici** (sahte saat, 10 s) · tek açma (sayaçlı kripto: datagram başına 1 `open`) ·
yinelenen zil (ikinci `seam.open` `None`, kripto çağrılmaz).

Bedel: `runtime.rs` farkı **boş**, `transport.rs` yalnız `+` (`session_epoch` için ek `impl`).
Geri döngü soketleri beyanlı uygulama ayrıntısıdır; Aralık'ta SAAT-G/Ç `DatagramIo` ile kalkar.

## Seçenek (b) — `runtime.rs`'e dar, eklemeli hub alım yolu

`client_task` ve `server_task`'a hub kipinde soket yerine sınırlı `tokio::mpsc::Receiver<Opened>`'dan
alan bir `select!` kolu ve iki yeni kurucu (`connect_session_hub`, `bind_expecting_hub`) eklenir;
`HubEndpoint` açmadan önce iki rolün kanalında `try_reserve` ile izin alır, açınca kaydı izinle
taşır. Sahiplik kanalın kendisindedir; zil ve yeniden çalma yoktur. Bedel: motor dosyası
(`runtime.rs`, ≈ +120 satır, iki görev döngüsü), **Paz 15 20:00'den önce** main'de, sahip MERGE;
dondurma penceresinde motor riski; `transport.rs`'te de kurucu seçimi için `+` satır.

## Öneri

**(a).** Şartı açmadan-önce-yer-ayırma ve zil/sahiplik ayrımıyla sağlar, motor dosyasına dokunmaz,
dondurmayı ve L yolunu etkilemez. Açık nokta (sahip kararı): (a)'da "süreç-içi kanal" kaydın
sahipliğini taşır, yerel UDP yalnız zildir; `runtime.rs` sıfır farkla zilsiz bir yol yoktur.
Sahip (b)'yi seçerse `HubEndpoint` aynı kalır, yalnız teslim ucu değişir.
