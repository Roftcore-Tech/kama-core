# INT-02 M1/M2 teşhis ölçümü — ölçüt ve beklenti kaydı (CEVAP PAKETİ 6)

| Alan | Değer |
|---|---|
| Kart | K4G-05 (L-INT) — teşhis ölçümü; K4G-05b tasarım notuna girdi |
| Sahip talebi | CEVAP PAKETİ 6 (2026-10-18): "bayat zamanlayıcı SACK'leri ve 28 B/s hız tabanı notta ölçümle gelsin, karar o zaman." |
| Durum | **ÖLÇÜMDEN ÖNCE yazıldı.** Ölçümden sonra bu dosya DEĞİŞTİRİLMEZ; sapma, bu dosyaya dokunmadan, ölçüm notunda dekompozisyonla yazılır. `int02_analyze.py` ölçütleri §4 bloğundan okur ve bu dosyanın sha256'sını çıktıya yazar. |
| Nitelik | Teşhis. Kapı DEĞİL, hüküm YOK. Motor düzeltmesi ÖNCESİ kod: `crates/adaptive-transport/**` bu ölçüm için değişmedi. |
| Kaynak | `int02_node` şema `int02-node-v3`: `m_log` (oturum başına ACK çerçevesi + motor durum değişikliği günlüğü), `timeline.samples[].<düğüm>.sessions[].motor` (1 s örnek). Analiz: `scripts/netem/int02_analyze.py` M1 / M2 bölümleri. |

## 1. Ölçüm düzeni

- Profil INT-02 ile aynı: `d200j200l30` = netem `delay 200ms 200ms loss 30% seed <tohum>`, `lo` üzerinde (her paket her yönde bir kez).
- Koşular: `gating2` tohum 3 ve 9; `ek-kesinti` tohum 1 ve 3; `--window 600 --timeline-every-ms 1000`.
- Örnekleme: zaman çizelgesi 1 s; düğüm sondası (m_log toplama) 250 ms. ACK çerçevesi ve durum değişikliği anları sonda aralığından BAĞIMSIZDIR: kayıt, olayın işlendiği anda istemci görevinde damgalanır, sonda yalnız toplar. Canlı aynanın halkası 32 kayıttır; iki sonda arasında daha fazlası gelirse kaybolan kayıt sayısı `acks_missed` / `states_missed` alanında görünür (gizlenmez).
- Durum değişikliği çözünürlüğü: istemci görevinin bir ana döngü turu (bir olay). Bir turda birden çok geçiş varsa ara durumlar kayıtta yoktur; `motor_transitions` farkı bunu gösterir.

## 2. M1 — bayat zamanlayıcı SACK'leri: tanımlar

- **Gönderen ucu**: bir `m_log.sessions[]` günlüğü (`side` = ACK'i alan gönderen düğüm, `peer` = alıcı). `gating2`'de asıl akış bob → alice (bob günlüğü); alice → bob yönünde yalnız alındılar (RECEIPT) akar, ayrı satırdır.
- **Çerçeve sınıfı** (her ACK kaydı, günlük sırasıyla):
  - **P (ilerlemeli)**: `new_units > 0` — çerçeve en az bir birimi İLK kez teyit etti.
  - **S (bayat)**: `new_units = 0` VE çerçeve V2 VE günlükteki bir önceki V2 kayda göre `seen` değişmedi (alıcı iki ACK üretimi arasında hiç yeni datagram görmedi). Önceki V2 kayıt yoksa S değil N.
  - **N (ilerlemesiz, yeni datagramlı)**: `new_units = 0` ve S değil (alıcı yeni datagram gördü — ör. teslim edilmiş birimin kopyası — ya da `seen` geriledi: dönüş yolunda sıra değişimi).
- **Motorun çerçeveyi işleyişi**: `mapped > 0` = motora seq listesi gitti; `last_ack_now` = motor son-ACK anını bu çerçeveyle tazeledi (ACK sessizliği sıfırlandı). Motorun kendi ilerleme tanımı (tail zamanlayıcısının yeniden kurulması): `tail_rearmed_now` ∧ `after.tail_probe_index = 0` ∧ `after.tail_probes = before.tail_probes` → "motor: ilerleme"; `tail_rearmed_now` ama aynı olayda tail-probe da çıktıysa → "motor: belirsiz".
- **Varış aralığı**: günlükte ardışık (sıra numarası farkı 1) iki çerçeveden İKİNCİSİ S ise, gönderendeki varış anları farkı Δ. Sınıf: k = yuvarla(Δ / `ACK_CADENCE_FLOOR_MS`), k ≥ 1 ve |Δ − k · FLOOR| ≤ `ARRIVAL_TOL_MS` → "taban×k"; değilse aynı sınama `T_SPACING_MS` ile → "kota×j"; ikisi de değilse "diğer". (FLOOR = 4 · T_SPACING olduğundan taban önce sınanır.)
- **Alıcıdaki üretim aralığı**: aynı `largest_pn`'i taşıyan ve ikisi de `ack_delay_raw < ACK_DELAY_SATURATED_RAW` olan ardışık iki S kaydı için Δ(`ack_delay`) = alıcının iki SACK üretimi arasındaki süre (yol gecikmesinden bağımsız). Sınıf: |Δ − k · FLOOR| ≤ `EMISSION_TOL_MS` → "taban×k", değilse aynı sınama T_SPACING ile, değilse "diğer".
- **İlerlemesiz dizi**: günlükte ardışık P-olmayan (S ya da N) çerçevelerin en uzun koşusu; en az `MIN_STREAK_FRAMES` çerçeve. Başı = ilk çerçevenin anı; sonu = diziyi kapatan P çerçevesinin anı; kapatan P yoksa "açık" ve süre son çerçeveye dek ölçülür. "Son S" = dizinin son S çerçevesinin anı.
- **RecoverFwd girişi**: durum günlüğünde `to = RecoverFwd` (ve `from ≠ RecoverFwd`). Konum: diziden önceki P çerçevesi ile dizinin ilk çerçevesi arasında → "dizi öncesi sessizlikte"; dizinin başı ile son S'si arasında → "bayat SACK'ler sürerken"; son S ile diziyi kapatan P arasında (açık dizide son S'den sonra) → "son bayat SACK'ten sonra, ilerlemeden önce"; hiçbiri değilse → "dizi dışı". Her girişte: son ACK'ten beri (`t_ms − last_ack_ms`), `ack_confidence_permille`, `emergency`, `fusion_emergency_mode` ve yol: seviye > `Arq` → "seviye"; füzyon kipi acil → "füzyon"; ikisi → "seviye+füzyon".
- **Dizide RecoverFwd payı**: dizi süresinin, durum günlüğüne göre RecoverFwd'de geçen kesri.
- **Eski kayıt doğrulaması**: eski teşhiste `gating2` s9 bob ACK anları `OLD_S9_BOB_ACK_S` (zaman tabanı eski kayıtta belirsiz → yalnız ARALIKLAR kıyaslanır). Aralıklar aynı varış sınamasıyla sınıflanır; bu koşunun s9 bob S aralıklarıyla yan yana yazılır. "bob takılmanın çoğunda RecoverFwd'deydi" iddiası, s9 bob dizilerinin RecoverFwd payıyla sınanır.

## 3. M2 — kesinti sonrası hız tabanı: tanımlar

- **Seri**: `ek-kesinti` zaman çizelgesi örnekleri (1 s), gönderen alice'in bob oturumu: `motor.pacer_rate_bps`, `motor.btlbw_bps`, `motor.tx_datagrams`, `motor.tx_bytes`, `motor.state`. bob → alice (alındılar) ayrı satırdır.
- **Evreler** (`t0`'a göre ms): ÖNCE = [`connect_requested_ms`, `outage.applied_ms`) içindeki ve `state ≠ PreConverge` örnekler; SIRASI = [`applied_ms`, `restored_ms`]; SONRA = (`restored_ms`, son örnek].
- **Her evrede**: örnek sayısı; pacer ve BtlBw medyan (en yakın sıra) / en küçük / en büyük; tabanda örnek oranı (`pacer_rate_bps ≤ MIN_RATE_BPS`); emisyon/s = Δ`tx_datagrams` / Δt ve bayt/s = Δ`tx_bytes` / Δt (evrenin ilk ve son örneği arasında); durum dağılımı.
- **Toparlanma** (pacer): SONRA evresinde `pacer_rate_bps ≥ RECOVERY_FRACTION × ÖNCE-medyanı` olan İLK örnek → "ilk dönüş"; o andan sonra eşiğin altına hiç inmeyen ilk örnek → "kalıcı dönüş". Süre = örnek anı − `restored_ms`. Yoksa "dönmedi". Aynısı BtlBw için.
- **Eski çıkarım kıyası**: SONRA evresinin ilk `OLD_POST_WINDOW_S` saniyesinde tel datagramı ve bayt/s (eski çıkarım: 125 s'de 49 emisyon ≈ 28 B/s ≈ `min_rate` 31 B/s).
- ÖNCE evresinde örnek yoksa eşik tanımsızdır: "ölçülemedi — kesinti öncesi PreConverge dışı örnek yok".

## 4. ÖLÇÜT (makine-okur blok; analiz buradan okur)

<!-- M12-OLCUT-BASLA -->
```text
# FLOOR = 65 B · 1e9 · 1000 / (250 ‰ · 31 B/s) ns (receiver.rs:83-93); T_SPACING = 65 B · 1e9 / 31 B/s ns (receiver.rs:115)
ACK_CADENCE_FLOOR_MS = 8387.096774
T_SPACING_MS = 2096.774194
# varış: tek yön gecikmesi [0, 400] ms (delay 200 ± 200) → iki varış farkı ± 400 ms; + ACK_TICK 20 ms (runtime.rs:69) × k ≤ 3 + 40 ms görev payı
ARRIVAL_TOL_MS = 500
# üretim: ACK_TICK 20 ms yoklama + ack_delay 256 µs kuantum + görev payı
EMISSION_TOL_MS = 50
ACK_DELAY_SATURATED_RAW = 65535
MIN_STREAK_FRAMES = 2
# budget.rs:29 min_rate_bps
MIN_RATE_BPS = 31
RECOVERY_FRACTION = 0.5
OLD_POST_WINDOW_S = 125
OLD_S9_BOB_ACK_S = 22.1 30.2 46.9 55.6 72.4 80.8 89.2 114.3
# tablo başına en çok satır (tam liste JSON özetinde)
MAX_ROWS = 40
```
<!-- M12-OLCUT-BITTI -->

## 5. Kod okumasından beklentiler (ölçümden ÖNCE; tahmin değil, ölçümle sınanacak öngörü)

Kaynak satırları dal `k4g/05-int02` üzerindedir. Bu dalda adaptive-transport'a yalnız K4G-05 salt-okuma erişimcileri eklendi (72cb772c; motor davranışı değişmedi), satır numaraları o eklemeden sonrakilerdir; bu ölçüm için adaptive-transport'a dokunulmadı.

- **E-M1-1 (üretim sıklığı)**: Alıcıda bekleyen yeni bilgi yokken (`since_last_ack = 0`) zamanlayıcı SACK'i her `ACK_CADENCE_FLOOR` = 8387.097 ms'de bir üretilir (receiver.rs:309-316: `cadence_eff = ACK_CADENCE_FLOOR_NS`); sunucu ACK zamanlayıcısı 20 ms'de bir yoklar (runtime.rs:69) → üretim aralığı [8387, 8407) ms. Epizot kota kipinde aynı SACK'in kopyaları `T_SPACING` = 2096.774 ms aralıkla, epizot başına en çok `QUOTA_R_DEFAULT` = 4 kez gelebilir (receiver.rs:111-115, :298-306). Beklenen ölçüm: alıcı üretim aralıkları (Δ`ack_delay`) taban×1.
- **E-M1-2 (gönderende varış)**: S çerçevelerinin varış aralıkları k · 8.387 s ± 0.5 s'de kümelenir; k > 1 yalnız dönüş yolunda kaybolan SACK'lerden. Dönüş yolunda bağımsız %30 kayıp varsayımıyla k = 1 kesri ≈ 0.7 (öngörü; kapı değil). `ack_delay` 65535 × 256 µs = 16.78 s'de doyar (receiver.rs:459-466) → üretim aralığı yalnız dizinin ilk bir-iki S çifti için ölçülebilir.
- **E-M1-3 (motor S çerçevesini işler)**: S çerçevesinin PN'leri `PN_SEQ_WINDOW` = 65536 içinde kaldıkça motor seq'ine çevrilir (runtime.rs:78) → `mapped > 0`; motor boş olmayan her listede son-ACK anını tazeler (sender.rs:3023-3027) → S çerçevelerinde `last_ack_now = true`. Motorun tail ilerleme tanımı (`inflight` küçüldü, sender.rs:3219-3228) S çerçevesinde yalnız aynı olayda kayıp ilanı uçuştan kayıt çıkarırsa doğrulanır → S'lerin çoğunda "motor: ilerleme" YOK beklenir.
- **E-M1-4 (ACK sessizliği → acil durum → RecoverFwd)**: `ack_confidence` eşiği = max(3 · srtt, 2 · EAI), EAI = 2 · ort.birim / pacer + 65/31 s (sender.rs:6252-6285, :3402-3412, :1479, :1494-1498, :1545) ⇒ eşik ≥ 4.19 s. Seviye tırmanması health < 300 ‰ (emergency.rs:93) ⇒ sessizlik > 3.33 × eşik ≥ 13.98 s; inme health > 700 ‰ ve inmeler arası ≥ 1 s (emergency.rs:93-95). İkinci yol: füzyon kuralı loss HIGH ∧ bw LOW ∧ ack_confidence LOW > 0.5 (fusion.rs:543-548) — üyelik eşikleri bu okumayla sayısallaştırılmadı. Öngörü: (a) tek taban aralığı (≈ 8.4 s sessizlik) seviyeyi tırmandırmaz (ack_confidence ≥ eşik / 8.4 s ≥ 499 ‰); (b) seviye yolundan RecoverFwd girişi, sessizliğin 3.33 × eşiği aştığı aralıklarda olur — eşik 4.19–5.03 s iken bir S çerçevesinin dönüş yolunda kaybı (sessizlik ≈ 16.8 s) yeter, daha büyük eşikte iki ya da daha çok ardışık S kaybı gerekir — yani bayat SACK'ler SÜRERKEN; (c) her S çerçevesi sessizliği sıfırlar → seviye ≥ 1 s arayla iner → dizi boyunca RecoverFwd'e giriş-çıkış salınımı. Füzyon yolu ölçümde `fusion_emergency_mode` ile ayrılır.
- **E-M1-5 (dizi süresi)**: Bayat SACK dizisi, alıcıya yeni datagram (takılı birimin bir kopyası) ulaşana dek sürer. Takılı birim ARQ tavanından (`arq_retx_cap` = 3) sonra kopya kuyruğuna devredilir; kopya aralığı geometrik ×2 büyür, tavansız (`drain_due_duplicates`; K4G-05 teşhisi, 72cb772c). Öngörü: dizi süreleri koşu ilerledikçe uzar; sayısal öngörü YOK.
- **E-M2-1 (kesinti sırasında)**: ACK gelmez → BtlBw süzgeci güncellenmez (budama yalnız `update`'te, bw.rs:221-262) → `btlbw_bps` kesinti boyunca sabit. Pacer hızı yalnız faz kazancı / füzyon hız kazancıyla değişebilir (sender.rs:4239-4268); ACK'siz olaylarda (zamanlayıcı) da yeniden hesaplanır.
- **E-M2-2 (kesinti sonrası ilk ACK'ler)**: BtlBw penceresi W = 8 · (2 · 267 B / son-örnek-bw + 2.097 s) (sender.rs:3046-3052, :1517); son-örnek-bw ≳ 99 B/s iken W < 60 s → kesinti öncesi örnekler kesinti sonrası İLK güncellemede budanır (KEEP-LAST yalnız süzgeç boşalacaksa en yeni örneği tutar, bw.rs:248). Kesintiden önce gönderilip sonra teyit edilen paketlerin hız örneği kesinti süresine yayılır (teslim / ≥ 60 s) → `btlbw_bps` çöker; pacer `clamp(f(btlbw), 31 B/s, ∞)` tabanına iner (budget.rs:29, :52; sender.rs:4268).
- **E-M2-3 (toparlanma)**: Taban hızında ölçülen teslim hızı tabana yakındır; büyüme yalnız faz kazancıyla (Startup / ProbeUp) olur → yavaş. Eski çıkarımla (kesinti sonrası ilk 125 s ≈ 28 B/s) tutarlı öngörü: ilk 125 s'de tabanda örnek oranı yüksek. Toparlanma süresi için sayısal öngörü YOK.

## 6. Bilinen ölçüm sınırları (ölçümden önce beyan)

- Motorun `ack_confidence_permille` hesabı özeldir (sender.rs:6252); aynadaki değer füzyonun SON değerlendirmede gördüğüdür (`diag_fusion_input_snapshot`): olay anında hesaplanır, sonda anında yeniden hesaplanmaz — örnekler arasında motor olayı yoksa bayattır (`fusion_event_index` ile görülür).
- Alıcı tarafındaki SACK üretim anları doğrudan ölçülmez (sunucu ucunda canlı ayna yok); üretim aralığı yalnız Δ`ack_delay` ile ve doygunluk (16.78 s) öncesinde ölçülür.
- "Sonra" motor görünümü aynı olayda planlanan emisyonları da içerir (motor olayı tek çağrıda işler ve planlar); `inflight` küçülmesi bu yüzden ACK kaldırmasının alt sınırı değildir — `tx_during` ile birlikte okunur.
- Zaman çizelgesi 1 s çözünürlüklüdür: toparlanma anları ±1 s.
