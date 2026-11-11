# R25 bağımsız inceleme — eski motor / main / #181 (2026-11-11)

Durum: **inceleme notu** (H2). Kaynak ağacı: `main @ 6716eb8d` + PR #181 başı `83975797`; bu dalın
eklediği yalnız `crates/adaptive-transport/tests/cc_port_sim.rs` (kollar, sayaçlar, anahtarlar) ve bu
belge. Ürün koduna (`src/`) dokunulmadı. Etiket: **sim, ayrık-olay; netem DEĞİL** — sayılar motorlar
arası yön içindir, sahadaki süre değildir. "Geçti" yalnız koşmuş test için.

## 1. Özet

1. **#181'in "eski motor 5,4 s" satırı ürünün eski profili değildir.** Sim'in `Engine::Legacy` kolu
   `Sender::new()` + `Receiver::new()`'dir: kuyruk kurtarması KAPALI, ilk pencere 0. Ürünün geri alma
   yolu (`KAMA_TRANSPORT_ENGINE=legacy`) `ClientSession::new()`'dir: `validated_v1` kuyruk kurtarması
   AÇIK + ilk pencere 2·MTU (`crates/client-core/src/lib.rs:93-97`, `runtime.rs:2005-2066`). Bu profil
   sim'de r25'in **en kötü** koludur: ileti p50 **106,8 s** (#181 portu 8,5 s).
2. **Eski profilin r25 kökü #181'in port için bulduğu kökün aynısıdır (U10):** RTT örneksiz ilk PTO
   1 s; ilk paketin ACK'i ≥ 8,1 s'de gelebilir (130 B ileri ≈ 5,2 s + 65 B SACK ≈ 2,6 s + yayılım).
   Prob hedefi tombstone'a emekli edilir ve geç ACK'i Karn ile dışlanır → RTT örneği oluşmaz, her tur
   yeni kopya, 20 paketlik kuyruk taşar. Sınama: yalnız RTT örneksiz ilk PTO'yu 10 s yapmak eski profili
   **106,8 → 5,8 s**'ye indirir (prob 57 → 15, kopya 133 → 4, kuyruk düşen 150 → 0). 10 s taban PTO
   tavanını kaldırmak (U11'in eski karşılığı) etkisizdir (107,6 s).
3. **Hipotezlerin sim'deki durumu:** H-a (ACK kadansı) — sim'de **çürütüldü** (port dönüş baytı eski
   kollardan az; bkz. §5 gerçek yığın). H-b (10 s tavan) — port için doğru (#181 U11, 82,5 → 17,3 s),
   eski profil için **tek başına değil**. H-c (ilk uçuş 4·MTU) — bu hücrede **çürütüldü** (ileti 10 s
   aralıklı, hücre veriyle sınırlı; IW 4·MTU ile 12 000 B bayt düzeyinde aynı). H-d (RecoverFwd /
   kopya eski motorun teslim kaynağı) — **çürütüldü**: kazanan eski kol (tail kapalı) yalnız 4 kopya
   gönderir; kopyalar eski profilde kaybın sonucudur, teslimin kaynağı değil. H-e — kısmen (§6).
4. **SACK sim düzeltmesi (`83975797`) gecikmeyi değiştirmez;** yalnız eski kolun dönüş baytını
   doğru sayar (5 126 → 7 076 B). Port kolu aynı.
5. **Öneri (§8):** düşük bant kipinde RTT örneksiz ilk PTO'yu ilk paketin dönebileceği süreden kısa
   tutmamak. Global "ilk PTO 3 s" deneyi r25'i 8,5 → 7,0 s'ye indirir; öbür 7 sim hücresi **bayt
   düzeyinde aynı** (CP-1, manşet, kesinti dahil). Genel motoru yavaşlatmayan biçim: önceki oturumun
   RTT'si (RFC 9002 §6.2.2) ve eski profil için U10'un eşdeğeri.

## 2. Kollar ve sim sapmaları

`r25_arms` (yalnız `CC_R25_ARMS=…` ile koşar; kapı değil): 25 B/s iki yön, kuyruk 20 (iki yön), %20
rastgele kayıp (ileri), 60 × 65 B ileti / 10 s, ileri tel zarfı 65 B (paket 130 B), 10 tohum, medyan.

| kol | kurucu | not |
|---|---|---|
| `legacy` | `Sender::new()` + `Receiver::new()` | #181'deki "eski motor"; tail KAPALI, IW 0 |
| `legacy-product` | + `validated_v1` + IW 2·MTU | `ClientSession::new()` = ürün geri alma yolu |
| `legacy-tailv1` | + `validated_v1`, IW 0 | bench `tail-v1` kolu (`crates/bench/src/kama.rs:111-117`) |
| `legacy-iw2` | + IW 2·MTU, tail kapalı | ayrıştırma |
| `legacy-k1a6` | `legacy-product` + E2a (tt_rfc, g = 1 ms, rtt_rule_v11, iw_idle_hold, pto_probes = 2) | gösterimdeki `K1A6`'nın **yalnız E2a alt kümesi**; E1 bayrakları, `gap_ack`, `return_prior_gated`, a3/a4/a6 main'de yok |
| `cc-port` | `Sender::with_cc_port()` + `Receiver::with_cc_port()` | ilk uçuş 10·MTU |
| `cc-port-iw4` | + ilk uçuş 4·MTU | ürün runtime'ı (`runtime.rs:2059-2065`) |

Sim'in bench'ten bilinen sapmaları:
- **Gecikme:** `run_opt` tek yönü `rtt_ms / 2` alır; r25 testi `300` verir → tek yön **150 ms**.
  Bench `d300` tek yön 300 ms'dir. `CC_R25_RTT=600` satırı aşağıda (sonuç yönü değişmez).
- **Mutlak düzey:** gece bench'inde ürün eski profili (gösterim bench `K0`, bayraklar kapalı) r25'te
  p50 25,2–28,6 s, port (U10 öncesi) 48,2–48,6 s idi. Sim ikisini de kötümser verir (106,8 s / 95,6 s);
  sıralama bench'le **aynı** (eski profil ≤ U10 öncesi port), oran farklı. Sim kuyruk, kayıp ve
  zamanlayıcıyı idealleştirir; netem/runtime'daki 20 ms ACK tik'i ve gerçek pacer davranışı yoktur.

## 3. Mekanizma tablosu

r25 kuramsal değerleri: ileri paket 130 B → **5,2 s**; SACK (0 aralık) 65 B → **2,6 s**, +4 B/aralık
(+0,16 s); boş kuyrukta ilk RTT ≈ 0,3 + 5,2 + 2,6 = **8,1 s** (tek yön 300 ms'de 8,4 s); ileri yük
13 B/s (bant %52, kayıpla ~%65).

| mekanizma | eski motor (dosya:satır, sabit) | port (dosya:satır, sabit) | r25 etkisi (kuramsal) | hipotez |
|---|---|---|---|---|
| Alıcı ACK kadansı | `ACK_FREQ_N = 2` (`receiver.rs:76`, `tick_ack` 474-484); zamanlayıcı `on_ack_timer_legacy` (311-364): kuyruk ACK'i `K_TAIL = 2`×son aralık, epizot kotası `QUOTA_R_DEFAULT = 4` (111) ≥ `T_SPACING_NS` = 65/31 s ≈ 2,1 s (115) aralıklı, kadans `clamp(2·son aralık, 2,1 s, ACK_CADENCE_FLOOR_NS ≈ 8,39 s)` (93-94). Bant ölçülmez: önsel `RETURN_BW_PRIOR = 31` B/s (87) | aynı N = 2 frekans yolu; zamanlayıcı yalnız `MAX_ACK_DELAY_NS = 25 ms` (275, 295-305); sırasız varışta anında ACK (238-240); kadans/kota yok | port: varış aralığı ≫ 25 ms olduğundan pratikte ~1 SACK/varış + sırasız varışta ek SACK → ~6,5 B/s (%26) taban; eski: kota yinelemeleri yeni bilgi taşımadan da çıkar | H-a |
| Yinelenen varışta ACK | zorla (`receiver.rs:226-228`) | aynı | iki motorda aynı | H-a |
| ACK gecikme bilgisi | çerçevede `max_ack_delay` alanı yok (`ack.rs:55-78`); `ack_delay` 16,78 s'de doyar (`receiver.rs:80, 499-507`) | aynı | — | H-e |
| PTO tabanı | `validated_v1` (`tail_recovery.rs:72-82`): `clamp(srtt + 4·rttvar + 25 ms, 10 ms, 10 s)`; RTT örneksiz 1 s; geri çekilme tavanı 30 s; **prob turu 3**; E2a kapalıyken tur başına 1 prob. `Sender::new()`'de tail **kapalı** (`sender.rs:2019`) | `rfc9002_cc_port()` (`tail_recovery.rs:89-99`): prob turu tavansız, **U11** taban tavanı yok (`max_pto_ns = u64::MAX`); tur başına 2 prob, **U12** RTT örneksizken 1 (`sender.rs:6785-6789`); prob pencereden muaf (pacer jetonu) | srtt ≥ 8,1 s → 10 s tavan `srtt + 4·rttvar`'ı keser, ACK gelmeden prob; RTT örneksiz 1 s, 3 s probları ilk ACK'ten (≥ 8,1 s) önce **kesin sahte** | H-b |
| PTO probu hedefi | hedef tombstone'a emekli + Karn için tombstone işaretlenir (`sender.rs:6757-6760`) → hedefin geç ACK'i RTT örneği vermez | **U10:** hedef uçuşta kalır; birim başına uçuşta ≤ 8 kopya (`CC_MAX_UNIT_COPIES_IN_FLIGHT`, 1567) | RTT örneksiz erken prob + emeklilik = RTT örneği hiç oluşmaz (eski profilin r25 kökü, §4.3) | — (yeni) |
| Tombstone TTL | `K_TOMB = 8` × T_loss (`sender.rs:1564, 3848-3851`); eski tabanla ≥ 8 × kadans tabanı (≥ ~50 s) | aynı formül; T_loss tabanı g = 1 ms → RTT örneksiz 8 × 9/8 × 100 ms = **0,9 s** (U10 öncesi; `sender.rs:6752` yorumundaki "≈ 3 s" formülle uyuşmuyor) | port U10 öncesi: hedefin ACK'i (≥ 8,1 s) TTL'den sonra gelir | — |
| Kayıp ilanı — zaman | `max(9/8·srtt, 1,5·EAI)` (`sender.rs:3796-3797, 3824-3836`); EAI = `2·birim/pacer_hızı + 65/31 s` (3803-3813) → r25'te ~4,3–7,3 s, taban **~6,4–11 s** | `max(9/8·max(srtt, latest), g = 1 ms)` (`tt_rfc ∥ cc_on`, 3824-3827) | port: ilk RTT örneği şişkinse eşik de uzar; taban koruması yok (srtt doğruysa sorun değil) | H-e |
| Kayıp ilanı — paket | `max(3, 2·recent_max_reorder)` (`arq.rs:162-166`; iki motorda aynı `retx_cfg`, `sender.rs:2149`) | aynı | 60 küçük ileti, ~6 paket uçuşta: paket eşiği nadiren tetiklenir | H-e |
| İlk uçuş / min pencere | PreConverge pace tabanı `PRE_CONVERGE_FLOOR_BW = 31 B/s` (`sender.rs:1537`); IW 0 → tek paket; ürün IW 2·MTU (`DEFAULT_BURST_CAP`) | `min_cwnd = 4·MTU` (`cc/bbr.rs:191-193`); kurucu `rfc9002_initial_window` 10·MTU (187-189); ürün 4·MTU | ileti 10 s aralıklı: uçuşta veri yok denecek kadar az → IW bağlamaz | H-c |
| Soğuk başlangıç / PreConverge | PreConverge'de ilk RTT örneğine dek kayıp ilanı bastırılır (`sender.rs:3995`, iki motor) | aynı | iki motorda aynı | — |
| RecoverFwd / kopya | ack_confidence sessizliği `max(3·srtt, 2·EAI)` üstünde → `RecoverFwd`, kopya çarpanı 2, 20 ms aralık (`sender.rs:1531-1534, 4421-4457, 6918-6930`); ARQ retx tavanı 3 sonra `HandToForward` (`arq.rs:70-77`) | aynı durum makinesi; kopyalar `cc_fits` ile pencereye bağlı (`sender.rs:2262-2264, 6549-6554`); retx tavanı `u8::MAX` (`arq.rs:85-91`) | kazanan eski kol yalnız 4 kopya; ürün eski profilde 133 kopya kayıp-sonucu | H-d |
| retx-first | DATA önce; ARQ her `K_ARQ_SHARE = 4`. jetonda ve yalnız `RecoverArq`'ta (`sender.rs:1590, 4788-4829`) | retx her zaman önce (`cc_on`, 4827) | düşük bantta retx-first kuyruğu kopyalarla doldurursa yeni iletiyi geciktirir | H-e |
| Pacing | `min(gain·BtlBw·fuzzy, max(2·BDP, 4·MTU)/srtt)` (`rate.rs:164-177`); kayıp hızı düşürmez (INV-RATE-1) | BBR `pacing_rate` (gain × BtlBw), örneksiz 1,25·pencere/srtt (`sender.rs:2584-2603`); kayıp pencereyi `recovery_window` ile sınırlar | hücre veriyle sınırlı | — |

## 4. Sim sonuçları (10 tohum, medyan; p50 köşeli parantez = tohumlar arası en küçük..en büyük)

### 4.1 Üç kol — iş emrinin istediği (tek yön 150 ms, SACK düzeltmesi sonrası sim)

| kol | ileti p50 | p95 | teslim | ileri B | dönüş B | SACK | prob | retx (dup) | kuyruk düşen |
|---|---|---|---|---|---|---|---|---|---|
| main motoru, `legacy` (tail kapalı) | 5,3 s [5,3..61,2] | 40,5 s | 60/60 | 10 119 | 7 076 | 77 | 0 | 17 (4) | 0 |
| main motoru, `cc-port` | **95,6 s** [90,8..98,4] | 262,8 s | 60/60 | 19 286 | 10 836 | 99 | 48 | 85 (31) | 21 |
| main motoru, `cc-port-iw4` | 95,6 s (aynı) | | | | | | | | |
| #181, `cc-port` | **8,5 s** [5,3..11,2] | 41,3 s | 60/60 | 11 658 | 6 370 | 70 | 13 | 28 (4) | 0 |
| #181, `cc-port-iw4` | 8,5 s (aynı) | | | | | | | | |
| **ürün eski profili** (`legacy-product`) | **106,8 s** [95,6..121,6] | 318,0 s | 58/60 | 37 096 | 9 826 | 90 | 57 | 222 (133) | 150 |
| ürün eski + E2a alt kümesi (`legacy-k1a6`) | 55,6 s [29,2..90,0] | 177,6 s | 60/60 | 14 887 | 7 971 | 79 | 28 | 52 (17) | 3 |

Eski motor yolu main ile #181 arasında bayt düzeyinde aynıdır (`legacy` satırı iki tabanda özdeş;
U10–U13 yalnız `cc_on()`).

### 4.2 Ayrıştırma (#181 tabanı)

| kol / anahtar | ileti p50 | prob | retx (dup) | kuyruk düşen |
|---|---|---|---|---|
| `legacy-iw2` (IW 2·MTU, tail kapalı) | 5,3 s | 0 | 17 (4) | 0 |
| `legacy-tailv1` (validated_v1, IW 0; bench tail-v1) | 104,8 s | 46 | 190 (118) | 119 |
| `legacy-product` + `CC_LEGACY_PTO_CAP_OFF` (10 s tavan yok) | 107,6 s | 52 | 221 (133) | 150 |
| `legacy-product` + RTT örneksiz ilk PTO 3 s | 107,6 s | 40 | 190 (131) | 121 |
| `legacy-product` + RTT örneksiz ilk PTO **10 s** | **5,8 s** [5,3..103,2] | 15 | 28 (4) | 0 |
| `legacy-product` + ilk PTO 10 s + tavan yok | 5,6 s | 9 | 23 (4) | 0 |
| `cc-port` + RTT örneksiz ilk PTO 3 s | 7,0 s | 11 | 26 (4) | 0 |
| `cc-port` + RTT örneksiz ilk PTO 6 s | 7,8 s (p95 35,7 s) | 11 | 27 (4) | 0 |

Okuma: ilk pencere eski motorda etkisiz (`legacy-iw2` = `legacy`). Farkın tamamı kuyruk
kurtarmasındandır ve 10 s tavanı değil, **RTT örneksiz erken ilk prob** belirleyicidir: 3 s yetmez
(ilk ACK ≥ 8,1 s), 10 s yeter.

### 4.3 SACK sim düzeltmesinin etkisi (`CC_SACK_NEWONLY=1` = düzeltme öncesi)

| kol | dönüş B (öncesi → sonrası) | ileti p50 |
|---|---|---|
| `legacy` (main ve #181) | 5 126 → 7 076 | 5,3 s → 5,3 s |
| `legacy-product` | 8 567 → 9 826 | 106,8 s → 106,8 s |
| `legacy-k1a6` | 6 528 → 7 971 | 55,6 s → 55,6 s |
| `cc-port` (main / #181) | 10 836 / 6 370 → aynı | 95,6 / 8,5 s → aynı |

Düzeltme yalnız tel maliyetini doğru sayar: eski alıcının yinelenen (yeni bilgi taşımayan) kota
SACK'leri artık dönüş baytında görünür. Gecikme ve göndericinin davranışı değişmez. Port alıcısı
yinelenen SACK'i yalnız yinelenen varışta üretir; satırı aynı kalır.

### 4.4 Tek yön 300 ms (`CC_R25_RTT=600`, bench `d300` eşdeğeri)

| kol | ileti p50 | teslim |
|---|---|---|
| `legacy` | 5,5 s | 60/60 |
| `legacy-product` | 108,5 s | 60/60 |
| `legacy-k1a6` | 54,9 s | 60/60 |
| `cc-port` (#181) | 7,1 s | 60/60 |

Sıralama ve büyüklükler 150 ms satırıyla aynı; gecikme serileştirmenin (5,2 s + 2,6 s) yanında küçük.

## 5. Gerçek yığın karşılığı (`shaped_relay`)

(Doldurulacak.)

## 6. Hipotezler

- **H-a (ACK kadansı dönüş hattını doldurur):** portun `MAX_ACK_DELAY_NS = 25 ms` zamanlayıcısı ve
  sırasız varışta anında ACK kuralı doğru (`receiver.rs:238-240, 275, 295-305`); "her varışa ACK" ise
  harfiyen değil — sıralı varışta N = 2 frekans yolu da var (`receiver.rs:76, 476`), ama varış aralığı
  ≫ 25 ms olduğundan pratikte ~1 SACK/varış. Sim'de dönüş hattı darboğaz değil: #181 portunun dönüş
  baytı (6 370) eski kollardan az (7 076; ürün eski 9 826), dönüş kuyruğunda düşme 0. Eski alıcının
  spec §12 "bant-farkında kadansı" **ölçülmüş bant kullanmaz**: önsel `RETURN_BW_PRIOR = 31 B/s` ve
  sabit `T_SPACING` 2,1 s / taban 8,39 s'dir (`receiver.rs:87, 93-94, 115`; adaptasyon D2 sonrası,
  91-92). Gerçek yığın satırı §5'te.
- **H-b (10 s taban PTO tavanı):** port için **doğru** ve #181 U11'le giderildi (82,5 → 17,3 s,
  #181 tablosu). Eski profilde aynı tavan var ama tek başına **etkisiz** (§4.2: tavansız 107,6 s).
  "Eski motorda prob sayısı sınırlı" doğru (`max_tail_probes = 3`), ama sınır r25'i kurtarmaz.
- **H-c (min_cwnd 4·MTU / ilk uçuş):** bu hücrede **çürütüldü**: IW 4·MTU ile 12 000 B arasında
  bayt düzeyinde fark yok; ileti 10 s aralıkla sunulduğu için uçuşa girecek veri yok. "Eski motor tek
  paketten başlar" doğru (IW 0, PreConverge 31 B/s), ama ürün IW 2·MTU ile de aynı sonucu verir.
  Dosya hücrelerinde (150 kB) H-c ayrıca sınanmalı.
- **H-d (RecoverFwd kopyaları eski motorun teslim kaynağı):** **çürütüldü**: en iyi eski kol 4 kopyayla
  60/60 teslim eder; ürün eski profilin 133 kopyası RTT'siz prob döngüsünün sonucudur ve kuyruğu
  taşırır (150 düşme). Portta `cc_fits` kopyaları pencereye bağlar; #181'de kopya 4.
- **H-e:** (i) Zaman eşiği: port 9/8·max(srtt, latest), taban 1 ms; eski taban 1,5·EAI ≈ 6,4–11 s.
  RTT örneği doğruysa fark küçük; #181'in "ilk RTT örneği şişkin → eşik uzun" gözlemi port için
  geçerli. (ii) Paket eşiği 3 iki motorda aynı, küçük uçuşta etkisiz. (iii) `MAX_ACK_DELAY` 25 ms
  iki motorda PTO'ya eklenir (`tail_recovery.rs:79, 174-179`); r25'te ihmal edilebilir. (iv) retx-first
  portta her zaman, eskide yalnız `RecoverArq`'ta; r25'te kopya sayısı düşük olduğundan (#181: 4) ikinci
  derece.

## 7. #181'in bulgusu doğru mu, eksik ne

**Doğru olan:** port kökü (U10: prob hedefinin emekliliği + RTT örneksiz hat → hiç RTT örneği yok)
sim'de bağımsız olarak doğrulandı: main portu 95,6 s, #181 portu 8,5 s; eski motor yolu bayt düzeyinde
aynı. U11 (10 s tavan) port için gerekli. Kalan farkın RTT örneksiz başlangıç problarından geldiği
gözlemi de destekleniyor: RTT örneksiz ilk PTO'yu 3 s'ye çıkarmak 8,5 → 7,0 s verir.

**Eksik olan:**
1. **Karşılaştırma tabanı ürün eski profili değil.** "Eski motor 5,4 s" kuyruk kurtarması kapalı,
   ürünün göndermediği bir yapılandırmadır. Ürünün geri alma yolu sim'de 106,8 s'dir; yani sim'de #181
   portu ürün eski profilinden **~12× iyidir**, ondan geride değildir. Bench'in gece K0 sonucu
   (25–28 s) ile sim'in 106,8 s'si arasındaki fark sim'in kötümserliğidir (§2), yön aynıdır.
2. **Aynı kök eski profilde de var** ve geri alma yolu (`KAMA_TRANSPORT_ENGINE=legacy`) r25'te bu
   yüzden kötüdür. #181 U10'u yalnız `cc_on()` altında uygular.
3. **ACK kadansı (gerçek yığında port 17 k ↔ eski 10 k dönüş baytı):** §5.
4. Küçük: `sender.rs:6752` ve `CC-PORT-ESLEME.md`'deki "tombstone TTL ≈ 3 s" port için 0,9 s'dir
   (`K_TOMB` 8 × 9/8 × `INITIAL_RTT` 100 ms; port tabanı g = 1 ms). Sonuç değişmez (ikisi de 8,1 s'den
   kısa).

## 8. Öneri (yalnız düşük bant kipinde; eski kazanımları bozmadan)

Kök ortak: **RTT örneği yokken ilk prob, ilk paketin dönebileceği süreden önce atılıyor.** Hızlı hatta
bu süre milisaniyelerdir ve 1 s'lik RFC 9002 ilk PTO'su doğrudur; 25 B/s'de ≥ 8 s'dir.

1. **Port — önceki oturumun RTT'si (RFC 9002 §6.2.2):** runtime eş başına son `srtt`/`rttvar`'ı
   saklar, yeni oturumun RTT örneksiz ilk PTO'su `max(1 s, srtt_önceki + 4·rttvar_önceki)` olur.
   Hızlı hatta önceki srtt küçük → davranış bugünküyle aynı; düşük bantta ilk PTO serileştirmeyi
   kapsar. Kanıt (sim, global yaklaşım): ilk PTO 3 s → r25 8,5 → 7,0 s; öbür 7 hücre (CP-1 150 kB
   12,19 s, manşet q6/q1000 l20 10/10 15,89 / 15,71 s, kesinti 5/5 84,3 s, PTT 0,34 s, FEC 12,15 s)
   **bayt düzeyinde aynı**. Eş başına önbellek ilk oturumu düzeltmez; ilk oturum için ikinci kural:
2. **İlk oturum için düşük bant kipi (eski ve port):** ilk ACK'ten önce göndericinin elinde bant
   bilgisi yoktur (eski motor PreConverge'de zaten 31 B/s tabanında pace eder, hızlı hatta da), bu
   yüzden "kendiliğinden algılayan" bir kural genel motoru da yavaşlatır. Kip **dışarıdan** açılmalı:
   `RuntimeConfig`'te beklenen ilk RTT ipucu (ör. gösterimin düşük bant profili 9 s) ya da (1)'deki
   önbellekte `srtt_önceki > 2 s`. Kip açıkken RTT örneksiz ilk PTO = `max(1 s, ipucu)`; kapalıyken
   bugünkü davranış bit-eşit.
3. **Eski profil (geri alma yolu):** U10'un eşdeğeri — RTT örneği yokken prob hedefi tombstone'a
   emekli edilmez (`sender.rs:6757-6760`'taki dal `cc_on()` yerine `cc_on() ∥ srtt.is_none()`).
   Ek gönderim yok, yalnız hedefin ACK'i RTT örneği olur. Sim kanıtı dolaylı: RTT örneksiz ilk PTO
   10 s ile eski profil 106,8 → 5,8 s.

Kapılar: öneri uygulanırsa `cc_port_sim` 8 hücresi (CP-1 ≤ 25 s, manşet 10/10, kesinti 5/5) ve r25
kapısı (≤ 12 s) bayt düzeyinde/kapı düzeyinde yeniden koşulmalı; yukarıdaki global deney yalnız
yönü gösterir. Ürün değişikliği bu notun kapsamı dışında (ayrı PR, sahip kararı).

## 9. Yeniden üretim

```bash
# üç kol + ayrıştırma (#181 tabanı; main motoru için aynı sim dosyası main kaynağıyla)
CC_R25_ARMS=1 cargo test --locked -p adaptive-transport --all-features --test cc_port_sim r25_arms -- --nocapture
CC_R25_ARMS=1 CC_R25_RTT=600 cargo test ... r25_arms -- --nocapture
CC_SACK_NEWONLY=1 CC_R25_ARMS=1 cargo test ... r25_arms -- --nocapture
CC_LEGACY_PTO_INIT_MS=10000 CC_R25_ARMS=legacy-product cargo test ... r25_arms -- --nocapture
CC_PTO_INIT_MS=3000 cargo test --locked -p adaptive-transport --all-features --test cc_port_sim -- --nocapture --test-threads=1
```

## 10. Sınırlar

- Sim netem değildir; mutlak süreler bench'ten 2–4× uzun (§2). Yön ve ayrıştırma içindir.
- `legacy-k1a6` gösterim K1A6'nın yalnız E2a alt kümesidir.
- Öneriler sim'de global yaklaşımla sınandı; önerilen biçimler (eş başına RTT, düşük bant
  tetikleyicisi, eski U10) uygulanmadı, sınanmadı.
