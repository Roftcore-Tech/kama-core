# DR-CS-1 — COLD-START PROGRESS DR: "MINIMAL BOOTSTRAP"

> **YAMA KAYDI — 2026-10-24 [v1.1 · K4G-05b — kabul fikstürü düzeltmesi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-8 (a) koşul 4 — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** v1.1'de kabul fikstürleri düzeltildi; neden: teyit edilmiş birimi yeniden planlayan v1 davranışına dayanıyordu. Kapsam: `crates/adaptive-transport/tests/dr_cs1_acceptance.rs` DI-1 (`f2b_di1_first_data_loss_bootstrap_recovery`), DI-1C (`f2b_di1c_rearmed_pto_lifecycle_exposure`), DI-2 (`f2b_di2_single_ack_erasure_spurious_expected`); yalnız ACK'lenen seq'lerin seçimi değişir, assertion ve eşikler DEĞİŞMEDİ; eski/yeni kod × eski/yeni fikstür matrisi ve mutasyon çıktısı K4G-05b PR açıklamasında. Otoriter v1.1 kaydı: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]. Bu belgenin metni değiştirilmedi.

> **ARŞİV ÜST-NOTU:** Bu doküman talimat-zincirinin (v1 → v2 → v3 → v4 → v5 → v6 → **v7**) nihai
> normatif halidir; v7, v6'yı hükümsüz kılar. v1-incelemesi (EK1, 11 madde) ve v3-incelemesi (EK2,
> B1-B5+T1-T2) AYNEN yürürlükte; v2-incelemesi (N1-N8) gövdeye işlenmiştir; v4-incelemesi = EK3
> (C1-C4); v5-incelemesi = EK4 (A1-A2); v6-incelemesi = EK5 (E1-E2). İç başlıklar bu sürümden
> itibaren SÜRÜMSÜZDÜR (bayat sürüm-etiketi sınıfı yapısal kapalı). Gövde-amendman çelişkisi =
> amendman kazanır → DUR-B.
>
> **Abkarada v6-inceleme hükmü:** *CONDITIONAL GO — MINIMAL BOOTSTRAP DESIGN IS SOUND, BUT
> SAME-TIMESTAMP INBOUND EVENTS MUST BE ATOMICALLY BATCHED BEFORE DUE-TIMER PROCESSING.*
> Bu doküman o kapanışı (EK5: atomik batch API + Tick-only no-op daraltması) normatif içerir.
>
> **Dayanak:** FO `42d4ef0` (cold-start direct-integration kanıtı) · FP `9e0a4b5` (DEFECT-hükmü) ·
> FQ `28f0f6a` (faithful-harness) · FR `782fddd` (wall-clock-runner) · Abkarada denetim §6-§7.
> **Başlatma yetkisi:** F0, F1, F2A bu belgeyle; F2B/F3 = batch-atomiklik-kanıtı (F1) dahil tüm kapılar.

---

## 0. BAĞLAYICI ÇERÇEVE

### 0.1 Kapsam — MINIMAL BOOTSTRAP
```text
external wakeup → authenticated probe → echo: bidirectional-path + bootstrap-RTT →
PreConverge→Startup → geçmiş outstanding DATA için MEVCUT PTO/ARQ → typed timeout
```
YASAK: `seen_count`/hedefli-retransmit (→DR-CS-2) · geometrik-kopya değişikliği (→ayrı DR) ·
post-timeout echo kabulü (Seçenek-B; park) · FEC/DR-1b/P2/fuzzy (kapsam dışı; P0 ayrı yürür).

### 0.2 MÜHÜR-AMENDMAN (DÖRT kalem)
(a) PreConverge çıkışına ikinci meşru srtt kaynağı (probe-echo; `srtt.is_some()→Startup` kuralı
DEĞİŞMEZ) · (b) PreConverge içi CS-Probe merdiveni · (c) `next_wakeup_ns()` + `Event::Tick` +
**`on_event_batch` atomik giriş noktası** (EK5 ile açıkça genişletildi — sessiz genişleme değil):
tekil `on_event` korunur; flag-OFF'ta adapter devre dışı, legacy tekil yol byte-for-byte;
`Event::Tick` ≡ boş batch (kapsam M1) · (d) Bootstrap-sonrası geçmiş-outstanding PTO yetkisi:
ProbeEcho bootstrap-SRTT oluşturduğunda, PreConverge-öncesi gönderilmiş outstanding-unacked
birimler için MEVCUT PTO/loss-detection uygunluğunun yeniden değerlendirilmesi VE bu birimlerin
PTO deadline'ının wakeup sözleşmesine bağlanması (ikisinin sonucu birlikte). **Bağlanma
YAŞAM-DÖNGÜSÜ kapsamlıdır (EK4-A2):** bootstrap-origin outstanding birim için mevcut ARQ'nun
kurduğu İLK ve TAKİP EDEN bütün PTO deadline'ları, birim ACK/recovery/cleanup ile outstanding
kümeden çıkana kadar `next_wakeup_ns()` kapsamındadır; her fire sonrası ARQ yeni deadline kurarsa
otomatik bağlanır. Loss threshold, retransmit formatı, sequence/nonce semantiği, ARQ recovery
kuralları DEĞİŞTİRİLEMEZ.

**DEĞİŞMEYENLER (ihlal=PR-RED):** bw.rs/delivered-EAI/BtlBw · send_rate-loss-modeli · INV-RATE-1 ·
TokenBucket · SACK-V2 · qd-yolu · ZAMAN-MODELİ ns-semantiği · ARQ kuralları/formatları/eşikleri
((d) yalnız uygulama-genişletmesi) · FEC yolu · üretim byte-layout'ları · mühürlü DATA-önce
sıralama · geometrik kopya formülü/backoff'u · loss-ledger · ack_confidence · delivery-sampling.
Dışına taşan diff = DUR-B.

### 0.3 YÜRÜRLÜKTEKİ İNCELEME AMENDMANLARI

**EK1** (11 madde) ve **EK2** (B1-B5+T1-T2) aynen yürürlükte (referansla).

**EK3 — TAM GO ÖNCESİ ZORUNLU DÜZELTMELER (NORMATİF):**
```text
C1 — F2 döngüselliği: F2 → F2A (Probe-Emission-Feasibility) + F2B (Full-Bootstrap).
F2A yalnız minimal probe enqueue/emission; echo/RTT/state/PTO yok. DI-1B/DI-6B F2A'da.
Yeşil değilse DUR-B.

C2 — Overdue PTO: bootstrap-SRTT sonrası hesaplanan PTO deadline <= now_ns ise DUE'dur;
tetikleyen batch'in faz-3'ünde çalıştırılır. Geleceğe ötelenemez, busy-loop oluşturamaz,
iki kez fire edemez.

C3 — Equal-timestamp ordering: aynı now_ns batch'inde authenticated inbound ACK/echo →
cancellation/disarm → due timer evaluation → output scheduling sırası. ACK/echo ile aynı
timestamp'teki probe/giveup timer'ı success kanıtından SONRA değerlendirilir. DI-12A..D zorunlu.

C4 — Bidirectional control accounting: B_COLD_BUDGET yalnız forward probe bütçesi olarak
yeniden adlandırılır. cold_probe_tx_bytes / cold_echo_tx_bytes /
bootstrap_induced_retransmit_bytes AYRI sayaçlar. Full-duplex ve shared/half-duplex
serialization ayrı raporlanır. Echo wire max değeri spec'ten türetilir.
```
**SEÇİLEN SEMANTİKLER (bağlayıcı):**
- **C2 → SAME-BATCH DUE PROCESSING (EK5):** her `on_event_batch` dönmeden önce dört fazı sırayla
  tamamlar; due exposed timer'lar faz-3'te BİR KEZ ateşlenir. **Post-condition (NORMATİF, batch
  sınırında):** batch dönüşünde hiçbir exposed timer due değildir; `next_wakeup_ns()` daima
  `{None, gelecek}` — geçmiş değer HİÇ expose edilmez (busy-loop yapısal imkânsız).
  Retire-on-fire çift-ateşlemeyi engeller. Overdue deadline'lar tetikleyen batch'in faz-3'ünde tüketilir.
- **C3 → batch pipeline = tek `on_event_batch` çağrısının iç sırası** (dört faz; EK5-E1); ns-kesin
  eşitlik (ZAMAN-MODELİ); aynı-ns inbound'lar adapter damgalamasıyla AYNI batch'te (EK5-ii);
  kesin-ÖNCE gelen timer meşru kazanır (DI-12D).

**EK4 — İKİ ZORUNLU AMENDMAN (NORMATİF — F2B/F3 önkoşulu):**
```text
A1 — RX patience profil-türevi bound:
RX_COLD_ABANDON = max(
    T_COLD_GIVEUP + RTT_MAX_SUPPORTED + RECOVERY_ALLOWANCE,
    T_NEXT_RECOVERY_ELIGIBLE + MAX_RECOVERY_SERIALIZATION
      + FORWARD_DELAY_BOUND + RECEIVER_PROCESSING_MARGIN )
Sabit 75s YALNIZ hesaplanan profil bound'u 75s'in altındaysa kullanılabilir. 31 B/s gibi
profillerde ya daha uzun profile-derived patience ya da S-B session-hold + hard-cap ZORUNLU.

A2 — Re-armed PTO exposure: bootstrap-origin outstanding birim için mevcut ARQ'nun kurduğu
ilk ve TAKİP EDEN bütün PTO deadline'ları, birim outstanding kümeden çıkana kadar
next_wakeup_ns() kapsamındadır. Yalnız ilk PTO'nun expose edilmesi YETERSİZDİR.
```
*A1 terim tanımları:* `T_NEXT_RECOVERY_ELIGIBLE` = outstanding cold birimler üzerinden bir SONRAKİ
ACK-bağımsız kurtarma emisyonunun uygunluk zamanı maksimumu (legacy geometrik program; 1200B ≈61.2s) ·
`MAX_RECOVERY_SERIALIZATION` = max_unit_wire_bytes / usable_rate (profil) · `FORWARD_DELAY_BOUND`
(profil) · `RECEIVER_PROCESSING_MARGIN` [CAL]. Bound, profil × max-birim-boyutu başına
F0-tespit-(viii)'de GERÇEK sabitlerden hesaplanır ve dondurulur. *A2, (d) sınırı içindedir; C2
post-condition ile doğal uyumlu.*

**EK5 — BATCH ATOMİKLİĞİ (NORMATİF):**
```text
E1 — Aynı-timestamp inbound olayları, due-timer işlenmeden ÖNCE atomik batch'lenir.
Tekil on_event üzerinde "driver önce toplar" dokümantasyon vaadi YETERSİZ; garanti API
düzeyinde. Seçilen model: AÇIK BATCH API — on_event_batch { now_ns, inbound_events[] }.
Normatif sıra: 1) authenticated inbound'ları işle 2) state resolution/cancellation/disarm
3) due timer'ları BİR KEZ çalıştır 4) output scheduling.

E2 — Exact no-op YALNIZ Event::Tick (= boş batch): due exposed timer ve bekleyen scheduling
işi yoksa schedule_transmit/queue-drain çağrılmaz. Inbound içeren batch'ler, due timer
olmasa bile normal işleme + output scheduling'i TAMAMLAR.
```
*E1 bağlayıcı uygulama kararları:* **(i)** `Event::Tick` ≡ `on_event_batch{now_ns, inbound=[]}` —
tek giriş noktası. **(ii)** Adapter saati poll/reactor iterasyonu başına BİR KEZ örnekler; o
iterasyonda toplanan TÜM inbound aynı `now_ns` ile damgalanır → aynı-ns olaylar ayrı batch'lere
DÜŞEMEZ (atomiklik inşaatla). **(iii)** Flag-OFF → adapter DEVRE DIŞI; legacy tekil `on_event` +
çağrı-başına saat örnekleme birebir (INV-CS-5). **(iv)** Batch-içi tip önceliği NORMATİF:
DATA-ACK → probe-echo → aynı-tip-varış-sırası (DI-12E). **(v)** C2 post-condition birimi BATCH'tir;
tekil `on_event` (flag ON) = adapter'ın batch-of-1 sarmalaması.

### 0.4 Yürürlük kuralları
Spec-otoritesi > kod · sessiz silme YOK (CANCELLED+gerekçe) · ölçüm-hükmü-kapsam kuralı · arşiv
deseni · her netem koşusu öncesi faithful acceptance 6/6 · kesin-dönüşüm-uydurma yasağı ·
DUR-A=rapor-et-ve-devam · DUR-B=bekle-onay · DUR-SON=tam rapor+dur.

---

## 1. PROBLEM (FO/FP referans — yeniden kanıtlama YOK)
Sender saf-reaktif (self-timer yok; caller-clocked). PreConverge çıkışı yalnız `srtt.is_some()`
(sender.rs:1382-88); srtt yalnız dolu ACK'ten (1136; boş ACK 987-89). PTO donuk (1177). Tek
ACK'siz yol: geometrik kopya `(3/2)(size+65)/31 s` → 6.3/17.7/61.2s; alıcı 8s'de terk; blok-0
90s'de tamamlanmadı. Kökler: K1 kendini-uyandıramama · K2 boyut-bağımlı-ilerleme · K3
tek-srtt-kaynağı · K4 sabır-sözleşmesizliği.

---

## 2. TASARIM — NORMATİF

### 2.1 İnvariantlar
- **INV-CS-1 (control progress):** ilk probe **wire-emission** ≤ `T_FIRST_PROBE+PACER_ERR`;
  boyuttan bağımsız; sonraki fire'lar 9/21/45s. Feasibility kanıtı F2A'da (DI-1B/DI-6B).
- **INV-CS-2 (bütçe — C4):** **`B_COLD_PROBE_TX_BUDGET`** yalnız gönderici forward probe trafiği:
  `cold_probe_tx_bytes ≤ 176B nominal / 200B hard`. Echo ayrı: `cold_echo_tx_bytes ≤
  successful_probe_receives × ECHO_WIRE_MAX` — sayaç **alıcı defterinde**; harness shared-medium
  raporu iki defteri birleştirir. `cold_control_total_bytes = probe_tx + echo_tx`.
  `bootstrap_induced_retransmit_bytes` ayrı (§2.4-a). 31 B/s raporu İKİ modelde: full-duplex
  (forward: probe/forward_rate; reverse: echo/reverse_rate AYRI) + shared/half-duplex
  ((probe+echo)/shared_rate). Legacy geometrik kopya değişmediği için bütçe dışı.
- **INV-CS-3 (ilk bootstrap sonucu — SEÇENEK-A):** İlk sonuç tam bir: `ResolvedByDataAck ·
  ResolvedByProbeEcho · TimedOut`. TimedOut = terminal bildirim; oturum zorunlu kapanmaz; epoch
  kapanır, tüm outstanding probe_seq KESİN retire → timeout-sonrası PROBE ECHO'ları YOK SAYILIR.
  Timeout-sonrası **DATA-ACK** kurtarması = `PostTimeoutRecovery`; ikinci terminal ÜRETİLMEZ.
  Terminal tekliği eş-zamanda C3 batch sırasıyla garanti. **Epoch yaşam döngüsü:** oluşum =
  PreConverge'de `srtt=None` iken ilk uygun DATA'nın wire'a çıkışı; kimlik = sender-yerel monoton
  `bootstrap_epoch` (wire'a EKLENMEZ; alıcı sözleşmesine kaynak OLAMAZ); kapanış = ilk sonuç;
  PostTimeoutRecovery epoch'u yeniden açmaz; yeni epoch = YALNIZ session (yeniden-)kurulumu veya
  F0-tespit-(v) onaylı yeniden-giriş.
- **INV-CS-4:** cold-start'ta YENİ retransmit yolu YOK; kurtarma yalnız srtt-sonrası MEVCUT
  PTO/time-threshold/ARQ.
- **INV-CS-5 (kimlik/regresyon; EK5):** Flag OFF = adapter devre dışı, legacy tekil yol
  byte-for-byte özdeş (output-bytes, scheduling-order, pacing, counters, traces) + tüm pinler
  yeşil; deterministic event-sequence identity; **batch-of-1 ≡ legacy tekil çağrı** kimlik testi
  (flag ON). Flag ON + temiz-hızlı link: tekil-olay batch'lerinde wire-özdeş; çok-olaylı ns-eş
  batch'lerde frame KÜMESİ ve sırası özdeş; temiz-yavaş link (RTT>T_FIRST_PROBE) = ≤1 zararsız
  probe (belgelenir).
- **INV-CS-6 (data recovery — kapasite-bağlı):** `recovery ≤ t_probe_echo + PTO(srtt) +
  pacer_queue_delay + serialization + forward_delay`; `serialization ≥ unit_wire_bytes /
  usable_rate`; 31 B/s'de 1200B alt sınırı ~38.7s — altında iddia YASAK.
- **INV-CS-7 (probe_seq benzersizliği):** session ömrü boyunca monoton, yeniden KULLANILMAZ
  (epoch'lar arası dahil); lookup `(session_generation, probe_seq)`; session kurulumu yeni
  namespace; wrap → yeni probe YOK + typed diag; wire alan genişliği F0-tespit-(i).
- **INV-CS-RX-RESOURCE (yalnız WIRE-GÖRÜNÜR kimliklerle; patience EK4-A1):** alıcı cold-state
  sınırları: session/flow başına hard-cap (byte + incomplete-block [CAL]); "epoch başına bir blok"
  alıcı tarafında YOK; `Superseded/Cancelled` YALNIZ wire-görünür kimlikle; session close temizler;
  explicit-close bekleme ≠ sınırsız retention; cap aşımı sessiz eviction değil typed diagnostic.
  F0-tespit-(v) hükmü: aynı wire-görünür session içinde ayırt-edilemez PreConverge yeniden-girişi
  = DUR-B.

### 2.2 Mekanizma

**M1 — `on_event_batch` + `next_wakeup_ns()` + `Event::Tick` (EK5).** *Scope:* bağlanan timer'lar
= CS probe fire'ları, CS giveup, (d)-kapsamı PTO deadline'ları (re-arm'lar dahil, EK4-A2);
genel-tam-wakeup-API iddiası YOK. **BATCH API (NORMATİF):** `on_event_batch{now_ns,
inbound_events[]}` tek giriş noktası; `Event::Tick` ≡ boş batch. Dört faz sırayla, ATOMİK:
```text
1. Authenticated inbound olayları işle (tip önceliği: DATA-ACK → probe-echo → varış-sırası)
2. State resolution + cancellation/disarm: srtt ingest → bootstrap terminal result →
   probe ladder disarm → giveup cancellation → outstanding probe retire
3. Due timer evaluation (BİR KEZ): (a) CS probe fire (b) PTO (c) ColdStartTimeout
4. Output scheduling / queue drain
```
**OVERDUE-DEADLINE (C2, batch birimiyle):** bootstrap-SRTT ingest sonrası PTO deadline ≤ now_ns
ise DUE'dur → tetikleyen batch'in faz-3'ünde çalıştırılır (fire = mevcut ARQ yoluna enqueue; wire
emission pacing'e tabi). Geleceğe ÖTELENEMEZ; retire-on-fire; ACK/outstanding-temizliği ilgili
wakeup kayıtlarını retire eder. **Post-condition (batch sınırında):** dönüşte due exposed timer
kalmaz; `next_wakeup_ns()` ∈ {None, gelecek}. **ADAPTER SÖZLEŞMESİ (teslim edilen kod):** flag ON
iken TÜM production driver'lar adapter üzerinden; adapter saati iterasyon başına BİR KEZ örnekler,
tüm inbound'u aynı `now_ns` ile damgalar, TEK `on_event_batch` çağrısı yapar; sonrasında hâlâ due
varsa boş batch (Tick). Flag OFF → adapter DEVRE DIŞI. **NO-OP (E2 — YALNIZ Tick/boş-batch):**
due exposed timer VE bekleyen scheduling işi yoksa exact no-op (schedule_transmit/drain ÇAĞRILMAZ;
counter/trace değişmez — test). Inbound içeren batch'ler normal işleme + output scheduling'i
TAMAMLAR (DI-12G). *F1 envanteri:* TÜM production Sender driver call-site'ları + adapter'a
bağlanma kanıtı; bağlanmamış production driver = hüküm PARTIAL. Clock-domain: mevcut `now_ns`;
karışım YASAK.

**M2 — CS-Probe merdiveni.** Arm: PreConverge'de `srtt=None` iken ilk DATA'nın wire'a çıkışı.
Program: interval 3/6/12/24s → fire 3/9/21/45s; N=4; ≤44B/probe (amp≤1 pad); toplam ≤176B;
probe_seq: INV-CS-7 namespace. Disarm: srtt VEYA state-çıkışı VEYA epoch-kapanışı. Wire: spec'teki
`CTRL_PATH_PROBE_v2` cold-start varyantı BİREBİR (`seen_count`/`arrival_gap` varsa YALNIZ ledger).
Echo: mevcut flow state'inden idempotent, per-probe uzun-ömürlü allocation YOK; rate-limit
`T_ECHO_MIN`/akış; replay: mevcut kontrol-frame korumasıyla uyum (çelişki=DUR-B). **F2B GİRİŞ
KAPILARI:** (i) spec-statü + probe_seq genişliği (taslaksa normatifleştirme; 0x10-0x18
çelişkisi=DUR-B) · (ii) mixed-version matrisi çift-sütun (wire-güvenlik + feature-etkinlik;
safe-ignore değilse DUR-B) · (iii) replay+epoch binding · (iv) estimator-izolasyon planı ·
(v) echo tanımı/rate-limit · (vi) INV-CS-7 planı · (vii) **F2A YEŞİL** (C1).

**M3 — Echo → bootstrap-RTT (F2B).** Kabul: `(session_generation, probe_seq)` lookup;
tek-tüketim. RTT örneği `source=ProbeEcho, confidence=Bootstrap` etiketiyle YALNIZ RTT estimator'a
(matematik değişmez; TV-RTT golden'ları aynen). *İzolasyon (NORMATİF):* probe RTT ingest **cwnd,
BtlBw, delivery-rate, loss, ack-confidence, pacing-rate'i doğrudan değiştiremez**; izin verilen
tek state etkisi mevcut `srtt.is_some()` kapısının doğal PreConverge→Startup geçişi; Startup giriş
davranışı değişmez (N8-spot kanıtı). *Late-echo YOK-SAYMA (5):* data-ACK srtt oluşturduysa · seq
outstanding değilse · session_generation/epoch değiştiyse · echo işlendiyse · epoch TimedOut ile
kapandıysa. *(d)-uygulaması:* bootstrap-SRTT oluşunca AYNI iterasyonda: geçmiş-outstanding
birimler için MEVCUT PTO/loss-detection uygunluğu yeniden değerlendirilir → due ise C2 gereği
hemen fire → değilse deadline wakeup kümesine. **(EK4-A2) Yaşam döngüsü:** birim outstanding'ten
çıkana kadar ARQ'nun kurduğu HER yeni PTO deadline'ı otomatik expose; ACK/recovery/cleanup birimin
TÜM wakeup kayıtlarını retire eder (DI-1C).

**M5 — Typed `ColdStartTimeout` (F3).** Önkoşul: output-API denetimi. NORMATİF: epoch başına EN
FAZLA bir kez · ACK/echo srtt oluşturursa iptal · geç timeout üretilemez · timeout sonrası legacy
geometrik devam eder, **CS probe merdiveni TAMAMLANMIŞTIR, yeniden ARM EDİLMEZ** · timeout-sonrası
DATA-ACK kurtarması `PostTimeoutRecovery` (ikinci terminal YOK) · sürdür/kapat kararı uygulamanın.

**M6 — İki-uç sabır sözleşmesi (F3; EK4-A1).** `RX_COLD_ABANDON` = EK4-A1 max-formülü. **Karar
kuralı:** sabit 75s YALNIZ profil-türevi bound ≤75s ise; bound >75s profillerde ya
profile-derived patience ya **S-B session-hold + hard-cap ZORUNLU**. Kapsam kararı DUR-A'da
(Seçenek-1 ürün-alıcı dahil → tam hüküm / Seçenek-2 harness+spec → SENDER-SIDE/PARTIAL hüküm),
F0-tespit-(viii) tablosu önünde.

**M7 — Feature flag.** Mevcut konvansiyonla ad; default OFF; flag yalnız LOCAL; wire-güvenlik +
feature-etkinlik matrisle; default-flip bu turda YAPILMAZ.

### 2.3 [CAL] TABLOSU
| Sabit | İlk değer | Not |
|---|---|---|
| T_FIRST_PROBE | 3.0 s | temiz-yavaş linkte ≤1 zararsız probe |
| PROBE_INTERVALS | 3,6,12,24 s | fire 3,9,21,45 s |
| N_PROBE | 4 | probe toplam ≤176B |
| PROBE_WIRE_MAX | 44 B | amp≤1 pad dahil |
| B_COLD_PROBE_TX_BUDGET | 200 B (nominal 176B) | YALNIZ forward probe (C4) |
| ECHO_WIRE_MAX | F0 spec-tespitinden TÜRETİLİR | uydurulmaz |
| B_COLD_ECHO_TX_BUDGET | N_PROBE × ECHO_WIRE_MAX | alıcı defteri |
| T_COLD_GIVEUP | 60 s | son probe(45s)+RTT_MAX sonrası |
| RTT_MAX_SUPPORTED | 5 s | sözleşme paramı |
| RECOVERY_ALLOWANCE | 10 s | sözleşme paramı |
| RX_COLD_ABANDON | EK4-A1 max-formülü | profil×boyut başına F0-(viii) |
| T_NEXT_RECOVERY_ELIGIBLE | legacy programdan (1200B≈61.2s) | outstanding üzerinden max |
| MAX_RECOVERY_SERIALIZATION | max_unit_wire_bytes / usable_rate | profil-türevi |
| FORWARD_DELAY_BOUND | profil-türevi | F0-(viii) |
| RECEIVER_PROCESSING_MARGIN | 2 s | [CAL] |
| RX_COLD_BYTE_CAP / RX_COLD_BLOCK_CAP | F0'da mevcut buffer sınırlarından | session/flow |
| T_ECHO_MIN | 500 ms | echo rate-limit/akış |
| PACER_ERR | 250 ms | INV-CS-1 toleransı |
| T_W1_BOUND | formülden; W-1 ÖNCESİ hash+timestamp ile donar | kabul-matrisi W-1 |

### 2.4 Aile eşlemesi + bilinen maliyet
Tasarım = C (bounded probe budget) + D (ölçülmüş state-decoupling) + typed sonuç.
**§2.4-a — BİLİNEN MALİYET (NORMATİF KABUL):** ACK-only erasure altında bir mevcut-PTO
retransmission'ı **spurious olabilir** — target-delivery bilgisinin minimal kapsamda
bulunmamasının bilinen maliyeti. Correctness etkilenmez (alıcı seq-dedup atar); maliyet
bant/serialization; üst sınır bootstrap-anı outstanding kümesi. Çift muhasebe zorunlu (INV-CS-2);
31 B/s'de ek serialization açıkça raporlanır (1200B → ~38.7s). Kabul edilmezse çözüm DR-CS-2
(ACK-refresh/SACK-snapshot). Delivery-bilgisiz suppression YASAK.

### 2.5 Emisyon-DR arayüzü
M1 ortak primitif; burada yalnız CS+(d) timer'ları; parity/emisyon/blok-yaşam-döngüsüne dokunulmaz.

---

## 3. FAZLAR (talimat §3'ün özeti — bağlayıcı metin talimatta)
F0 (doc+test-only; 8 tespit; → DUR-A rapor-et-ve-devam) → F1 (M1 batch API + kanıtlar; F2B/F3-kapısı
batch-atomiklik) → F2A (probe-emission-feasibility; DI-1B/DI-6B) → F2B (M2+M3 full; kapılar
(i)-(vii)) → F3 (M5+M6) → F4 (kabul matrisi; faithful-acceptance-6/6-önkoşul) → F5 (RAPOR → DUR-SON).
