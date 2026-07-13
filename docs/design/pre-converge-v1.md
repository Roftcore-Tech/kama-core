# PRE_CONVERGE Cold-Start — Tasarım Notu (revize-D0)

> **Statü: KOŞULLU-ONAY İŞLENDİ — Abkarada diff onayı BEKLİYOR.** Bu belge, ham-D0'a (a) Abkarada
> onay-revizyon talimatının 6 kararı + 3 yeni bölümü ([K#]) ve (b) 15 onaylı adversarial-review
> bulgusunun ([R#]) işlenmiş halidir. Ham→revize diff ayrıca sunulur; her değişiklik §0.5
> changelog'unda [K#]/[R#] ile kaynağa bağlıdır. **Onaydan sonra D1 (spec patch) başlar.**
> **İş sırası:** revize-D0 onayı → D1 → D2+. **Otorite:** `transport-layer-tdd.md` (WHAT) >
> `adaptive-transport-engine-spec.md` (HOW) > bu not > kod. Bu not otoriteyle **çelişemez**.
> **Kapsam:** yalnız cold-start (motor hiç ölçüm yokken). Genel bandwidth-aware ölçekleme
> (ADIM-6 ana gövdesi) ayrı dilim; bu not referans verir, kapsamı genişletmez.
> **Hâlâ ONAY BEKLİYOR (byte finalize edilmedi):** §4.1 SACK-V2 ve §4.2 CTRL_PATH_PROBE genişletmesi
> byte-layout'ları — Karar 4/K3 gereği revize taslak olarak Abkarada'ya tekrar gelir.

---

## 0. Bu not neyi çözer

kbps testleri (2026-07-11) iki katmanlı, ölçülerek doğrulanmış kök-neden gösterdi:
- **Forward:** `INITIAL_RTT=100ms` + time-threshold → 1 kbps'te ilk ACK dönemeden (~1-2s) kayıp ilan
  → spurious retransmit fırtınası → temiz 1 kbps'te 3× byte. (`INITIAL_RTT=3s` diagnostic: 2250B→750B.)
- **Return:** receiver ACK cadence (20ms) → SACK seli 125 B/s dönüş borusunu tıkadı → teyit ~19s gecikti.
- **Sistemik:** `INITIAL_RTT`, ACK cadence, `min_rate` (fixture 10000 B/s=80 kbps floor!), probe/loss/
  emergency eşikleri — hepsi kbit-domain'e hardcoded, hiçbiri bandwidth-aware değil. Zarf: **0.25 kbps – Mbps.**

Bu not cold-start davranışını tasarım seviyesinde çözer: sabit-timeout modeli → "en-kötü-köşe davranış
önseli + termometre ölçüm" modeli.

**Otorite temeli:** TDD §10 (Cold-Start: "bant en kötü varsayılır, ama FEC YOK"); TDD §12/engine §12
(ACK decimation policy — implement edilmemişti); engine §12 (RETRY_BUDGET, PTO, IDLE_TIMER); engine §1
(wire taksonomisi — her datagram authenticated bir class'a ait); engine §16 (golden-vektör TV deseni);
TDD §13 (`SESSION_DATA = 50B + payload` fizik temeli).

## 0.5 Revizyon changelog — kaynak izlenebilirliği ([K#]=Abkarada kararı, [R#]=review bulgusu)

| kaynak | bölüm | değişiklik |
|---|---|---|
| **[K-KEYSTONE]** (+R5,R6,R7,R12,R13) | §2.3b, §3.B, §4.2 | Probe/echo **Noise ALTINDAN → Noise İÇİNE** (versiyonlu `CTRL_PATH_PROBE 0x12` genişletmesi, outer-AEAD güvenlik katmanında). Below-Noise KAT-donmuş INV-CRYPTO-1/engine §1 taksonomisini + §2a-madde-1'i ihlal ediyordu. Tek değişiklik **5 bulguyu eritir**: forge (R5), reflection (R6), DPI-cleartext (R7), taksonomi (R12), estimator-forge (R13). |
| **[K1]** | §3.B, Ek-A | B-opt1+keyed-token **SUPERSEDED**; ama 3 gereksinimi mekanizmadan-bağımsız yaşar: silent-drop, amp≤1 pad (AEAD içinde), responder rate-limit. Ek-A + Karar 1 SİLİNMEDİ, "SUPERSEDED by KEYSTONE" gerekçesiyle korundu. |
| **[K2]** | §2.4-return, §3.C, §9 | Minimal ACK-cadence **firm D2 co-requisite** (open-option kaldırıldı); FULL decimation → ADIM-6 ana. |
| **[K3]**+**[R2]** | §4.1 | SACK-V2 frame_type **0x11→0x14** (0x11=CTRL_MTU_PROBE çakışması); pencere-oranı ONAY; arrival_rate 0xFFFF doygunluk semantiği eklendi. |
| **[K4]** | §4.2 | Probe byte-layout **ONAY BEKLİYOR** olarak revize (CTRL ext yapısı; token kaldırıldı — AEAD kapsıyor). |
| **[K5]** | §5 | [CAL] formüller ONAY; sayılar açıkça "PLACEHOLDER — netem lowband'dan kalibre"; kalibrasyon-domaini sütunu her satırda, 4-decade doğrulama notu. |
| **[K6]**+**[R4]** | §3.A, §5, §6 | RETRY_BUDGET deadline-türevi + 2 normatif cümle (PATH_DEAD=yol-vazgeçişi; tek-yol→sessizlik-yasak). **[R4]:** failover IDLE_TIMEOUT'a değil **dedicated `no_ack(T)` timer**'ına bağlı (IDLE data-pending iken hiç ateşlenmez → PATH_DEAD erişilemezdi). |
| **[§2a]** | §3.D | Alpha DPI tehdit-modeli (3 sınırlı iddia). Keystone sonrası §2a-madde-1 ("plaintext magic yok") **artık geçerli** (0x50/0x51 cleartext gitti). |
| **[§2b]** | §2.2 | Backpressure paragrafı (pacer-tek-kapı; app floodu taban-bütçe dışına çıkamaz). |
| **[§2c]** | §1 | "Kapı yok ≠ fren yok" cümlesi. |
| **[R1]** | §2.2, §5 | k_base: akışın **ilk SACK'i cadence-exempt** (hemen emit) → RTT_floor ilk-SACK'in doğru modeli; formül fallback + TV worst-phase. |
| **[R3]** | §4.2, §5 | echo `arrival_gap` u16 µs → **u32 µs** (kbps gap'leri saniye; u16=65ms taşardı). |
| **[R8]** | §5, §6 | **min_rate=10000 floor PRE_CONVERGE'de MUST-NOT-APPLY** — cold-start pacing floor = PRE_CONVERGE_FLOOR_BW. (Steady-state floor → ADIM-6 ana.) *Notun tüm amacını sessizce çürütecek clamp.* |
| **[R10]** | §7 | Kabul eşitsizliği boyutsal düzeltme: `tx_wire ≤ altsınır × 1.15`. |
| **[R14]** | §2.4, §6 | PRE_CONVERGE'de `ack_confidence` **FULL sabitlenir** (§5.1 sessizlik-sinyali normal SACK gecikmesinde çöküp emergency-dup tetikliyordu — yasak-2'yi mekanizma seviyesinde kapatır). |
| **[R15]** | §5 | `sack_wire` 53B → **türetilmiş 65B** (ACK_FRAME_V2 gövdesi + §13 zarf); RTT_floor/ACK_CADENCE örnekleri güncellendi. |
| **[R-mb]** | §5 | MINI_BURST "0.25k: 0-1" → **"0.25k: 0"** (formül tabanda 0'a yuvarlıyor; amortize açıklaması). |
| **[C]** | §7 | Bench security-stub **+16B AEAD-tag-eşdeğeri padding** SİMÜLE eder → D2 byte ölçümleri v-gerçek Noise/AEAD katmanıyla **DEĞİŞMEZ** (ölçüm gerçek maliyeti şimdiden yansıtsın; sent_ns/tag-dolgusu dersi). |
| **[K-final]** | §2.3b, §3.D, §4.2, §5, §7, Ek-A | 2. aşama ek 5 cümle + A-doğrulaması: B8 SoC-guard (QUIC-bütünleşme DEĞİL); B4 `no_ack(T)` iki-sınır formülü; B1 sınıf-içi-özdeş (A: probe SESSION_DATA'dan 2B seq16 farklı → sınıf-ele-verme, tür değil) + §2a-3 sınıf-yüzeyi notu; B3 çift-kapı floor-contamination ispatı; D-note reddedilen alternatifler. |

---

## 1. Temel ilke — KAPI YOK (ama FREN VAR), TERMOMETRE VAR

Veri akışını hiçbir ölçüm/handshake/probe BLOKLAMAZ. Motor t=0'da gönderir; ölçüm gönderilenin yanında
toplanır. QUIC'in p5 ölümü handshake'in **boyutu** değil **kapı** olmasıydı (baseline: hs-fail %40-60,
udp_tx=0; connectionless Kama orada c(10s)=1.00 vs quinn 0.00). Turnusol: **bir bileşen veri akışını
bekletiyorsa tasarım yanlıştır.**

**[§2c] Kapı yok ≠ fren yok:** hiçbir şey akışı BLOKLAMAZ (kapı), ama her şey taban-pacing'le ÇIKAR
(fren). QUIC'in hatası freni kapıyla karıştırmaktı. Cold-start'ın iki hipotezi ("kanal dar mı kayıplı
mı?") cevaplanmaz; motor ikisini birden kapsayacak şekilde **davranır** (en-kötü-köşe pacing = fren)
ve ölçüm birikince kademeli tahliye eder.

---

## 2. Mimari — dört bileşen + operatör önseli

### 2.1 Davranış önseli: fuzzy en-kötü-köşe (≡ TDD §10 LOW_BANDWIDTH_STRICT)
- Açılışta motor fuzzy en-kötü köşesi gibi **DAVRANIR**: `bw = PRE_CONVERGE_FLOOR_BW`, `loss = extrem`.
  Pacing bu tabana kurulur → zarf-içi hiçbir kanalda self-congestion **inşa gereği** imkânsız.
- **Önsel DAVRANIŞTIR, ÖRNEK DEĞİL:** hiçbir estimator'a yazılmaz; estimator'lar cold-start'ta BOŞ.
- **İlk geçerli ölçümle KOŞULSUZ tahliye.** Prob takvimi önselin geçerliliğinden bağımsız sürer.
- **Dominance (bilinçli takas):** kanal dipteyse davranış doğru; iyiyse ilk echo/SACK saniyeler içinde
  önseli öldürür → yanılma cezası kendini sınırlar. (`min_rate=10000` dersinin kurumsallaşması:
  taban = ürün floor'u, kbit fixture değil — bkz. [R8] §5/§6.)
- **Spec cümlesi (aynen §3'e):** *"PRE_CONVERGE'de motor fuzzy en-kötü-köşe önseliyle DAVRANIR; bu
  önsel hiçbir estimator'a örnek olarak yazılmaz, ilk geçerli ölçümle koşulsuz tahliye edilir ve prob
  takvimi önselin geçerliliğinden bağımsız sürer."*

### 2.2 Veri: programlı geometrik kopya takvimi ([CAL] — serialization fiziğinden)
- İlk birim t=0'da gider; **ACK beklenmez**. Taban ve çarpan UYDURULMAZ; taban-bw serialization
  fiziğinden türetilir (§5). "Kayıp mı yavaş mı?" cevaplanmaz; takvim iki hipotezi de kapsar.
  RECOVER_FWD'nin cold-start uzantısı; tetik "henüz hiçbir ölçüm yok".
- **[R1] Stop koşulu (byte-budget için ZORUNLU):** birimin kopyaları o birim SACK ile teyit edilince
  DURUR. **Akışın ilk SACK'i cadence-exempt** — ilk DATA gelişinde ACK_CADENCE'i atlayıp HEMEN emit
  edilir (yalnız sonraki SACK'ler rate-limitli). Böylece ilk SACK ~RTT_floor'da gelir (§5), taban
  `k_base·RTT_floor > RTT_floor` seçilir → temiz kanalda SACK ilk kopyayı **ateşlemeden iptal eder**
  → tx_wire/altsınır ≈ 1× (kabul §7 ≤1.15×). (İlk-SACK cadence-exempt OLMASAYDI k_base ≥
  1+ACK_CADENCE/RTT_floor ≈ 1.6-2.3 gerekirdi — §5 fallback.) Kayıplı kanalda kopya ateşlenir.
  Sayı-tavanı YOK ([K6] §3.A): aralık `COPY_MAX_INTERVAL`'e kadar büyür, deadline'a dek düşük tempoda sürer.
- RX_DUP_SUPPRESS (receiver.rs) fazla kopyaları semantik zararsız kılar.
- **[§2b] Backpressure:** uygulama sunumu sınırsız; wire çıkışı **PACER-TEK-KAPI** (token-bucket §13);
  kuyruk sınırı §14 (inflight/memory cap + bounded pools). PRE_CONVERGE'de pacer taban-tempoda →
  uygulama 2M birim sunsa bile wire'a **taban-bütçe dışında tek byte çıkmaz**; kuyruk dolunca
  backpressure uygulamaya döner. ("Cold-start'ta app floodlarsa" sorusunun spec cevabı.)

### 2.3 Ölçüm: zengin SACK + probe/echo — İKİSİ DE Noise İÇİNDE, authenticated

**[K-KEYSTONE] Katman kararı:** ham-D0 probe'u Noise ALTINA koyuyordu; bu, KAT-donmuş INV-CRYPTO-1 /
engine §1 taksonomisi (unauthenticated transport-emitted class YOK) ve §2a-madde-1 ("plaintext magic
yok") ile çelişiyordu (R12/R7), ayrıca on-path echo-forge (R5) + estimator-forge (R13) yüzeyi açıyordu.
**Çözüm:** probe/echo = **authenticated hop-control frame** (versiyonlu `CTRL_PATH_PROBE 0x12`
genişletmesi), outer-AEAD güvenlik katmanında (§0.2), hub_handle ile demux. Bu tek değişiklik R5/R6/R7/
R12/R13'ü birden eritir. **INV-ENGINE-SOC korunur:** transport anahtar TUTMAZ — AEAD'i güvenlik katmanı
yapar, transport opak metadata (probe_seq/seen_count/arrival_gap) okur.
**[K-final] SoC-guard:** *Probe güvenlik katmanının birimidir; transport yalnız içeriği (probe_seq/ölçüm)
üretir, güvenlik dikişinden opak blob olarak geçer — AEAD'i transport DEĞİL güvenlik katmanı uygular.
Bu, QUIC-tipi transport-kripto bütünleşmesine YAKLAŞTIRMAZ (INV-ENGINE-SOC): keystone yalnız probe'un
taşındığı zarfı değiştirdi, katman sınırını değil.*

**Akış-bağlılık kaydı ("B-opt1 varken kayıp yok"):** probe yalnız **kurulmuş akış içinde** üretilir
(ilk DATA sonrası, aynı hub_handle). Akış kurulmuşsa Noise transport-anahtarı **zaten vardır** → AEAD
maliyeti yalnız +16B tag, yeni handshake YOK, INIT-öncesi authenticated-probe penceresi sorunu YOK
(probe INIT'ten önce hiç üretilmez).

**(a) Zengin SACK (§4.1 taslak, ONAY BEKLİYOR):** `seen_count` (yön-ayrımlı kayıp: forward-loss vs
return-loss, p5 ayrımı) + `arrival_rate` (bw ilk örnek). Noise üstü, authenticated. D2 eski formatta;
genişletme D3.

**(b) Probe/echo (§4.2 taslak, ONAY BEKLİYOR):** amaç dar kanalda ucuz öğrenme (0.25 kbps'te 49B probe
~2.5s vs 306B data ~9.8s → ~4× ucuz). **[K1] mekanizmadan-bağımsız gereksinimler (AEAD içinde aynen):**
(i) **silent-drop** — geçersiz/AEAD-çözülemeyen probe'a TEK BYTE cevap yok (aktif-tarama savunması artık
AEAD-doğrulamasıyla; davranış aynı); (ii) **amp ≤ 1** — probe, echo'yu domine edecek `pad`'le (AEAD
zarfı içinde); (iii) **responder rate-limit** — garbage'a AEAD-decrypt denemesi ucuz ama bedava değil,
DoS taban sınırı.
- **Statü cümlesi (aynen spec'e):** *"Probe hızlandırıcıdır; hiçbir şeyi bloklamaz, receiver'da YENİ
  state açmaz (akışın mevcut demux/SACK state'ini kullanır), yokluğu VEYA geçersizliği otoriter sonuç
  doğurmaz. Yokluğunda sistem yalnız daha yavaş öğrenir."*

### 2.4 Kademeli tahliye — sinyal başına AYRI kilit
- **İlk geçerli SACK/echo → RTT + loss kilidi açılır** (gerçek örneklerle). Probe/echo artık
  authenticated (keystone) olduğundan echo-türevi RTT güvenilir → kilit açabilir (R5/R13 eridi:
  forge imkânsız). ARQ artık RTT-temposunda.
- **bw kilidi AYRI:** taban varsayımı, ölçülmüş bw örnekleri **birikene** kadar sürer (tek örnek "bant
  VAR" der, "ne kadar" DEMEZ). FEC/redundancy agresifliği bw-kanıtıyla kademeli açılır — `min_rate`
  dersinin kurumsallaşması + duplication bw-throttle GAP'inin (sender.rs `budget_cap`/`parity_final`
  çağrılmıyor) kapanması: redundancy bw-kilidine bağlı, kilit açılana kadar taban-tempoda.
- **PRE_CONVERGE yasakları (spec'e numaralı):** (1) loss ilanı YOK; (2) emergency duplication YOK
  (TDD §10 "kör escalate ETME"); (3) sessizlikten estimator besleme YOK; **(4) [R14] `ack_confidence`
  (§5.1) DEĞERLENDİRİLMEZ — FULL (1000) sabitlenir.** Gerekçe: sessizlik-türevi reverse-path sinyali
  cold-start'ta çöküş değil taban-bw serialization gecikmesidir (threshold=ACK_SILENCE_MULT×srtt ≈300ms
  ≪ RTT_floor). TÜM tüketicileri (§3 ACTIVE→RECOVER_FWD, §8.4/§15 fuzzy, §11 emergency Schmitt, §12
  decimation) ilk gerçek dönüş örneğine kadar DONDURULUR → çift-redundancy tetiği ve iki-cadence
  çakışması yok; §2.2 geometrik takvim TEK forward-redundancy sürücüsü, C-maddesi cadence TEK return sürücüsü.
- **[K2][R4] Return-path (firm D2 co-requisite):** receiver ACK cadence de cold-start yapar — dönüş
  bw'sini bilmediğinden taban-önselle başlar (`ACK_CADENCE ≥ sack_wire/(Y·return_bw_önseli)`), DATA
  arrival-rate'iyle hızlanır. İlk SACK cadence-exempt ([R1]). Bu 19s-lag'in çözümü. FULL adaptif
  decimation (reverse-path-impaired N↑ eskalasyonu, per-profil budget) ADIM-6 ana gövdesi.

### 2.5 Operatör önseli
- Config `bandwidth_class` cold-start tabanını (`PRE_CONVERGE_FLOOR_BW`) ezer. **YALNIZ önseli değiştirir,
  ölçümü ASLA ezemez** — ilk gerçek örnekte çöpe.

---

## 3. Çözülen tutarlılık noktaları (otorite lehine)

### 3.A [K6][R4] Persistence vs RETRY_BUDGET / PATH_DEAD
İki KATMAN ayrı:
1. **Retry/kopya SAYISI cap'lenmez** — geometrik takvim **deadline**'la sınırlı (bounded store-and-
   forward). `RETRY_BUDGET` = sabit-sayı DEĞİL, deadline-türevi (`deadline/COPY_MAX_INTERVAL`).
2. **[R4] PATH_DEAD ayrı escalation, dedicated timer'la** — failover `IDLE_TIMEOUT`'a **BAĞLANMAZ**
   (engine §5.1: data-pending iken endpoint `active`, IDLE hiç ateşlenmez → PATH_DEAD erişilemez olurdu).
   Bunun yerine **`no_ack(T)→PATH_DEAD`** (TDD §9/§10), T = IDLE_TIMEOUT'tan **AYRI** yeni [CAL] sabiti;
   silence = `now − max(last_ack, first_send)` üzerinden ölçülür (data-pending iken monoton artar →
   erişilebilirlik ispatlı). IDLE_TIMER → IDLE (session korunur) rolü **DEĞİŞMEZ**. Failover sorumluluğu
   RETRY_BUDGET'tan `no_ack(T)`'ye **taşınır** (kaybolmaz).
3. **[K6] İki normatif cümle:** (a) **PATH_DEAD = yol-vazgeçişi, teslim-vazgeçişi DEĞİL** — failover'a
   eskalasyon; mesaj kuyruktan yalnız deadline'la düşer. (b) **Tek-yol durumu:** failover edecek
   alternatif hub YOKSA PATH_DEAD davranışı = geometrik kuyruğun ucunda düşük-tempo; **TAM SESSİZLİK
   YASAK** (blackout-survival; TV-COLDSTART-5 "tek-yol + echo'suz" varyantını kapsar).

### 3.B [K1][K-KEYSTONE] Probe güvenliği — keystone sonrası
Keystone (probe Noise-içi) fingerprint/forge/reflection/taksonomi yüzeylerini kapattı. **Ek-A + Karar 1
(below-Noise + keyed-token) SUPERSEDED** ama silinmedi (§Ek-A). Karar 1'in gereksinimleri (silent-drop,
amp≤1, rate-limit) mekanizmadan-bağımsız yaşamaya devam (§2.3b). Keyed-token artık gereksiz — AEAD onu
kapsar (proof-of-knowledge = geçerli AEAD tag). Kalan residual: authenticated bir peer'ın kendi akışında
davranışı — akış-içi, meşru; ek yüzey yok.

### 3.C [K2] Return-path ACK cadence — firm D2 co-requisite
Minimal `ACK_CADENCE ∝ return-serialization` TV-COLDSTART-1 için D2 co-requisite'idir (task §5.3 TV-1'i
D2'de tutar). "D3'e ötele" open-option'ı KALDIRILDI (task §5.3 ile çelişiyordu). FULL adaptif decimation
→ ADIM-6 ana gövdesi (dilim şişmez).

### 3.D [§2a] Alpha DPI tehdit-modeli — üç SINIRLI iddia
1. Wire'da plaintext invariant/magic YOK; Noise sonrası payload'lar ciphertext-random. **(Keystone
   sonrası probe/echo de AEAD içinde → 0x50/0x51 cleartext gitti → bu madde artık probe için de geçerli.)**
2. Aktif taramayla keşfedilemezlik: silent-responder (§2.3b) — geçersiz/AEAD-çözülemeyen probe'a sıfır cevap.
3. Tam trafik-ayırt-edilemezlik (boy/zamanlama şekillendirme, cover traffic) = **Beta+ yol haritası**;
   Alpha İDDİA ETMEZ. **[K-final] Sınıf-yüzeyi notu (A doğrulaması):** probe/echo dış çerçevesi CTRL-sınıfıyla
   özdeş ama SESSION_DATA'dan 2B (seq16 yokluğu) + AEAD-katmanıyla ayrışır → pasif gözlemci CTRL-sınıfını
   ayırt edebilir (**sınıf-ele-verme, TÜR değil** — probe diğer CTRL'lerden ayırt edilemez). Bu istatistiksel
   yüzey Alpha'nın kapatmadığı bu üçüncü maddeye dahildir (tasarım değişikliği değil, dürüst kayıt).
Bağlam: QUIC/KCP'nin DPI'da düştüğü yer (1)+(2) ihlalleri (QUIC Initial tanınabilir, KCP header çıplak);
Alpha bu ikisini kapatır, üçüncüyü vaat etmez. "DPI-dirençli" sıfatı bu üç maddeye referanssız KULLANILMAZ.

---

## 4. Wire taslakları — ONAY BEKLİYOR (byte yalnız onaylı haliyle D1'e girer)

> Fixed-width big-endian, varint YASAK (ADR-05/ccr §35). frame_type'lar engine §1.1 tablosuna karşı
> D1'de mutabakatlanır (mevcut: 0x10 ACK, 0x11 MTU_PROBE, 0x12 PATH_PROBE, 0x13 REKEY, 0x20 FEC; boş: 0x14-0x1F).

### 4.1 [K3][R2] Extended SACK — öneri (frame_type 0x14)
```
ACK_FRAME_V2 {
  frame_type    : u8       # 0x14  (0x10=V1 korunur; 0x11 DEĞİL — CTRL_MTU_PROBE ile çakışırdı, R2)
  ack_epoch     : u8
  largest_seq   : u16_be
  ack_delay     : u16_be   # µs
  block0_len    : u16_be
  range_count   : u8       # ≤ MAX_ACK_RANGES (16)
  ranges[range_count] { gap:u16_be; len:u16_be }
  # ── V2 ölçüm eki ──
  seen_count    : u32_be   # görülen toplam DATA (birim+kopya) → yön-ayrımlı loss
  arrival_rate  : u16_be   # receiver-gözlemli pencere-oranı B/s (ÖNERİLEN; packet-pair alternatifi red)
}
# 15B sabit + 4B×range_count. Noise üstü, authenticated.
```
- **[K3] arrival_rate doygunluk semantiği (u16 tavan 65535 B/s ≈ 524 kbps):** iki seçenek —
  **(ÖNERİLEN)** `0xFFFF = "ölçüm üst-bantta, cold-start ipucu olarak anlamsız"` (sentinel; cold-start'ta
  yeterlilik ölçüt, hassasiyet değil); ya da log-ölçekli/kademeli kodlama. Sentinel öneriliyor (basit,
  cold-start amacına yeter).

### 4.2 [K4][K-KEYSTONE][R3] Probe/echo — öneri (Noise-içi CTRL_PATH_PROBE genişletmesi, versiyonlu)
```
# CTRL_PATH_PROBE (0x12) — MEVCUT semantik korunur (versioned; sessiz kırma YOK).
# Yeni cold-start ölçüm varyantı (version/subtype alanıyla ayrışır); TÜMÜ hub_handle demux + outer-AEAD.
CTRL_PATH_PROBE_v2 (probe) {           # sender→receiver, kurulmuş akış içinde
  [L2] hub_handle : u32_be             # demux (mevcut, cleartext L2)
  [AEAD zarfı içinde:]
    subtype       : u8                 # cold-start probe alt-tipi (0x12 semantik versiyonlama)
    probe_seq     : u16_be             # echo eşleştirme
    pad           : u8[N]              # amp≤1: probe_wire ≥ echo_wire olacak N (AEAD içinde)
  [AEAD tag : 16B]
}
CTRL_PATH_PROBE_v2 (echo) {            # receiver→sender, stateless (akışın mevcut state'i)
  [L2] hub_handle : u32_be
  [AEAD zarfı içinde:]
    subtype       : u8                 # echo alt-tipi
    probe_seq     : u16_be             # yansıtılan
    seen_count    : u32_be             # şu ana kadar görülen DATA
    arrival_gap   : u32_be             # [R3] son iki DATA/probe arası µs — u32 (u16=65ms kbps'te taşardı; kbps gap'leri saniye)
  [AEAD tag : 16B]
}
# echo AEAD-gövdesi = 1+2+4+4 = 11B (+handle 4 +tag 16 = 31B wire). nonce ARTIK YOK (AEAD anti-forge/replay sağlar).
# token ARTIK YOK (AEAD proof-of-knowledge'ı kapsar). pad, echo'yu (11B gövde) domine edecek → amp≤1.
```
- **Değişiklik notu:** ham-D0'ın `nonce(8B)` + Karar 4'ün `token(~16B)` alanları **kaldırıldı** — AEAD
  ikisinin işlevini (anti-forge/replay + proof-of-knowledge) kapsar. Byte-layout ONAY BEKLİYOR (K4).
- **[K-final] Wire görünümü (A doğrulaması — byte kanıtı):** AEAD nonce cleartext DEĞİL — gerçek nonce
  HKDF ile reconstruct edilen full counter'dır (ccr §35.6/35.8: wire `seq16` "TEK BAŞINA nonce DEĞİL",
  resync hint). Probe cleartext nonce/sayaç TAŞIMAZ. Probe dış çerçevesi = `hub_handle(4B) + ciphertext +
  tag(16B)` → **CTRL sınıfı-İÇİ ÖZDEŞ** (ACK/MTU-probe/path-probe/rekey ile birebir; frame_type/subtype
  AEAD İÇİNDE → tür-ele-verme YOK). **SESSION_DATA'dan farklı:** o `hub_handle(4B)+seq16(2B)` (6B cleartext
  prefix, +2B) + inner-E2E AEAD taşır → pasif gözlemci **CTRL-sınıfını SESSION_DATA'dan ayırt eder ama
  probe'u diğer CTRL'lerden ayırt EDEMEZ** = **sınıf-ele-verme, TÜR değil** (§2a-madde-3). D1'de probe
  dış-framing'i CTRL sınıfının konvansiyonuna birebir uydurulur.
- **[D1-reconciled] Byte-layout finalize (D1 review R4-fix):** §1.1 "outer-AEAD sonrası ilk byte =
  frame_type" konvansiyonu gereği probe/echo AEAD-içine `frame_type(1)=0x12` eklendi (ACK_FRAME_V2 ile
  aynı) → echo gövdesi 11B→**12B**, wire 31B→**32B**; pad 32B'yi domine (amp≤1 korunur). Otorite: engine
  §11.1 (bu belgedeki 31B taslak D1'de reconcile edildi — D0 line "D1'de uydurulur" gereği).

---

## 5. [CAL] sabitleri — türetme formülleri + kalibrasyon domaini

> **[K5]** Formüller ONAY; **sayılar PLACEHOLDER — netem lowband baseline'ından kalibre edilecek** (golden
> vektörlere yalnız ÖLÇÜLMÜŞ değer işlenir, engine §16). "tüm bw" satırları için formülün 4-decade
> geçerliliği kalibrasyonda ayrıca doğrulanır (0.25k / 1k / 100k / 1M noktaları). Fizik: TDD §13
> `SESSION_DATA=50B+payload` (UDP/IPv4). 1 kbps=125 B/s; 0.25 kbps=31.25 B/s.

| sabit | formül (türetme) | kalibrasyon domaini | PLACEHOLDER örnek |
|---|---|---|---|
| `PRE_CONVERGE_FLOOR_BW` | ürün floor; operatör `bandwidth_class` ezer | önsel, tüm bw | 31.25 B/s (0.25 kbps) |
| **[R8]** cold-start pacing floor | **= PRE_CONVERGE_FLOOR_BW; min_rate=10000 fixture PRE_CONVERGE'de MUST-NOT-APPLY** (§6) | PRE_CONVERGE | 31.25 B/s |
| **[R15]** `sack_wire` | `28(UDP/IPv4)+22(handle4+seq2+tag16)+ACK_FRAME_V2 gövde(15+4·range)` — standalone SACK, 0 range | tüm bw | **65B** (69B @1 range) |
| `RTT_floor(unit)` | `(unit_wire+sack_wire)/floor_bw + PROP_MARGIN` | pace edilen taban | 256B@0.25k: (306+65)/31.25 ≈ **11.9s**; 64B@1k: (114+65)/125 ≈ **1.43s** |
| **[R1]** `COPY_BASE_FACTOR` (k_base) | `>1`; ilk-SACK cadence-exempt (§2.2) → RTT_floor ilk-SACK modeli → k_base 1.3-1.5. **Fallback** (exempt değilse): `k_base ≥ 1+ACK_CADENCE/RTT_floor` (≈1.6 @256B/floor, ≈2.3 @64B/floor) | tüm bw | **1.3-1.5** [PLACEHOLDER] |
| `COPY_MULTIPLIER` | geometrik backoff, RFC 6298 analoğu | tüm bw | **2** [STD] |
| `COPY_MAX_INTERVAL` | `deadline/MIN_COPIES_IN_DEADLINE` — aralık tavanı (süresiz düşük tempo) | deadline-türevi | deadline/8 [PLACEHOLDER] |
| **[R4][K-final]** `T_PATH_DEAD` | `no_ack(T)→PATH_DEAD` eşiği (IDLE_TIMEOUT'tan AYRI); silence=`now−max(last_ack,first_send)`. **İki sınır:** `geometrik-takvim-span < T < deadline − failover_marjı` — alt: tüm kopyalar şans bulmadan ateşlenmesin; üst: PATH_DEAD deadline'dan ÖNCE ateşlensin ki failover'ın davranma süresi kalsın (yoksa yol-vazgeçişi teslim-vazgeçişine dejenere olur). `failover_marjı` [CAL] | tüm bw | [PLACEHOLDER] |
| `RETURN_BUDGET_FRACTION` (Y) | SACK+echo dönüş ≤ Y·return_bw | tüm bw | **0.20-0.30** [PLACEHOLDER] |
| **[R15]** `ACK_CADENCE(return_bw)` | `sack_wire/(Y·return_bw)`; ilk SACK cadence-exempt ([R1]) | tüm bw | 0.25k: 65/(0.25·31.25)≈**8.3s**; 1k: ≈**2.1s**; fast: min-floor ~20ms |
| `PROBE_PAYLOAD_SIZE` | `probe_baş+pad`, `pad=echo_gövde−probe_baş+margin` → amp≤1 (AEAD içinde) | tüm bw | ~49B wire (echo 31B'yi domine) [PLACEHOLDER] |
| `PROBE_BUDGET_FRACTION` (Z) | probe forward ≤ Z·forward_bw | tüm bw | **0.10-0.20** [PLACEHOLDER] |
| **[R-mb]** `MINI_BURST_COUNT` | `min(TARGET, floor(Z·forward_bw·1s/probe_wire))` — opening 1s-burst | tüm bw | fast: 3-5; **0.25k: 0** (formül tabanda 0'a yuvarlar; floor probe'ları ~10-20s'de bir amortize, burst değil) [PLACEHOLDER] |
| `PROBE_RATE_LIMIT` | responder kaynak-başına echo/s (DoS taban; garbage AEAD-decrypt ucuz≠bedava) | responder | [PLACEHOLDER] |

**Not:** `RTT_floor`, `ACK_CADENCE`, `MINI_BURST_COUNT` **fonksiyon**dur (bw'ye göre değişir) —
"kbps-aware" tam olarak budur: eşik bir sayı değil, `unit_wire/bw` fiziğinin türevi.

---

## 6. Spec patch haritası (D1'de icra — onaydan sonra)

| # | hedef | değişiklik | tür |
|---|---|---|---|
| 1 | engine §3 + §12 | **PRE_CONVERGE** durumu: giriş, yasaklar (§2.4/1-4 **[R14] ack_confidence FULL dahil**), kademeli tahliye, çıkış | yeni state |
| 2 | engine §3 | fuzzy-önsel kuralı (§2.1) aynen | normatif cümle |
| 3 | engine §2 | Extended SACK **0x14** (§4.1) — **onay sonrası** byte-layout | wire (ONAY) |
| 4 | engine §1.1 + yeni § | **CTRL_PATH_PROBE 0x12 versiyonlu genişletme** (§4.2, Noise-içi) + statü cümlesi — **onay sonrası** | wire (ONAY) |
| 5 | engine §12 | ACK_CADENCE bandwidth-aware ([K2]); RETRY_BUDGET deadline-türevi + **`no_ack(T)→PATH_DEAD`, T≠IDLE_TIMEOUT [R4]**; IDLE_TIMER rolü değişmez | normatif revizyon |
| 6 | engine appendix | eski `INITIAL_RTT`/sabit-timeout **CANCELLED — MUST NOT IMPLEMENT**; **[R8] `min_rate=10000` fixture PRE_CONVERGE'de MUST-NOT-APPLY** (steady-state floor → ADIM-6 ana) | deprecate |
| 7 | engine §16 | §5 sabitleri + **kalibrasyon-domaini sütunu** ([K5], 4-decade doğrulama) | [CAL] |
| 8 | engine §16 | TV-COLDSTART-1..5 (+ [R14] pre-first-SACK RecoverFwd/emergency=OFF assertion; [K6] tek-yol varyantı) | yeni TV |

---

## 7. Kabul kriterleri + test vektörleri (ölçülür)

**Kabul (task §6):**
- **Temiz 1 kbps: 10/10**; **[R10]** `tx_wire ≤ altsınır × 1.15`, altsınır = `N·(50+payload)` (bench
  stub `N·(39+payload)` — ikisi raporlanır). *(Boyutsuz redundancy oranı; ham-D0'daki `/payload` payda hatalı.)*
  **[K-final] Floor-contamination guard (B3):** bu çift kapı (`tx_wire ≤ 1.15·altsınır` + 10/10) hayatta
  kalan bir `min_rate=80kbps` floor'unu **mekanik yakalar** — floor 1 kbps kanalı 64× floodlar →
  self-congestion → `tx_wire` 1.15×'i aşar VE completion <10/10 → TV-COLDSTART-1 FAIL. (R8'in test koruması.)
- **0.5 / 0.25 kbps:** fizik-türevi deadline (`k·ideal`, `ideal=N·unit_wire/bw`; `k∈{2,3,4}`), pozitif completion.
- **Dönüş byte'ı:** SACK+echo ≤ `RETURN_BUDGET_FRACTION·return_bw`.
- **[C] Ölçüm metodolojisi (v-gerçek katmanla değişmezlik şartı):** bench security-stub her frame'e
  **+16B AEAD-tag-eşdeğeri padding** ekler (identity-stub payload'ına) → `altsınır`/`tx_wire` ölçümleri
  gerçek Noise/AEAD maliyetini **ŞİMDİDEN** taşır → D2 kabul ölçümleri v-gerçek katmanla **DEĞİŞMEZ**
  (sent_ns/tag-dolgusu dersinin gereği: ölçüm gerçek maliyeti baştan yansıtsın). `altsınır=N·(50+payload)`
  zaten §13 zarfının 16B tag'ini içerir; stub'ın 39B başlığı **+16B pad** ile bu maliyeti temsil eder.
- **Üst-bant regresyon YOK:** v1 matrisi p4/p5 yeniden koşulur, 1.00 korunur. (p7 vb. floor-contaminated → ADIM-6 ana v2.)

**TV (engine §16, golden-vektör):**
- **TV-COLDSTART-1** (D2): temiz 1 kbps 10/10; `tx_wire ≤ 1.15·altsınır`; **[R1] worst-phase:** her birim
  boyutunda (64B@floor dahil) ilk SACK `k_base·RTT_floor`'dan önce → spurious ilk-kopya YOK; **[R14]**
  ilk gerçek SACK öncesi `state≠RecoverFwd ∧ emergency_level=Arq ∧ decimation=OFF`. **[C] D2 dilim şartı:**
  bench stub **+16B AEAD-tag padding aktif** — byte ölçümleri (`tx_wire`/`altsınır`) v-gerçek Noise
  katmanıyla değişmez (gerçek maliyet baştan ölçülür).
- **TV-COLDSTART-2** (D3): %95 loss'ta tek SACK'ten RTT + yön-ayrımlı loss; **bw kilidi AÇILMAZ**.
- **TV-COLDSTART-3** (D4): hızlı kanal (100kbit+%90 loss) ilk saniyelerde yakalanır.
- **TV-COLDSTART-4** (D4): 0.25 kbps'te probe bütçesi `Z·forward_bw`'yi AŞMAZ.
- **TV-COLDSTART-5** (D5): echo'suz koşuda motor asla tam sessizliğe düşmez; **[K6] "tek-yol + echo'suz"
  varyantı** — failover hub yokken de geometrik kuyruk sürer (tam sessizlik yasak).
- **[K1] negatif-testler:** geçersiz-token/AEAD'e sessizlik; akış-dışı probe'a sessizlik; amp≤1.
- **Sessizlik-çıkarım-yok** (task §6): PRE_CONVERGE'de N timeout sonrası estimator'lar BOŞ.

**Dilim disiplini (task §5):** her dilim TEK değişken + öncesi teşhis dekompozisyonu + sonrası AYNI
dekompozisyonla retest. "Düzeldi" hükmünü ÖLÇÜM verir.

---

## 8. Kapsam dışı (kayda geçer)
- **Fountain-tarzı init → Beta+.** Tek-paket mesajda kopya-takvimine dejenere; değeri çok-paketli yükte;
  INV-FEC-1 sınırına dokunur. (Buradaki takvim taban-bütçeli+geometrik-seyrekleşen → ölçüsüz bulk-first ≠.)
- **Timestamp-tabanlı tek-yön ölçüm → ELENDİ** (hostile'da saat senkronu yok).
- **B-opt2** (DATA-ayırt-edilemez şekillendirme) → Beta+ (§3.D-madde-3).
- **FULL bandwidth-aware ACK decimation eskalasyonu** → ADIM-6 ana gövdesi; D0 yalnız minimal tabanı ([K2] §3.C).

---

## 9. Onay durumu (revize sonrası)

§9'un ham-D0'daki 6 açık maddesi Abkarada talimatıyla KARARA bağlandı ([K1-K6]). Kalan **tek ONAY-BEKLİYOR:**
- **§4.1 SACK-V2 (0x14) + §4.2 CTRL_PATH_PROBE genişletmesi byte-layout'ları** — Karar 4/K3 gereği revize
  taslak olarak Abkarada'ya bu belgeyle gelir. Onaydan sonra D1 bunları birebir işler.

---

## Ek-A — SUPERSEDED: below-Noise probe + keyed-token (Karar 1) — kayıt disiplini

> **[K1 gereği SİLİNMEDİ]** Bu model D0 yazımı sırasında iletilmemiş bir tasarım turunda doğdu ve
> Abkarada'nın onay-revizyon talimatında Karar 1 olarak verildi. Adversarial-review sonrası KEYSTONE
> (probe Noise-içi) tarafından **SUPERSEDED** edildi. Neden vazgeçildiği üç ay sonra iz kaybolmasın diye
> burada kalıyor:

**Karar 1 neydi:** probe Noise ALTINDA + akış-bağlılık (B-opt1) + `hub_noise_x25519_pub`'dan türetilmiş
keyed-token (proof-of-knowledge, ~16B) + silent-responder (geçersiz tag'e sessizlik) + amp≤1 + rate-limit.
Gerekçe: aktif-tarama (GFW'nin Shadowsocks'u aktif probing'le yakalaması; obfs4/V2Ray cevabı = proof-of-
knowledge'sız sessizlik) + on-path 5-tuple-spoof enjeksiyonu token'la kapatılır. Anahtar zaten dağıtımda
(KAT-4 sonrası her client `hub_noise_x25519_pub`'ı bilir). Metafor: parolalı zil.

**Neden SUPERSEDED:** review 4 ek problem gösterdi ki ikisi KAT-donmuş invariant ihlaliydi:
- **R12 (KAT-blokör):** engine §1 taksonomisi unauthenticated transport-emitted class TANIMLAMAZ;
  INV-CRYPTO-1/TDD §0.1 KİLİTLİ outer-AEAD'i zorunlu kılar. Below-Noise probe bu donmuş invariant'ı
  **değiştirmeyi gerektirir** (ayrı ağır KAT-amendment) — yasak.
- **R7 + §2a-madde-1 iç çelişki:** below-Noise probe flag'i (0x50/0x51) Kama'nın **tek cleartext
  sabit-ilk-byte** paketiydi → §2a-madde-1 ("plaintext magic yok") kendi içinde çelişirdi.
- **R5 (echo-forge):** keyed-token **responder'ı** korur; echo token yansıtmadığından (Karar 4)
  **sender** on-path echo-forge'a açık kalırdı.

**Keystone bunları nasıl eritti:** probe Noise-içi (authenticated CTRL frame) → AEAD hem proof-of-
knowledge'ı (token gereksiz) hem anti-forge'u (nonce gereksiz) hem taksonomi-uyumu hem cleartext-magic
yokluğunu **tek hamlede** sağlar. Karar 1'in DAVRANIŞSAL gereksinimleri (silent-drop, amp≤1, rate-limit)
mekanizmadan bağımsız korundu (§2.3b) — yalnız uygulama katmanı (token → AEAD tag) değişti.

**[K-final] Reddedilen alternatifler (kayıt):** **B-opt3** = probe'u tamamen ertele (dar kanalda öğrenme
yavaşlar — reddedildi) · **B-opt2** = DATA-ayırt-edilemez şekillendirme (Beta+'a taşındı, §8 / §3.D-madde-3).

*revize-D0 sonu. Onay → D1 (spec patch, birebir bu nottan + §4 onaylı byte-layout'lar). Onaysız D1/D2 BAŞLAMAZ.*
