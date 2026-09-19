# KAMA TRANSPORT V1 — BENCHMARK METODOLOJİSİ

> Bu bir **sözleşme**dir, araştırma raporu değildir. §4.4 kıyas sayısı üreten her koşu bu
> belgeye uymak zorundadır. Belge **T0–T7'den ÖNCE** donar; kampanya sırasında değiştirilemez.
>
> Faz-4 kuralı: **bu belge hiçbir performans sonucu içermez.** İçindeki her sayı bir
> yapılandırma sabiti, bir eşik ya da bir enstrüman ölçümüdür — kıyas sonucu değildir.

Kapsam: `crates/bench` harness'ı. Otorite: `docs/specs/transport-layer-tdd.md` §4.4 ve
ADR-18. Çelişkide spec kazanır.

---

## 1. Kıyasın amacı

Kama v1'in **hostile ağlarda** (yüksek kayıp, düşük bant, yüksek gecikme, asimetrik yol)
uygulama verisini teslim etme yeteneğini, aynı ağda koşan yerleşik taşıyıcılarla kıyaslamak.

Birincil metrik: **deadline içinde teslim edilen uygulama mesajı oranı** (`completion(D)`).
İkincil: teslim edilen mesaj başına tel byte'ı, dönüş yolu (ACK) yükü, blackout (§11).

Kıyasın amacı **kazanmak değil, ölçmektir.** Ölçülmemiş hücre hakkında iddia kurulmaz.

---

## 2. Resmî Kama v1 profil kimliği

Tek kaynak: `KamaV1Profile` (`crates/client-core/src/runtime.rs`). `RuntimeConfig::default()`
bu profile devreder. Benchmark **elle `RuntimeConfig` kurmaz**; aynı constructor'ı çağırır.

Profilin kilitli alanları (`product_v1_profile_is_pinned` testi her birini pinler):

| alan | değer |
|---|---|
| `fec` | `Some(profile_id = 0x05, manual_r = None)` — adaptif, arbiter belirler |
| `max_unit_bytes` | `1024 − SOURCE_PREFIX_LEN` |
| `max_message_bytes` | `None` (fragmenter açık opt-in) |
| `initial_window_bytes` | `DEFAULT_BURST_CAP` |
| `structure_gate_level_permille` | `250` |
| `fec_loss_risk` | `bounded_v1` (L1/L2 cap 250‰, `reorder_suppress_permille = 300`, persistence P2) |
| `loss_confirm_horizon` | `CadenceLegacy` |
| `quota_r` | `4` |

**Kural:** resmî benchmark yolu bu constructor'ı kullanır. Bayraksız `--transport kama-product`
koşusu kanonik profildir.

---

## 3. Özel profil adlandırma

Kimlik **koddan türetilir**, operatör beyanından değil: `profile_delta(&rt_cfg)` çalışan
yapılandırmayı `KamaV1Profile` ile alan alan karşılaştırır.

```
sapma listesi boş  → arm = kama-v1-full
sapma listesi dolu → arm = kama-custom   + sapmaların tam listesi manifeste yazılır
```

Tek alan bile sapsa etiket `kama-custom` olur. `kama-v1-full` etiketiyle boş-olmayan sapma
listesi aynı manifeste **giremez** (`is_canonical_kama` çifte koruma; `fairness` komutu
`INVALID` verir).

§15 gereği belirsiz kol adı yasaktır. Rakip kolların adı test edilen primitifi taşır:
`quinn-reliable-stream`, `quinn-datagram-app-retry`, `tcp-default`, `kama-stub-legacy`.

---

## 4. Teslim anlamı (delivery semantics)

Tüm kollarda teslim olayı **uygulama düzeyindedir**. Transport-içi ACK teslim SAYILMAZ.

| kol | teslim tanımı |
|---|---|
| `kama-v1-full` | ilk teslim onayı (ACK veya FEC-RECOVERED) |
| `quinn-reliable-stream` | stream yazımının karşı tarafça onaylanması |
| `quinn-datagram-app-retry` | mesajı kapsayan ilk uygulama ACK'i |
| `tcp-default` | çerçevelenmiş mesajın uygulama ACK'inin göndericiye ulaşması |

Mükerrer teslim: ilk-ACK-kazanır (tüm kollarda). Latency = `delivered_ns − sent_ns`, aynı
mutlak saatten.

---

## 5. Güvenilirlik / sıra anlamı

| kol | güvenilirlik | sıra | retry sahibi |
|---|---|---|---|
| `kama-v1-full` | güvenilir mesaj | **yok** (mesaj granülü) | transport |
| `quinn-reliable-stream` | güvenilir sıralı akış | tam | transport |
| `quinn-datagram-app-retry` | datagram + uygulama retry | yok | uygulama |
| `tcp-default` | güvenilir sıralı akış | tam | transport |

**Anlamları eşitlemek YASAKTIR.** TCP'ye fazladan iş yüklemek de, Kama'ya sıra garantisi
eklemek de ölçümü bozar. Fark **kıyasın konusudur** ve raporda `comparison=non-normalized`
damgasıyla açıkça beyan edilir (§18).

---

## 6. Kurulum maliyeti politikası

**Tek ölçüm, iki görünüm — asla karıştırılmaz.**

Harness dört kolun tamamında aynı kuralı uygular: ölçüm saati bağlantı kurulumundan **sonra**
başlar; kurulum süresi ayrıca ölçülür ve `setup_ns` olarak manifeste yazılır.
`setup_in_window` alanı bu yapısal gerçeği taşır ve **kolun kodundan** gelir.

```
steady-state : kurulum pencere DIŞINDA (harness'ın ölçtüğü)
cold-flow    : uçtan uca, kurulum DAHİL  →  completion(D) = steady-state completion(D − setup_ns)
```

Türetme geçerlidir çünkü kurulum tüm akışı sabit `setup_ns` kadar öteler; her kol **kendi
ölçülen** maliyetini öder. Kama'ya olmayan bir kurulum maliyeti uydurulmaz: Kama'da zorunlu
veri-öncesi transport el sıkışması yoktur, `setup_ns`'i soket/endpoint kurulumudur.

`bench metrics --setup-view {steady-state|cold-flow}` görünümü seçer ve her çıktı satırı hangi
görünümde olduğunu yazar. `--setup-class` bayrağı ölçümü **değiştirmez**; harness'ın yapısal
gerçeğiyle karşılaştırılan bir iddiadır ve çelişirse koşu **manifest yazmadan** hata verir.

**Bilinen granülerlik sınırı:** `--deadlines-s` saniye tabanlıdır. Temiz yolda kurulum
maliyetleri milisaniye altındadır, dolayısıyla iki görünüm aynı sayıyı verir. Görünüm farkı
kurulumun pahalı olduğu yerde anlamlıdır (kayıplı yolda QUIC el sıkışması). Türetmenin
doğruluğu birim testiyle kilitlidir (`kurulum_gorunumu_simetrik_turetilir`), loopback
duman testiyle değil.

---

## 7. Quinn modları

İki mod ayrı kollardır ve ayrı adlandırılır:

* **`quinn-reliable-stream`** — mesaj başına stream; kurulum: `Endpoint::connect` + TLS/QUIC
  el sıkışması (`max_deadline` ile sınırlı); retry transport'a ait; ölçüm noktası stream
  yazımının onaylanması. **Kama'nın uygulama-düzeyi güvenilirlik hedefine en yakın mod
  budur; ana kıyas kolu bu olur.**
* **`quinn-datagram-app-retry`** — QUIC DATAGRAM + uygulama ARQ aynası; retry uygulamaya ait.

El sıkışma başarısızlığı/timeout **geçerli bir ölçüm sonucudur** (0 completion), harness
hatası değildir; agregaya girer, harcanan `setup_ns` yine raporlanır. 0-RTT kullanılmaz.

---

## 8. TCP modu

Platformun normal TCP yığını (`tokio::net::TcpStream`). Özel ARQ döngüsü **yoktur**.

| başlık | değer |
|---|---|
| soket | akış (SOCK_STREAM), IPv4/IPv6 |
| kurulum | `TcpStream::connect`, 20 s sınır |
| Nagle | `TCP_NODELAY = true` (tek ayar; başka egzotik tuning YOK) |
| tamponlama | çekirdek varsayılanı |
| çerçeveleme | `u32 len ‖ u64 msg_id` (12 B başlık), ACK = 8 B `msg_id` |
| yeniden birleştirme | uzunluk-öncelikli okuma (`read_exact`) |
| teslim tanımı | uygulama ACK'inin göndericiye ulaşması |
| bağlantı arızası | `connection-failure` sınıfı (§12), 0 completion |

Çerçeveleme minimum ve simetriktir; muhasebeye **dahildir**. TCP'nin mesaj sınırlarını
doğal olarak koruduğu iddia edilmez.

---

## 9. İleri / dönüş bozulma modeli

Asimetrik yol davranışı Kama'nın birincil hedefidir. Bu yüzden bozulma **tek bir alan
değildir**. Harness her yönü ayrı uygular (`kapi3_harness.sh`, üç netns) ve ayrı kaydeder:

```
impairment.forward : "delay 100ms loss 20% rate 1mbit"
impairment.reverse : "delay 100ms loss 2%  rate 1mbit"
```

Her yön için: kayıp, gecikme, reorder, bant/oran, burst/korelasyon modeli. `network = bad`
gibi belirsiz bir alan **yasaktır**. `fairness` komutu ileri ve dönüş yolunu **ayrı** kıyaslar;
biri farklıysa kıyas engellenir.

Tüm kollar aynı netem yapılandırmasını görür.

**Bilinen sınır:** `rate` disiplini jitter/reorder etkisini bastırır; reorder hedefli
hücrelerde `rate` kullanılmaz.

---

## 10. Tel maliyeti muhasebesi

Raporlanan kalemler: uygulama payload byte'ı · harness'ın gözleyebildiği toplam tel byte'ı ·
retransmit byte'ı · FEC/parity byte'ı · kontrol/ACK byte'ı · dahil edildiğinde kurulum byte'ı.

**BEYAN EDİLEN SINIR:** TCP kolunda harness yalnız **uygulama** byte'larını sayar. TCP'nin
çekirdek içi retransmit/ACK/başlık trafiği kullanıcı alanından görünmez. Bu yüzden TCP'nin
byte sayıları Kama/QUIC'in tam tel muhasebesiyle **doğrudan kıyaslanamaz**; manifest bu sınırı
`wire_accounting_note` alanında taşır ve `fairness` komutu farkı beyan eder.

Kama'nın parity/kontrol trafiğini sayıp rakibin transport ek yükünü dışlayarak "toplam maliyet"
demek **yasaktır**. Ölçülemeyen yerde hassasiyet uydurulmaz.

---

## 11. Blackout tanımı

```
blackout penceresi ⟺ (o pencerede tamamlanan uygulama teslimi < min_deliveries)
                   ∧ (o pencere boyunca gönderilmiş ama teslim edilmemiş mesaj VAR)
```

Varsayılan: pencere = 1 s, `min_deliveries` = 1. İkinci koşul zorunludur: akışın bittiği
sessizlik blackout değildir.

Raporlanan: `blackout: true/false` (ikili) · toplam blackout süresi · en uzun kesintisiz
blackout · kurtarma süresi (en uzun blackout'un başından ilk sonraki teslime). Kurtarma hiç
gelmezse süre **`n/a`** yazılır, 0 değil.

Amaç: felaket düzeyinde hostile başarısızlığın medyan completion sayısının arkasında
gizlenmesini engellemek. Bu tanım T0–T7'den **önce** donmuştur.

---

## 12. Arıza / timeout anlamı

Her koşu **tam olarak bir** sonuç sınıfına düşer:

| sınıf | anlamı | agregaya girer mi |
|---|---|---|
| `completed` | tüm mesajlar deadline içinde | evet |
| `deadline-miss` | akış çalıştı, bazı mesajlar geç | evet |
| `blackout` | en az bir blackout penceresi (§11) | evet |
| `timeout` | bağlantı kuruldu, teslim sıfır | evet |
| `connection-failure` | bağlantı/el sıkışma hiç kurulamadı | evet (transport gerçeği) |
| `harness-failure` | harness kendi hatasıyla düştü | **hayır** — ama sayısı raporlanır |
| `infra-failure` | runner/netns/ağ kurulumu arızası | **hayır** — ama sayısı raporlanır |

**Öncelik sırası** (bir koşu tam olarak bir sınıfa düşsün diye sabittir):
`harness-failure` → `connection-failure` → `timeout` → `blackout` → `deadline-miss` →
`completed`. Hiç teslim olmayan bir koşu `timeout`tur, `blackout` değil: kesinti değil, hiç
başlamama söz konusudur.

Timeout **etiketsiz sıfır teslime çevrilmez**. Harness arızası transport arızasıyla
karıştırılmaz. Ölçüm sayılmayan koşular dışlanır ama **sessizce silinmez**: insidansları her
çıktıda görünür.

---

## 13. Rastgelelik / yeniden üretilebilirlik

Her koşu kaydeder: `seed` · ağ profili (ileri + dönüş, aynen) · iteration id (`repeat`) ·
taşıyıcı · profil kimliği + sapma listesi · commit SHA.

Fuzzy controller ADR-17 gereği deterministiktir (tamsayı aritmetiği, RNG yok): aynı girdi →
aynı çıktı. İstatistik katmanı da RNG kullanmaz (bootstrap yeniden-örnekleme **yok**;
aralıklar kapalı-form/bisection).

**Bilinen sınır:** netem'in kayıp/reorder üreteci bit-bit yeniden üretilemez. Bu yüzden tek
koşu kanıt değildir; §15 kuralı bunun için vardır. Rastgelelik tek medyanın arkasına
saklanmaz — dağılım ve aralık her zaman birlikte raporlanır.

---

## 14. İstatistik politikası

Anahtar metrikler için asgari: `n` · merkez · dağılım · güven aralığı · stall/arıza insidansı.

Merkez **medyan**, dağılım **IQR**'dir. Gaussian varsayılmaz: completion dağılımı hostile
hücrelerde patolojiktir (0 ve 1'de yığılma). Medyanın güven aralığı **sıra istatistiğiyle**
(dağılımdan bağımsız) hesaplanır; oranlar için **kesin (Clopper–Pearson)** aralık kullanılır —
küçük n'de normal yaklaşım yanıltıcıdır.

`n < 6` iken medyan aralığı tüm örneği kapsar ve bilgi taşımaz: `CI=n/a(n<6)` yazılır,
uydurulmaz.

---

## 15. N ≥ 30 / güven aralığı kuralı

```
n < 30  → claim class = smoke   → harness/anlam doğrulama. PERFORMANS İDDİASI YASAK.
n ≥ 30  → claim class = final   → güven aralığıyla birlikte iddia yapılabilir.
```

Sınıf `n`'den **otomatik** türetilir; operatör yükseltemez. Başka bir örneklem boyutu ancak
**yazılı istatistiksel gerekçeyle** kullanılır. n=3/n=5 medyanı nihai hostile üstünlük kanıtı
olarak kullanılamaz.

---

## 16. Stall işlemesi

Üç sayı **birlikte** raporlanır, hiçbiri tek başına değil:

```
all-runs       : tüm ölçüm koşuları (stall dahil)
stall-free     : yalnız stall'sız koşular
stall incidence: stall / ölçüm-koşusu  (+ kesin güven aralığı)
```

Stall = teslimin tamamen durduğu koşu (`blackout` · `timeout` · `connection-failure`).
Başarısız/stall'lı koşu **sessizce silinmez**; ölçüm sayılmayan koşular ayrıca gösterilir.

---

## 17. Manifest şemaları

Şema kimliği: `kama-bench-manifest/v1`. Her koşu tek JSON dosyası yazar; iki bölüm taşır.

**Yapılandırma (ölçümden ÖNCE):** `commit` · `arm` · `transport` · `profile_delta` ·
`semantics{reliability, ordering, delivery_definition, duplicate_handling, retry_owner}` ·
`setup_class` (türetilmiş) · `setup_ns` · `impairment{forward, reverse}` · `payload_bytes` ·
`msg_count` · `deadline_ns` · `submit_interval_ns` · `seed` · `repeat` ·
`wire_accounting_note`.

**Aktivasyon (ölçümden SONRA, Faz-3 tanılarından):** `tail_configured` · `fec_configured` ·
`tail_probes` · `tail_cancelled` · `parity_emitted` · `parity_token_denied` · `retransmits` ·
`dups` · `state_transitions` · `recover_arq_entries` · `left_preconverge_ns` · `btlbw_bps` ·
`srtt_us` · `rtprop_us` · `return_ack_loss_permille` · `return_forward_loss_permille` ·
`bytes_data` · `bytes_retx` · `bytes_parity`.

Aktivasyon bölümü **yapılandırıldı ≠ ulaşılabilir ≠ ateşledi** ayrımını korur: bir mekanizmanın
config'de açık olması onun koştuğu anlamına gelmez.

`parity_token_denied` sayacı manifestte **taşınır**; Faz-4'te yorumlanmaz. "Koruyucu tahkim mi,
zararlı parity açlığı mı" sorusu Faz-5 T2'ye aittir.

---

## 18. Geçersiz kıyas kuralları

`bench fairness --manifests A.json B.json ...` her çifti denetler. **Üç düzey:**

| düzey | tetikleyen fark | sonuç | çıkış |
|---|---|---|---|
| `BLOCKING` | deney koşulu: yük, mesaj sayısı, deadline, sunum aralığı, ileri bozulma, dönüş bozulması, kurulum sınıfı | kıyas sayısı **üretilemez** | 2 |
| `NON-NORMALIZED` | anlam: güvenilirlik, sıra, teslim tanımı, retry sahibi, tel-muhasebesi sınırı | sayı üretilebilir, `comparison=non-normalized` damgası **zorunlu** | 1 |
| `NORMALIZED` | fark yok | tam normalize kıyas | 0 |

Ayrıca her manifest tek başına denetlenir: `kama-v1-full` etiketi boş-olmayan sapma listesiyle
birlikte `INVALID` verir.

Denetimin yakaladığı örnekler: Kama güvenilir ↔ Quinn DATAGRAM (non-normalized) · Kama soğuk
akış ↔ önceden kurulmuş QUIC (blocking) · Kama v1 ↔ elle sakatlanmış `kama-custom` (invalid).

Bu denetim rapor okuyucusunun dikkatine bırakılmaz; araç **kendisi** işaretler.

---

## 19. ADR-18 / nightly entegrasyonu

Gecelik hostile job'ı (`.github/workflows/nightly.yml`):

* `NETEM_RUNNER` repo değişkeni olmadan **kuyruğa hiç girmez** (24 saatlik asılı kalmanın kök
  nedeni buydu),
* rejim başına **ayrı job + ayrı timeout** (sharded, timeout-bounded, attributable),
* **kanonik `KamaV1Profile`'ı** kurar (elle yeniden yazılmış sevk config'i yok),
* yapılandırma **ve** aktivasyon manifestini üretir,
* sapma varsa `BENCHMARK_FAILURE` ile "Kama v1" etiketini **reddeder**.

Faz-4'ün sorumluluğu metodolojinin doğru olması; hostile kampanyayı **başlatmak değil**.
Gerçek T0–T7 koşumu Faz-5'te etkinleşir.

---

## 20. Faz-4 kapanış hükmü

Bu belge donduğunda benchmark yolu şunu garanti eder:

1. Resmî kıyas yolu kanonik ürün profilini kullanır ve sapma gizlenemez.
2. Teslim/güvenilirlik/sıra anlamı her kolda açıkça yazılıdır ve eşitlenmez.
3. Kurulum maliyeti simetriktir; iki görünüm karıştırılamaz.
4. İleri ve dönüş bozulmaları ayrı uygulanır ve ayrı kaydedilir.
5. Blackout, arıza ve timeout sınıfları ölçümden önce tanımlıdır.
6. İddia sınıfı örneklem boyutundan otomatik türetilir; n<30 performans iddiası yapamaz.
7. Geçersiz kıyaslar araç tarafından işaretlenir.

Bu belge **performans sonucu içermez** ve içeremez. Sonuçlar Faz-5 T0–T7 kampanyasında,
bu sözleşmeye uyarak üretilir.
