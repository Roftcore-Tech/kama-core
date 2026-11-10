# HB2-ALIM-YOLU — hub kipinde gelen oturum datagramının yolu (KK-H52 tasarım notu)

Durum: **(a) seçildi** (sahip kararı 2026-11-10). Kapsam: yalnız hub kipi (`DeviceConfig.hub` dolu);
doğrudan kip bit-eşit kalır. Uygulama `crates/client-core/src/hub/{shared_crypto,endpoint}.rs`
(dal `s2a/v3-hb2-endpoint`, PR `S2-A3-2a`). `runtime.rs` ve `transport.rs` farkı boş.

## Problem (değişmez şart)

Hub kipinde bir oturumun iki rolü (istemci ucu: ACK/RECOVERED/PARAM_ACK/CS_ECHO; veri alıcısı:
DATA/FEC/PARAM/DATA_FRAG/CS_PROBE) aynı `h` ile tek soketten gelir. Rol yalnız AEAD'den sonra,
çerçeve tipiyle bilinir; açma replay penceresini işler (`protector.rs:968`): datagram **tam bir kez**
açılır. Şart: **açılmış kayıt tüketici kabulüne kadar sahipliğini korur; kanal dolması, tüketici
durması, oturum kapanması yollarında sessizce kaybolmaz.** Görevlerin (`client_task`, `server_task`)
tek girdisi kendi UDP soketidir; `runtime.rs` değişmeden göreve başka giriş yolu yoktur.

## Sahiplik akışı

`HubCrypto<C>` S3 trait'lerini devreder; `Hub` kipinde süreç-içi, sınırlı kayıt kanalı
`OpenedStash` vardır (≤ 32 kayıt / `h`, ≤ 256 toplam; ayrılmış yuvalar dahil).

1. **Yer ayır** (`OpenedStash::reserve(h)`). Yer yoksa `None`: `HubEndpoint` datagramı **açmaz**,
   düşürür (`backpressure`). Kripto çağrılmaz; kayıp ağ kaybıyla eşdeğerdir, ARQ toparlar.
2. **Tek açma** (`HubCryptoAccess::open`). Açma başarısız ya da bağlam `Session(h)` değil →
   `cancel` (yuva geri) + sayaç (`hub_open_fail`, `hub_ctx_mismatch`); açılmış kayıt oluşmadı.
   Açıldı ama **tüketicisi yok** (tip T3 tablosunda yok; istemci rolü ama `h` için istemci ucu
   portu yok; veri alıcısı adresi verilmedi) → kayıt düşer ve sayılır (`hub_role_unknown`,
   `hub_no_client_port`, `hub_no_server_addr`). Bu yollar üç vakanın dışındadır: tüketici olmadığı
   için "kabule kadar sahiplik" uygulanamaz; ürün yolunda istemci rolü çerçevesi yalnız bu ucun
   gönderdiğine cevaptır, portu önceden öğrenilmiştir.
3. **Kanala taşı** (`fill`): `Opened` (`Clone` değil) artık kanalındır; tek sahibi kanaldır.
4. **Zil**: aynı baytlar yerel geri döngüden tüketicinin soketine gider. Zil sahiplik taşımaz; kayıp
   zil (alıcı tamponu dolu — çekirdek göndericiye bildirmez) kaydı etkilemez.
5. **Yeniden çal** (`due_rings`, 20 ms): tüketilmemiş kaydın zili, kayıt kanalda kaldıkça yinelenir.
6. **Kabul = tüketim**: görevin `seam.open(bytes)` çağrısı `HubCrypto::open`'a gelir; tam bayt
   eşleşmesiyle kayıt kanaldan **alınır** (`take`). Kripto ikinci kez çağrılmaz. Eşleşme yoksa
   (yinelenen zil, yabancı yerel datagram) `None` + `doorbell_dup`; `inner.open` çağrılmaz.
7. **Tüketimsiz çıkış yalnız iki yoldur ve sayılır** (`hub_stash_miss` = `StashCounters::lost()`):
   `Closed(h, _)` olayı `HubCrypto::poll_event`'ten geçerken o oturumun kayıtları
   (`dropped_closed`) · tüketici 10 s hiç almadı (`dropped_expired`, ölü tüketici).

## Üç vaka

| vaka | ne olur | kayıp | birim testi |
|---|---|---|---|
| **kanal dolması** | yeni datagram açılmadan düşer (adım 1); kanaldaki kayıtlar yerinde kalır; tüketici bir kaydı alınca yer açılır ve aynı datagram (ya da ARQ yeniden gönderimi) yine açılabilir | açılmış kayıp **0** (`backpressure` sayar) | `shared_crypto::tests::overflow_rejects_before_open_and_crypto_state_is_unchanged` · `endpoint::tests::full_stash_drops_before_open` |
| **duran tüketici** | kayıt kanalda kalır, zil 20 ms'de bir yeniden çalınır; tüketici dönünce kaydı alır. 10 s hiç almazsa ölü sayılır | durup dönen tüketicide **0**; ölü tüketicide `dropped_expired` | `shared_crypto::tests::stalled_consumer_keeps_the_record_and_it_is_rung_again` · `endpoint::tests::unconsumed_record_is_rung_again_without_a_second_open` · `shared_crypto::tests::dead_consumer_record_expires_and_is_counted` |
| **kapanan oturum** | `Closed(h)` geçerken `h`'nin kayıtları düşer; başka oturumun kaydı kalır | `dropped_closed` = düşen kayıt sayısı (tüketicisi yok) | `shared_crypto::tests::closed_session_drops_its_records_and_counts_them` |

Tek açma ve yinelenen zil: `shared_crypto::tests::hub_mode_opens_once_and_hands_the_record_over`,
`endpoint::tests::inbound_dispatch_opens_once_and_routes_by_frame_type` (sayaçlı sahte kripto:
datagram başına 1 `open`). Dumanda `hub_stash_miss = 0` ayrıca raporlanır; tek başına kanıt sayılmaz.

## Kilit sırası

Transport dış kilidi (`Mutex<HubCrypto<C>>`) tutarken `HubCrypto` iç kripto kilidini ya da kanal
kilidini alır, ikisini birlikte tutmaz. `HubEndpoint` dış kilidi almaz; iç kripto ve kanal
kilitlerini sırayla (birini bırakıp) alır. `HubLink` kilidi tutulurken kripto kilidi alınmaz.

## Seçilmedi: (b) — `runtime.rs`'e dar, eklemeli hub alım yolu

`client_task`/`server_task`'a hub kipinde soket yerine sınırlı `mpsc::Receiver<Opened>`'dan alan bir
kol ve iki yeni kurucu; `HubEndpoint` açmadan önce `try_reserve`. Motor dosyası, dondurma riski.
Sahip kararı: yalnız (a) şartı sağlayamazsa açılır (Paz 15 20:00'den önce, sahip MERGE).
