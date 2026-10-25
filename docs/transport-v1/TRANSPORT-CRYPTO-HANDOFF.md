# Transport v1 → Güvenlik Katmanı DEVİR SINIRI

> Bu belge **kripto mimarisi DEĞİLDİR.** Algoritma seçmez, el sıkışma tasarlamaz, hiçbir şey
> uygulamaz. Yalnız transport'un gelecek güvenlik katmanına NE VERDİĞİNİ ve NE VARSAYDIĞINI
> sabitler. Sahip invaryantı (2026-09-02): security = transport'un MÜŞTERİSİ; çift yönlü körlük;
> güvenlik takılıp-çıkarılabilir; seam dar kalır (INV-016, INV-ENGINE-SOC).

Kaynak: `crates/client-core/src/runtime.rs`, `crates/kama-transport-schema/src/lib.rs`,
`crates/adaptive-transport/src/{receiver,fec,ack}.rs`. Kod okunarak yazıldı (2026-09-11).

> **YAMA KAYDI — 2026-10-25 [K4G-05c — Abkarada kararı CEVAP PAKETİ 10, 2026-10-24, DUR-05c-9 (a) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §10.1 "kuyruklar" satırı: dolu teslim kuyruğunda birim artık düşmez, ACK'siz reddedilir; offer kuyruğu ayrıca birim-kimliği mesafesiyle durur. Üstü çizili metin silinmedi.

## 10.1 Üst-katman birimi

| | v1 |
|---|---|
| sunulan birim | opak `Vec<u8>` mesaj — `ClientSender::send(payload)` / `send_with_deadline(payload, deadline_ns: Option<u64>)` |
| granülerlik | **mesaj = birim** (fragmenter kapalı): her mesaj tek DATA çerçevesi, tek `unit_id` |
| azami boyut | `max_message_bytes` = `max_unit_bytes` = **1 013 B** (`1024 − SOURCE_PREFIX_LEN 11`); aşarsa `RuntimeError::UnitTooLarge{got,cap}` |
| fragmentasyon | `max_message_bytes: Some(n)` ile açık opt-in (v1'de KAPALI); açılırsa mesaj 1 013 B'lik chunk'lara bölünür, her chunk kendi birim-id'si/ARQ/FEC yuvası, alıcı birleştirir — **tel formatı görünür değişir** |
| teslim olayı | alıcıda `Delivery{peer: SocketAddr, unit_id: u64, payload: Vec<u8>}`; göndericide `acked() -> Option<u64>` (ilk-ACK-kazanır birim id akışı) |
| kuyruklar | bounded: `offer_queue 1024` (dolu → `QueueFull`; **[2026-10-25 — K4G-05c]** en yeni ile en eski teyitsiz birim arasındaki kimlik mesafesi `max_inflight_units`'e ulaşınca offer alınmaz, kuyruk dolar), `delivery_queue 1024` ~~(taşan teslim SAYILIR, sessiz düşmez)~~ **[2026-10-25 — K4G-05c]** (dolu → birim kanala girmez ve ACK'lenmez, `full_refusals` sayılır; teslim düşmez, `delivery_dropped` yapısal 0) |

## 10.2 Sıra

**Transport v1 SIRA GARANTİSİ VERMEZ.** `RxOutput.delivered` belgesi: "in-order garanti YOK —
E2E envelope sıralar." Mesajlar bağımsız kurtarılır (ARQ/FEC), teslim sırası varış sırasıdır.
Üst katman sıraya ihtiyaç duyuyorsa kendi sıra numarasını taşır. Bu, hostile üstünlüğün bir
kısmının kaynağıdır (head-of-line yok; INV-011) ve v1'de değiştirilmez.

## 10.3 Güvenilirlik

| | v1 |
|---|---|
| teslim semantiği | güvenilir mesaj: deadline içinde ARQ (+ FEC) ile kurtarılır; `send_with_deadline` bütçesi aşılırsa birim terk edilebilir (motor birimi kendi kendine terk etmez — drain `drain_cap` ile sınırlı) |
| mükerrer | alıcı `RX_DUP_SUPPRESS`: aynı birim ikinci kez gelirse DÜŞÜRÜLÜR + ACK teyit edilir (`RxOutput.was_duplicate`); üst katmana **en fazla bir kez** teslim (pencere-içi); pencere-dışı çok-geç kopya teorik → nihai dedup üst katmanda (E2E envelope) |
| yeniden iletim görünürlüğü | üst katmana GÖRÜNMEZ: retransmit/tail-probe/parity transport-içi; `ClientStats`/`ActivationDiag` yalnız gözlem |
| gönderici onayı | `acked()` = ilk ACK (WireAck) **veya** FEC-RECOVERED teyidi (`ConfirmSource::{WireAck, Recovered}`); Recovered kredi üretmez |

## 10.4 İlk-veri semantiği

* **Zorunlu ön-DATA transport el sıkışması YOKTUR** (INV-010). `connect_with_policy` UDP soket +
  endpoint kurar (ölçülen `setup_ns` ≈ 40–60 µs), ilk mesaj hemen sunulabilir; sender PreConverge'de
  ilk pencere (2 400 B) kadar orijinal DATA'yı ACK beklemeden gönderir.
* PreConverge çıkışı ilk geçerli SACK'le (RTT örneği) olur; çıkana dek FEC parity ve
  recovery DONDURULUR (KURULUŞ-GUARD), kuruluş probu (DR-EST-1) token-muaf çalışır.
* Transport göndermeden önce ŞUNU VARSAYAR: hedef `SocketAddr` bilinir; **kimlik, anahtar,
  oturum kurulumu varsaymaz.** İlk korunmuş payload'ın teorik olarak ilk datagramda
  sunulabilmesi, güvenlik katmanının 0-RTT/1-RTT kararına bağlıdır — bu belge o kararı VERMEZ.
* Sınır (LIM-001): ilk uçuşta hiç ACK dönmezse transport ilerlemez; güvenlik katmanı ilk
  korunmuş veriyi buna dayanarak "ulaştı" sayamaz.

## 10.5 Metadata sınırı

Payload etrafında bugün var olan transport alanları (kaynak: transport-schema §2.6 + motor çerçeveleri):

| alan | üreten | tel-görünür | sınıflandırma (kavramsal; uygulanmadı) |
|---|---|---|---|
| `unit_id`/`seq` (u64, gönderici-monoton) | transport | evet | **sonunda kimlik-doğrulanmalı** (replay/yeniden-sıralama saldırı yüzeyi) |
| DATA çerçeve tipi (0x00), ACK (0x10), ACK_V2 (0x14), FEC_SHARD (0x20) | transport | evet | kimlik-doğrulanmalı (tip karıştırma) |
| `SOURCE_PREFIX` 11 B: `message_id 4 + chunk_offset 4 + actual_payload_len 2 + source_flags 1` | transport | evet (FEC source-symbol içinde) | kimlik-doğrulanmalı (uzunluk/ofset bütünlüğü) |
| `LogicalShardHeader{kind, profile_id u8, block_id u32, esi u16, k_actual u8}` | transport | evet | kimlik-doğrulanmalı (INV-FEC-1: authenticate-BEFORE-decode) |
| ACK_V2 gövdesi: kümülatif + ≤16 aralık, `seen_count`, `ack_delay` (256 µs birim) | transport (alıcı) | evet | kimlik-doğrulanmalı (sahte ACK = kayıp gizleme) |
| `ack_epoch` alanı (çözülüyor, TÜKETİLMİYOR) | transport | evet | güvenlik katmanı rekey/wrap epoch'una bağlanacak — v1'de kablosuz (audit tema E) |
| `EXTENDED_OUTER_OVERHEAD` 26 B: `h_ext 4 + wire_format 1 + wire_flags 1 + pn_trunc 4 + tag 16` | **güvenlik katmanı** (noise-wire) | evet | güvenlik katmanının kendisi; transport OPAK görür |
| pacer/estimator durumları, `ActivationDiag` | transport | **hayır** (yerel) | yerel kalır |
| `setup_ns`, manifest alanları | bench | hayır | yerel/rapor |

**Bugün ürün yolunda** `IdentitySeam` (kimlik dönüşümü, +16 B stub tag) çalışır; gerçek mühür
noise-wire'a aittir ve transport ona MÜŞTERİDİR. Transport hiçbir alanı kendisi doğrulamaz.

## 10.6 Birim boyu / ek yük bütçesi

```
FEC source-symbol E               = 1 024 B  (profil 0x05)
  = SOURCE_PREFIX 11 + payload ≤ 1 013 + zero_pad
sealed FEC shard (EXTENDED)       = 26 (outer) + 11 + 1 024 = 1 061 B  < 1 280 B PMTU tabanı
legacy SESSION_DATA overhead      = 22 B (hub_handle 4 + seq 2 + tag 16)
DATA çerçevesi (v0 shell)         ≈ 26 B başlık + payload   (T0 ölçümü: 283 B/256 B mesaj)
MTU varsayımı                     = 1 200 B (DEFAULT_MTU), burst cap 2 400 B
```
Güvenlik ek yükü (tag, nonce/pn, başlık) **payload tavanının içinden** ödenir: `max_unit_bytes`
= `E − SOURCE_PREFIX_LEN` sabittir; güvenlik katmanı kendi ek yükünü uygulama payload'ından
düşmek ya da E'yi (profil) değiştirmek zorundadır — ikincisi profil kimliğini değiştirir
(EVOLUTION §9.5). Fragmenter açılırsa her chunk'a +11 B. AEAD/tag/sertifika boyutları BURADA
SEÇİLMEZ.

## 10.7 Kimlik varsayımları

| | transport v1 |
|---|---|
| eş kimliği | **OPAK**: eş = `SocketAddr` (server eş tablosu `max_peers 64`, fail-closed); kimlik doğrulama YOK |
| oturum kimliği | transport-içi: PreConverge→Startup geçişi "kurulmuş" sayılır; kriptografik oturum kavramı YOK |
| yol kimliği | tek yol, göç yok; adres değişimi = yeni eş |
| bağlantı kimliği | UDP 5-tuple; `hub_handle` (legacy 4 B) taşınır ama transport yorumlamaz |
| rekey/epoch | `ack_epoch` çözülür, tüketilmez; epoch semantiği güvenlik katmanınındır |

Transport, eşin kim olduğunu BİLMEZ ve bilmek zorunda değildir; sahte-eş/kimlik-hırsızlığı
güvenlik katmanının işidir.

## 10.8 Replay / mükerrer sınırı

Transport dedup'u **kayıp-kurtarma hijyenidir, güvenlik değil**: `RX_DUP_SUPPRESS` yalnız
alıcı penceresi içindeki aynı-`unit_id` kopyayı düşürür; pencere dışı (çok geç, yeniden
başlatma sonrası, farklı eş adresinden) kopyalar ayırt EDİLMEZ; `unit_id` gönderici-monoton ama
kimlik-doğrulanmamış. Güvenlik katmanı transport dedup'undan ŞUNU ÇIKARAMAZ: "bu payload daha
önce görülmedi." Replay koruması burada UYGULANMAZ; `pn_trunc`/`ack_epoch` alanları bunun için
ayrılmıştır (tema E, post-v1).

## 10.9 Arıza yayılımı

| olay | üst katmana nasıl görünür |
|---|---|
| kuyruk dolu / endpoint kapalı / birim büyük | `send*` → `Err(RuntimeError::{QueueFull, Closed, UnitTooLarge})` (senkron) |
| teslim başarısızlığı (deadline aşımı, kalıcı kayıp) | `acked()` o `unit_id`'yi ÜRETMEZ; açık "başarısız" olayı YOK — üst katman deadline'ı kendi izler |
| yol bozulması | `ClientStats.fate.return_path` (health Unknown/Healthy/Degraded/Dead, fwd/ret loss‰) ve `ActivationDiag` — **salt gözlem**, olay kanalı değil |
| kuruluş başarısızlığı (sıfır-ACK) | ayrımsız: `left_preconverge_ns = 0`, `acked()` sessiz (LIM-001) |
| I/O | `RuntimeError::Io` |
| kripto arızası | transport'ta KARŞILIĞI YOK — güvenlik katmanı açılmayan datagramı transport'a "kayıp" gibi mi, "saldırı" gibi mi göstereceğine kendi karar verir; transport bugün açılamayan paketi görmez (IdentitySeam stub) |

Gelecek güvenlik işi şu üçünü ayırt edebilmelidir: transport teslim arızası (`acked` yok) ·
yol bozulması (`return_path` health) · kriptografik arıza (transport'un yüzeyinde yok, güvenlik
katmanı tanımlar). Bu belge yalnız arayüzü kaydeder.
