# Kama — Adaptive Transport Engine Spec (Implementable)

> **YAMA KAYDI — 2026-10-24 [v1.1 · K4G-05b — RTT örnek kabulü + kopya kuyruğu sırası + BURST_CAP tur başı tabanı + sınıflar arası kopya payı + §16.2 saat-sınıfı satırı; sahip kararları CEVAP PAKETİ 5, 2026-10-16; CEVAP PAKETİ 7, 2026-10-18 (DUR-3 (b), DUR-9 (a)); CEVAP PAKETİ 7-EK, 2026-10-18 (B1); CEVAP PAKETİ 9, 2026-10-24 (RTT kabulü, BURST_CAP, sınıflar arası pay; DUR-1 (a) srtt-yok istisnası kaldırıldı); CEVAP PAKETİ 10, 11, 11-EK, 12 ve 13, 2026-10-24 (sınıflar arası payın kapsamı ve rezervasyonu; K_MAP adet sınırı, budama K4G-05c); CEVAP PAKETİ 15 v3 ve 15 EK-1, 2026-10-25 (DUR-38: B′, K1–K3; DUR-40) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §6 Girdi satırına **v1.1 KABUL KURALI** eklendi (çerçevedeki sender seq'lerinin en büyüğü bu çerçeveyle ilk kez onaylanıyorsa yalnız onun kaydı, aksi hâlde örnek yok · köken Data, Retransmit ya da Duplicate, zaman o iletimin gönderim anı; FEC onarım sembolü örnek vermez · doygun olmayan ack_delay · tutma süresiyle şişmemiş örnek; geri düşüş yok; srtt yokken de aynı kural) ve **tazelik ölçütü** (yalnız gözlem: `rtt_stale` sayacı); §6 min-rtt guard'ının RAW kolu ve "örnek ATLANMAZ" hükmü bu kurala bağlandı; §5 TUR-2a/TUR-2b kol-1 ve TUR-2c kol-2 RTT'si bu kurala bağlandı (superseded işareti RTT'yi dışlamaz; BW-örneği ve delivered-kredi DEĞİŞMEDİ; birim başına kayıt budamasıyla (K4G-05c) düşen kaydın geç-ACK'i Expired/Unknown); §5'te kol-2 "RTT ALINMAZ", ZEYL'deki "(superseded iletimin RTT'si güven-dışı)" ve §6 Girdi satırındaki "her acked paket için" üstü çizildi; §13'e kopya kuyruğu içi sıra (en eski onaysız birim önce), BURST_CAP tur başı tabanı (DATA — kuyruk boşsa DEFAULT_MTU, v1 —, ARQ planı, kuyruk-probu hedefi ve kopya sırası başı; ilk kırpmadan önce; token üretilmez — PRE_CONVERGE'de v1, beyanlı), RTO_MAX tavanı ve sınıflar arası sıranın ~~TEK istisnası (PRE_CONVERGE hariç bütün durumlarda vadesi gelmiş kopya kuyruğu başı en geç her K_ARQ_SHARE'inci turda DATA'dan önce hizmet hakkı kazanır; token bulamazsa rezerv — başın kopyası dışındaki hiçbir token tüketicisi rezervin altına inemez)~~ istisna ve karşılık kuralı (PRE_CONVERGE hariç bütün durumlarda: karşılık hakkı, sahipli tek rezerv — kopya, ARQ ya da DATA başı —, ARQ payı rezervi, yaş tavanlı HOL, (iii) HİZMET GARANTİSİ — engine §13) eklendi; §13 ekleme kuralındaki öne çekmeye sıklık atfı eklendi (zincir dönemi başına en çok bir kez — transport-layer-tdd §12 [AMENDMENT-12]; DUR-41 (B), sahip kararı bekliyor — tasarım notu Ek B.1d); §16.2 saat-sınıfı tablosuna TEK satır eklendi (kopya kuyruğu aralık tavanı + sahipsizlik süresi + parite kaydı süre dolumu + RTT tazelik ölçütü). Otoriter v1.1 kaydı: transport-layer-tdd §12 [AMENDMENT-12]. Kuantum döngüsü, tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-24 [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** Yama yetkisi (§5/§6/§13 + §16.2'de tek satır) dışında kalıp v1.1'de bayatlayan şu ifadeler DEĞİŞTİRİLMEDİ; v1.1 için otorite transport-layer-tdd §12 [AMENDMENT-12]: §2.1 `ack_delay_saturated` "beklenen 0" (en büyük seq'in alımından 16.78 s sonra gönderilen ACK doyar; v1.1'de doygun ack_delay'li örnek §6 KABUL KURALI (d) ile alınmaz) · §5.1 sessizlik eşiği `ACK_SILENCE_MULT × srtt` (v1 kodu: max(3·srtt, K_CONF_EAI·EAI); sahipsizlik süresi H bu eşikten türer) · §9.1 ARQ_RETX_CAP "veya connection-fail" (v1.1: hiçbir politikada düşürme yok, birim kopya kuyruğuna devredilir) · §12 "PTO backoff exponential (2^n)" (üst sınır RTO_MAX = 30 s) · §16.2 "geometrik kopya deadline'sızlığı" satırı (PRE_CONVERGE dışı aralık tavanı hemen altındaki v1.1 satırında) ve K-1 KAPANIŞ DURUMU'ndaki kol-1 RTT'si (§6 KABUL KURALI'na bağlı) · EK4-A2 uyanma envanteri (`reports/ek4-a2/EK4-A2-NEXT-WAKEUP-CONTRACT.md`; sha256-pinli, dokunulmadı): v1.1'de 7. kaynak = sahipsizlik vadesi; 3. kaynak (kopya kuyruğu) drenajla aynı sıra başını raporlar, aralığı PRE_CONVERGE dışında RTO_MAX'la sınırlı. Düzeltmeleri "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.

> **YAMA KAYDI — 2026-10-04b [K3 — D2 / Tema E KAPANDI, Abkarada kararı]:** §2'ye not: **ürün ACK'i = engine `ACK_FRAME_V2` (0x14)**; WS-D1'de `meta.full_wire_pn`'e anahtarlanır (her yeniden gönderim yeni PN; tablo PN → mantıksal birim). noise-wire `Control::Ack/Sack` (0x10/0x11) ailesi SİLİNDİ (noise-wire §2.56/§2.57). Bu belgedeki çerçeve yerleşimi DEĞİŞMEDİ. SPEC-ONLY.

> **YAMA KAYDI — 2026-10-04 [DR-CONNECT-1 H4 — sahip kararı]:** §1.1 transport-control `frame_type` registry'sine iki codepoint açıldı: **`0x1B HEARTBEAT`**, **`0x1C HEARTBEAT_ACK`** (hop heartbeat; HubChannel ctx; otoriter çerçeve + kurallar transport-layer-tdd §10 [AMENDMENT-10]). Çakışma tarandı: §1.1'de boştu; noise-wire §2.56 E2E registry'sinde reserved(reject) kalır. Mevcut codepoint'lerin anlamı/yerleşimi DEĞİŞMEDİ. SPEC-ONLY.

> **YAMA KAYDI — 2026-10-03 [DR-CONNECT-1 H5]:** Üç emisyon yolundaki (§3.3-B, §11, §11.1) AMENDMENT-9 bağ notlarında `K` değeri yazıldı: **K = 1024** (KS-PN-1 KAPANDI; otoriter metin transport-layer-tdd §12 [AMENDMENT-9]). Tavan hâlâ SPEC-ONLY; kod değişmedi. Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-09-23 [D-GAP-2 bağı]:** Sayı-tavanı olmayan üç emisyon yolu (§3.3-B geometrik kopya, §11 emergency forward redundancy, §11.1 cold-start probe/echo) transport-layer-tdd [AMENDMENT-9] "hop-ACK almadan ardışık PN ≤ K" tavanına bağlandı (K: KS-PN-1 [KALİBRASYON]). SPEC-ONLY — kod değişmedi; uygulandığında EVOLUTION §22'nin altı beyanı geçerlidir (kimlik `transport-v1.x`).


> **YAMA KAYDI — 2026-09-18b:** §10.1 shard alan etiketleri `[GÜVENLİK katmanı]` → `[SEAM — session E2E anahtarı]` (AMENDMENT-5 ile tutarlı; RS transport'ta); §13 otorite tablosu "shard AEAD'i GÜVENLİK katmanı yürütür" → seam/hop-ctx diliyle. §0.2/§10 arası çelişki kapandı.


> **YAMA KAYDI — 2026-09-18 [AMENDMENT-5, dar; Claude Code metni]:** §0.2 :47 ve :712 — FEC POLİTİKA + EXECUTION (RS encode/reconstruct) TRANSPORT'un (runtime.rs:1408/1501/2061); per-shard AEAD SEAM'in (seal :998 / open :1184); open → reconstruct sırası (INV-FEC-1) korunur. Eski "EXECUTION güvenlik katmanının" hiç uygulanmamıştı. Başka değişiklik YOK; "E2E transport'un üstünde" topolojisi YAZILMADI (iptal).


**Belge:** `adaptive-transport-engine-spec.md` v0.1 — Alpha
**Statü:** IMPLEMENTATION SPEC (kod blueprint'i; `transport-layer-tdd.md` mimari/policy otoritesinin altında)
**Bağlam:** `transport-layer-tdd.md` (Architecture & Alpha Policy TDD) *ne* inşa edileceğini dondurdu.
Bu belge *nasıl* — state machine → struct, ACK frame → wire layout, estimator → fonksiyon.
Rust-native (ADR-01), Tokio (ADR-02), reed-solomon-erasure (ADR-16), deterministik fuzzy (ADR-17).

> **Otorite sırası:** `transport-layer-tdd.md` (WHAT/policy) > bu belge (HOW/engine). Çelişki olursa TDD kazanır; bu belge düzeltilir.
> **Sabit disiplini (NORMATİF):** Tunable sabitler `[CAL]` = netem baseline'dan (ADR-18) kalibre edilir, UYDURULMAZ. `[STD:kaynak]` = literatürden alınmış principled başlangıç (ör. BBR default). İkisi de rapora ÖLÇÜLMÜŞ/kaynaklı yazılır (§4.4.0 magic-number yasağı).

---

## 0. Kapsam, Konum ve Rate-Control Ailesi (NORMATİF)

### 0.1 Reliability topolojisi — HOP-BY-HOP

```
Alice ──[reliable transport hop 1]──> Hub ──[reliable transport hop 2]──> Bob
                                       └─ mailbox (store-and-forward, redb; ADR-13)
```
- Transport reliability **hop-scoped**: Alice↔Hub ve Hub↔Bob ayrı reliable hop'lar. Hub, envelope'u mailbox'a yazar → forward.
- E2E kripto (L3 Session Model V3 envelope) **uçtan uca**; transport hop'ları envelope'u OPAK taşır.
- **ACK anlamı:** Alice'in transport ACK'i = "Hub envelope'u aldı (mailbox'a yazıldı)", "Bob okudu" DEĞİL. Bob-teslim/okundu app-katmanı E2E receipt (ayrı, bu belgenin kapsamı dışı).
- Rate controller HER hop'ta ayrı çalışır (endpoint bir sonraki hop'a kendi ölçtüğü bw/loss/rtt ile pace eder).

### 0.2 Transport ⟂ Kripto Ayrımı (NORMATİF — SoC, çekirdek ilke)

Transport engine paket TÜRÜYLE ve KRİPTOYLA İLGİLENMEZ. Görevi tek: opak, **güvenliği zaten sağlanmış** birimleri hedefe ulaştırmak. **AEAD TUTMAZ, anahtar YÖNETMEZ, şifrelemez / çözmez / verify etmez.**

```
GÜVENLİK (ayrı katman)  : Noise handshake + kompakt session design (Session Model V3; TDD §0.1/§3, ccr §35).
                          TÜM AEAD burada yürütülür: inner E2E envelope + control/shard outer-AEAD +
                          code-then-encrypt-authenticate-shards (INV-FEC-1).
TRANSPORT (bu belge)    : opak birimleri PACE / DELIVER / TRACK eder; reliability için METADATA okur
                          (seq / block_id / shard_index — güvenlik katmanı bunları AAD olarak authenticate eder);
                          loss / bw / rtt / jitter ÖLÇER; FEC/ARQ/rate POLİTİKASINI belirler.
```

Sonuçlar (NORMATİF):
```
- seq: transport reliability için OKUR; ama seq güvenlik katmanının AAD'sidir → tampering AEAD verify'ı kırar
       → güvenlik katmanı drop eder. Transport seq'i optimistik kullanır, DOĞRULAMAZ.
- ACK: İÇERİĞİ transport üretir (hangi seq'ler geldi) ama client↔hub Noise-secured kanalın İÇİNDE taşınır
       → forge-koruması Noise'dan gelir, transport'tan DEĞİL. Transport ACK'e AEAD UYGULAMAZ; güvenlik katmanı uygular.
- FEC: POLİTİKA (ne kadar parity, §8.4 fuzzy — ölçülen loss'tan) transport'un;
       [AMENDMENT-5] (impl-driven, kod-spec tutarlılığı): FEC POLİTİKA (k, r; §8.4) ve EXECUTION (RS encode / reconstruct)
       TRANSPORT'undur (client-core RsSimdCodec.encode runtime.rs:1408/:1501, .reconstruct :2061). Per-shard AEAD SEAM'indir
       (SecuritySeam::seal :998 / open :1184, tel kenarı). Sıra korunur: gönderici RS-encode → seal, alıcı open → reconstruct
       (authenticate-before-decode, INV-FEC-1). Transport shard başlığını (block_id/esi/k_actual) reliability için okur,
       anahtar tutmaz; seam kurtarma bilmez. ~~Eski: EXECUTION güvenlik katmanının; güvenlik katmanı encode+AEAD yapar~~ (hiç uygulanmadı).
- "Pipeline D outer-AEAD" (TDD §0.1) control/shard için EXECUTE edilir ama GÜVENLİK katmanında;
       bu belge o AEAD'i transport-owned olarak REDEFINE ETMEZ (yalnız birimi opak taşır).
```

> **INV-ENGINE-SOC:** Transport engine hiçbir AEAD/anahtar tutmaz; kripto = Noise + kompakt (ayrı katman). Transport **type-agnostic + crypto-agnostic**: yalnız opak birim iletir + metadata okur + ölçer + policy verir. Güvenlik birimi zaten güvenli teslim eder; transport "ne olursa olsun ulaştırmaya" çalışır.

### 0.3 Rate-Control ailesi — SRC (Kama Rate Control), model-based

```
SRC = BBR-türevi delivery-rate model çekirdek + FEC/ARQ füzyonu + forward-recovery.
  ÜÇ ORTOGONAL SİNYAL (asla birbirine indirgenmez):
    bandwidth  := BtlBw  = max-filter(delivered_rate)     [ÖLÇÜLÜR; loss'tan TÜRETİLMEZ]
    congestion := queue_delay = srtt − RTprop             [delay-based sinyal; loss DEĞİL]
    loss       := loss_pre_fec / residual_loss (sayaç)    [recovery kolunu besler; RATE'İ DÜŞÜRMEZ]
```
- **INV-RATE-1 (TDD):** loss send_rate'i AZALTMAZ. SRC yalnız `queue_delay` (delay) + `inflight_cap` ile pace eder.
  **[TUR-7 S1-onaylı EK-CÜMLE — kör-nokta kapanışı]:** FAZ-GAIN'İ de loss'tan bağımsızdır:
  faz yalnız KANAL-ÖLÇÜMÜ olaylarıyla değişir (plato/BDP-eşiği/probe-saatleri); recovery-durumları
  pacing-fazı DEĞİL, gönderim-seçimi OVERLAY'idir (kayıp-ilanı faz-gain'ini değiştiremez —
  kilit-golden: `inv_rate_1_loss_declaration_preserves_phase_gain`; cift-kilit-v1 §0-ÖN/S1).
- BBR'ın ölçüm çekirdeği ALINIR; BBR ProbeBW/ProbeRTT gain-cycling state machine ALINMAZ (stabil-path varsayar) → §8 gentle-probe.
- BBR'da olmayan iki kol EKLENİR: (a) aynı state'ten FEC/ARQ arbitrasyonu (§8.4), (b) ack_confidence LOW'da forward-recovery (§11).

---

## 1. Packet Classes (NORMATİF)

Wire'daki her datagram tek bir class'a aittir. Class, ilk byte'tan (ya da hub_handle demux'undan sonra frame_type'tan) ayrışır.

```
CLASS               taşır                                       güvenlik (AYRI katman*)  ref
──────────────────────────────────────────────────────────────────────────────────────────
SESSION_INIT        zero-RTT E2E bootstrap (ss_e+ss_s)          inner E2E                §3,§1
SESSION_INIT_RESP   responder ephemeral (epoch-0→1 FS-upgrade)  inner E2E                §1
SESSION_DATA        hot-path: hub_handle+seq+ct+tag (22B+pl)    inner E2E tag            §2
SIGNED_CONTROL      SESSION_DATA + Ed25519 64B (non-repud.)     inner + Ed25519          §1
─── transport-control (client↔hub hop; güvenlik katmanı outer-AEAD uygular, §0.2) ──────────
ACK_FRAME           SACK feedback (§2)                          hop outer-AEAD*          §12 TDD
CTRL_FRAME          MTU probe / path-probe / rekey-signal       hop outer-AEAD*          §11 TDD
FEC_SHARD           block data/parity shard                     per-shard AEAD*          §7 TDD, INV-FEC-1
```
> **\* Tüm güvenlik (AEAD) NOISE+KOMPAKT katmanda yürütülür (§0.2); transport bu birimleri OPAK görür — tuttuğu/uyguladığı AEAD YOK.** Bu tablo "güvenlik nereden geliyor"u gösterir, transport'un ne yaptığını değil.

- SESSION_DATA hot-path'e ACK/FEC header, transport-prefix, imza EKLENMEZ (TDD §2 INV-WIRE). ACK ayrı CLASS.
- FEC_SHARD yalnız block/shard framing MEVCUTSA (segment-block pipeline); tek-datagram'da FEC yok (TDD §7).
- Transport için hepsi opak "birim"dir; transport yalnız seq/block_id/shard_index metadata'sını okur (§0.2).

### 1.1 Wire class tag (hub_handle sonrası)

```
transport-control frame ilk byte = frame_type:
  0x10 ACK_FRAME
  0x11 CTRL_MTU_PROBE
  0x12 CTRL_PATH_PROBE          # CANCELLED — conflicts with authoritative §2.56 registry: 0x12 is RECOVERED.
                                # Superseded by DR-CS-1: 0x19 CTRL_CS_PROBE / 0x1A CTRL_CS_ECHO
                                # (kama-noise-wire-tdd-v2.0 §2.56/§2.57 otoritatif; bu-satır arşiv-notu, sessiz-silme-yok)
  0x13 CTRL_REKEY_SIGNAL
  0x14 ACK_FRAME_V2             # [K3/R2] cold-start ölçüm eki (§2.2); V1 0x10 korunur
  0x1B HEARTBEAT                # [DR-CONNECT-1 H4, 2026-10-04] hop heartbeat = prob = NAT keepalive; HubChannel ctx;
                                # body 0 B veya echo_token u32_be; kendi sayacı yok (transport-layer-tdd §10 [AMENDMENT-10])
  0x1C HEARTBEAT_ACK            # [DR-CONNECT-1 H4] yanıt; body isteğinkiyle aynı uzunlukta, token aynen yansıtılır
                                # 0x1B/0x1C E2E inner_kind DEĞİLDİR: noise-wire §2.56'da reserved(reject) kalır
  0x20 FEC_SHARD_HEADER
(SESSION_* class'ları hub_handle demux + E2E envelope tipinden ayrışır; transport-control frame_type güvenlik katmanı outer-AEAD-çözümünden SONRA görünür — çözümü GÜVENLİK katmanı yapar, §0.2.)
# [K-final] Cold-start probe/echo (§11.1) dış çerçevesi = hub_handle(4B)+ciphertext+tag → CTRL sınıfı-İÇİ ÖZDEŞ
# (frame_type/subtype AEAD-içi → tür-ele-verme YOK). SESSION_DATA'dan seq16(2B)+inner-E2E ile ayrışır →
# sınıf-ele-verme (TÜR değil); §10.1 "frame_type sızar" gerilimiyle tutarlı, tam indistinguishability Beta+.
```

---

## 2. ACK / SACK Frame Format (NORMATİF — fixed-width, ccr §35 varint YASAK)

> **[K3 — D2 / Tema E KAPANDI, 2026-10-04 · Abkarada]** **Ürün ACK'i bu bölümün ailesidir: engine `ACK_FRAME_V2`
> (0x14; §2.2).** Tek doğruluk kaynağı budur. WS-D1'de ACK'in taşıdığı kimlik `meta.full_wire_pn`'e anahtarlanır
> (KM-SEAM-TC-001 K2): her yeniden gönderim YENİ PN alır; gönderen tablosu PN → mantıksal birim eşler. noise-wire'ın
> E2E `Control::Ack/Sack` (0x10/0x11) ailesi SİLİNMİŞTİR (noise-wire §2.56/§2.57): E2E ARQ yoktur — kurtarma
> hop-yereldir (bu motor) + mailbox fallback (app-rpc §6.11); E2E teslim bilgisi AMP `RECEIPT`'tedir. Hop-yerel ARQ'nun
> kontrol döngüsü yüksek jitter'da E2E'den kısadır. Aşağıdaki "seq u16" ifadesi bugünkü kodu anlatır; D1 ile PN'e geçer.

Kama `seq` = transport hop seq, **u16** (TDD §2 SESSION_DATA seq 2B). ACK bu seq uzayında çalışır.

```
ACK_FRAME {                     # içeriği transport üretir; client↔hub Noise-secured kanalda taşınır (güv. katmanı, §0.2)
  frame_type    : u8            # 0x10
  ack_epoch     : u8            # rekey/counter-wrap epoch (largest_seq bu epoch'ta)
  largest_seq   : u16_be        # alınan en yüksek seq (bu epoch)
  ack_delay     : u16_be        # µs; largest_seq alımı → bu ACK gönderimi (RTT düzeltmesi, §6)
                                # [TUR-1x] V1 EMIT EDİLMEZ (yalnız decode-uyumluluk): µs u16 tavanı
                                # 65.5ms sub-kbps ACK-hold'unda anında doyar → §6 düzeltmesi çalışamaz
                                # (§16.2 satırı). EMIT = V2 (§2.2, 256µs).
  block0_len    : u16_be        # en üstteki ardışık-alınan blok uzunluğu (largest dahil)
  range_count   : u8            # EK gap+blok çifti sayısı (0 = yalnız cumulative/block0)
  ranges[range_count] {
    gap         : u16_be        # bir önceki bloğun altındaki KAYIP seq sayısı
    len         : u16_be        # ardışık-alınan blok uzunluğu
  }
}
# Boyut: 9B sabit + 4B × range_count. Varint YOK (fixed-width big-endian, ccr §35).
```

**Anlam (descending — QUIC ACK yapısına benzer, fixed-width):**
```
Block 0 (acked): [largest_seq − block0_len + 1 .. largest_seq]
i≥1 için:
  gap_i kadar seq KAYIP (henüz alınmadı)
  Block i (acked): gap+len ile bir alt bloğu tanımlar
Rekonstrüksiyon:
  cursor = largest_seq − block0_len + 1
  her range: cursor -= gap_i;  block_i = [cursor − len_i + 1 .. cursor];  cursor -= len_i
```

**seq-wrap (u16) rekonstrüksiyonu (NORMATİF):**
```
Wire seq 16-bit; full_counter 64-bit sender'da monoton. Rekey wrap'ten ÖNCE (TDD: counter wrap→rekey);
ack_epoch rekey sınırını disambiguate eder. Decoder full seq'i "expected next"e en yakın aday ile çözer:
  candidate = (expected_high_bits << 16) | wire_seq
  |candidate − expected| minimize edecek high-bits seçilir (QUIC packet-number decode analog).
Farklı ack_epoch → farklı key domain; cross-epoch ACK reddedilir.
```

### 2.1 ACK üretim kuralları (receiver)
```
- ACK-tetik: max(ACK_FREQ_N alınan DATA, ACK_TIMER_T) hangisi önce (§12).   [CAL]
- range_count ≤ MAX_ACK_RANGES; taşarsa en yeni (en yüksek seq) bloklar öncelik.   [CAL: MAX_ACK_RANGES ~16]
- ack_delay = now − largest_seq_recv_time. [TUR-1x] V2 encoding (NORMATİF, D-4/1 Seçenek-A):
  `encoded = min(floor(delay_us/256), 65535)` (FLOOR — receiver kendi delay'ini ŞİŞİRMEZ, decoded ≤
  gerçek → sender path-RTT'si fazla küçülmez); `encoded==65535` → satürasyon audit-sayacı
  (`ack_delay_saturated`, beklenen 0: tavan 16.78s > ACK_CADENCE_FLOOR 8.39s).
- Reverse-path impaired (ack_confidence LOW) → ACK decimation (§11): ACK_FREQ artar, ACK-only budget düşer.
```

### 2.2 Extended SACK — ACK_FRAME_V2 (NORMATİF — cold-start ölçüm eki) [K3/R2/revize-D0 §4.1]

> V1 (0x10) KORUNUR. V2 geriye-uyumlu — yeni frame_type **0x14** (0x11 DEĞİL: CTRL_MTU_PROBE ile
> çakışırdı, R2). V2 ölçüm eki (`seen_count`+`arrival_rate`) cold-start'ta yön-ayrımlı loss + ilk bw ipucu verir.

```
ACK_FRAME_V2 {
  frame_type    : u8       # 0x14   (§1.1 tabloda 0x14; 0x14-0x1F boştu, çakışma YOK)
  ack_epoch     : u8
  largest_seq   : u16_be
  ack_delay     : u16_be   # [TUR-1x, D-4/1 Seçenek-A] birim 256µs (tavan 65535×256µs ≈ 16.78s >
                           #   cadence-floor 8.39s). Floor-encoding + satürasyon-sayacı §2.1'de.
                           #   µs-birim YALNIZ V1'dedir (V1 emit edilmez, decode-uyumluluk).
  block0_len    : u16_be
  range_count   : u8       # ≤ MAX_ACK_RANGES (16)
  ranges[range_count] { gap:u16_be; len:u16_be }
  # ── V2 ölçüm eki ──
  seen_count    : u32_be   # görülen toplam DATA (birim+kopya) → sender kendi gönderdiğiyle kıyaslar:
                           #   seen≪sent = forward-loss; seen≈sent ama SACK gelmiyor = return-loss (p5 ayrımı)
  arrival_rate  : u16_be   # receiver-gözlemli pencere-oranı B/s (ilk bw örneği). Sentinel 0xFFFF =
                           #   "ölçüm YOK ya da üst-bantta — cold-start ipucu olarak KULLANMA".
                           #   [TUR-1x zeyli] Mevcut artımda receiver datagram BOYUTU görmez → daima
                           #   sentinel emit edilir; gerçek pencere-oranı K3 tüketicisiyle bağlanır.
}
# 15B sabit + 4B×range_count. Noise üstü, authenticated (§0.2 outer-AEAD güvenlik katmanı).
# arrival_rate pencere-oranı (packet-pair DEĞİL — cold-start'ta gürültüye dayanıklı yeterli).
```

---

## 3. Sender State Machine (NORMATİF)

```
Durumlar:
  IDLE            → gönderilecek yok; timer'lar pasif
  PRE_CONVERGE    → cold-start: motor HİÇ ölçüm yokken; en-kötü-köşe önsel pacing + geometrik kopya (§3.3-B)   [K-KEYSTONE]
  STARTUP         → BtlBw hızlı keşif (pacing_gain yüksek), path doldur
  STEADY          → pace = BtlBw; periyodik BW/RTprop probe
  RECOVER_ARQ     → residual loss var + ack_confidence OK → selective retransmit (§9)
  RECOVER_FWD     → ack_confidence LOW → forward redundancy (§11), ACK-bağımsız
  DRAIN_RTPROP    → RTprop probe: inflight düşür, min-RTT ölç (§8.3)
  DRAIN           → STARTUP-çıkış overfill-tahliyesi (BBR-analog): düşük-gain pace, inflight
                    cwnd_gain×BDP'ye inene dek (Startup-kuyruk Steady'ye taşınmaz)               [TUR-3b]
  PROBE_UP        → periyodik +gain bant-probu: app-limited-kilidi kırar (btlbw delivered-rate'ten
                    ölçülür → rate=btlbw kendi-tavanını hiç aşamaz; probe tavanı test eder)      [TUR-3c]

Geçişler (event-driven, deterministik):
  IDLE   --data-hazır-->                 PRE_CONVERGE                                          [K-KEYSTONE §3.3]
  PRE_CONVERGE --ilk geçerli SACK/echo (RTT+loss kilidi §3.3-D)--> STARTUP                     [R1/R14]
  STARTUP--BtlBw plato (Δ<ε, N round)--> DRAIN --inflight≤cwnd_gain×BDP ∨ dwell≥3×ack_epoch--> STEADY  [TUR-3b]
  STEADY --periyot(4×ack_epoch) ∧ qd≤500‰ ∧ backlog--> PROBE_UP --success|abort|timeout|preempt--> STEADY  [TUR-3c]
  ACTIVE --ack_confidence LOW----------->  RECOVER_FWD  --ackconf≥MED--> STEADY   [AMENDMENT-1]
  ACTIVE --residual_loss>0 ∧ ackconf≥MED-> RECOVER_ARQ  --temiz-------> STEADY   [AMENDMENT-1]
  STEADY --rtprop_probe_due------------->  DRAIN_RTPROP --min-rtt ölçüldü--> STEADY
  (any)  --idle > IDLE_TIMEOUT---------->  IDLE
  ── ACTIVE = {STARTUP,STEADY,RECOVER_ARQ,RECOVER_FWD,DRAIN_RTPROP,DRAIN,PROBE_UP} (IDLE hariç tüm aktif fazlar).
     PRE_CONVERGE ACTIVE'e DAHİL DEĞİL: recovery (FWD/ARQ) + emergency + loss-ilanı §3.3-C'de DONDURULUR.
     Değerlendirme önceliği (her event): IDLE(iş yok) > PRE_CONVERGE(ölçüm-yok §3.3) > RECOVER_FWD(ackconf
     LOW) > RECOVER_ARQ(pending loss) > faz-içi(STARTUP plato / DRAIN / STEADY / DRAIN_RTPROP / PROBE_UP).
     PROBE_UP'ı emergency/pending kesmesi = PREEMPT çıkışı (4. tür, AYRI-raporlu muhasebe [TUR-3c]).
```

> **[AMENDMENT-1] (impl-driven, gerekçeli — kod-spec tutarlılığı):** Recovery (FWD/ARQ) yalnız STEADY'den
> DEĞİL, her ACTIVE fazdan erişilir. **Gerekçe:** ACK-yolu çökerse (p5_revpath baseline — quinn c(10s)=0.00)
> STARTUP BtlBw platosuna ULAŞAMAZ (ACK yok → btlbw büyümez) → STEADY erişilemez → RECOVER_FWD KİLİTLENİRDİ;
> ayrıca loss STARTUP sırasında olabilir (yaygın) → retransmit tıkanmamalı. forward-recovery zaten
> ACK-BAĞIMSIZ (§11) — faza bağlı olmaması, recovery-obsesif taban ("veri ULAŞSIN") ile tutarlı. Öncelik
> RECOVER_FWD > RECOVER_ARQ: ackconf LOW iken ARQ futile (ACK dönmüyor), forward-recovery devralır.

**Sender ana döngü (her event):**
```
on_event(e):
  match e:
    PacketSent(pkt)  -> §3.1 OnPacketSent
    AckReceived(ack) -> §3.2 OnAckReceived
    TimerFired(t)    -> §12 timer handler
    AppData(bytes)   -> enqueue(tx_queue); if state==IDLE: state=PRE_CONVERGE   # §3.3 [K-KEYSTONE]; PRE_CONVERGE→STARTUP ilk geçerli SACK/echo'da (§3.3-D)
  ClassifyPath()        # §5
  (fec_ratio, arq_aggr, send_rate, compress) = FuzzyEvaluate()   # §8.4, §15 — compress: KALINTI çıktı, tüketilmez [AMENDMENT-4]
  ClampRecoveryBudget()  # §8.5
  UpdatePacingRate(send_rate)  # §13
  ScheduleTransmit()     # pacing token bucket'a göre DATA / retransmit / FEC / duplication
```

### 3.1 OnPacketSent(pkt)
```
pkt.send_time = now
pkt.seq       = next_seq++        # u16 wire, full_counter 64-bit
pkt.size      = wire_bytes
inflight_bytes += pkt.size
append(sent_packets, pkt)          # RTT/loss/delivered muhasebesi için
mark_app_limited_if(tx_queue boş ∧ inflight < cwnd)   # §7.3 app-limited işareti
arm_pto_timer()                    # §12 — PRE_CONVERGE'de ATLANIR (§3.3-C/5: srtt BOŞ → PTO tanımsız; geometrik kopya §3.3-B yerini alır)
```

### 3.2 OnAckReceived(ack)
```
newly_acked = resolve_acked_ranges(ack, sent_packets)   # §2 rekonstrüksiyon
for p in newly_acked:
    inflight_bytes -= p.size
UpdateRTT(ack, newly_acked)             # §6
UpdateDeliveredRate(newly_acked, ack)   # §7
DetectLoss(ack)                         # §5 (loss = SİNYAL DEĞİL rate için; recovery için)
free(acked packets)
reset_pto_timer()
```

---

### 3.3 PRE_CONVERGE — Cold-Start (NORMATİF)

> **[K-KEYSTONE / revize-D0 §2.1-2.4]** Motor HİÇ ölçüm yokken (cold-start) bu durumdadır.
> İlke: **KAPI YOK (ama FREN VAR), TERMOMETRE VAR** — veri t=0'da çıkar; ölçüm gönderilenin yanında
> toplanır. Eski "sabit INITIAL_RTT-seed + sabit-timeout" cold-start modeli CANCELLED (Ek-B).

**Giriş/çıkış:**
```
IDLE --AppData--> PRE_CONVERGE            # t=0, ölçüm yok; ilk birim beklemeden gider
PRE_CONVERGE --ilk geçerli SACK/echo--> STARTUP   # RTT+loss kilidi açılır (§3.3-D; echo de açar — §11.1(iv))
```

**(A) Davranış önseli — fuzzy en-kötü-köşe (NORMATİF cümle):** [K5/§2.1]
> *"PRE_CONVERGE'de motor fuzzy en-kötü-köşe önseliyle DAVRANIR; bu önsel hiçbir estimator'a örnek
> olarak yazılmaz, ilk geçerli ölçümle koşulsuz tahliye edilir ve prob takvimi önselin geçerliliğinden
> bağımsız sürer."*
- `bw = PRE_CONVERGE_FLOOR_BW` (§16 [CAL]; operatör `bandwidth_class` yalnız ÖNSELİ ezer, ölçümü ASLA),
  `loss = extrem`. Pacing bu tabana kurulur → zarf-içi (≥ taban) hiçbir kanalda self-congestion inşa
  gereği imkânsız. Estimator'lar (BtlBw/srtt/loss) cold-start'ta BOŞ.
- **[R8] Pacing floor:** PRE_CONVERGE'de send_rate clamp floor'u = `PRE_CONVERGE_FLOOR_BW`; `min_rate_bps`
  fixture (=10000 B/s=80 kbps) **MUST-NOT-APPLY** (Ek-B). Yoksa 1 kbps kanal 64× floodlanır (self-congestion).
  [K-D2] **BURST clamp da ZORUNLU** (yalnız rate DEĞİL): token-bucket DEFAULT_BURST_CAP (2×MTU) t0'da tüm
  kuyruğu tek hamlede wire'a çıkarır → floor'u baypas eder (self-congestion + §3.3-B kopyalar temiz kanalda
  spurious ateşler; burst tüm wire-emit'leri t0'a çökertir → [K-D2] copy-clock spread'i etkisiz kalır).
  PRE_CONVERGE'de burst tek-birime clamp'lenir → ilk birim t=0'da gider, kalanı floor-refill yayar.
- **[R1/R2/R3-ADIM6] Pacer kbit-domain genellemesi (NORMATİF — AŞAMA-1 denetim bulgusu):** Pacer aritmetiği
  80 kbps altına inemiyordu (dar-pencere ADIM-6 D2'nin 10-msg burst'ü maskeledi; 100-msg sub-kbps açığa
  çıkardı). ÜÇ kök-fix:
  - **[R1] Token refill SUB-BYTE precision:** `rate·dt_ns/1e9` per-tick tamsayı-truncation'ı rate<1000 B/s'te
    (31 B/s @ 1ms = 0.031→0) her tick 0 üretip pacer'ı KİLİTLİYORDU (PreConverge no-ack STALL kök nedeni: 1
    birim gönderip durur). Fix: kesir-kalan sonraki refill'e taşınır → floor gerçekten token üretir.
  - **[R2] min_rate kanal-türevli:** MUTLAK 80 kbps min_rate SADECE PreConverge'de kapatılmıştı; Startup'a
    çıkınca <80 kbps kanalı 80× overdrive ediyordu (self-congestion çöküşü). min_rate artık TÜM state'lerde
    kanal-hayatta-kalma floor'u (=`PRE_CONVERGE_FLOOR_BW`); efektif rate BtlBw'yi izler.
    **CANCELLED:** `min_rate_bps=10_000` mutlak sabiti (MUST-NOT: <80 kbps kanalda floor olarak uygulama).
  - **[R3] restore-burst kanal-türevli:** PreConverge çıkış burst'ü + micro-burst cap artık `clamp(rate·
    BURST_WINDOW, 1 birim, ~2×MTU)` — bant-kör 2×MTU=2400B sabiti yerine. sub-kbps'te 1-birim (backlog-spike
    yok), hızlı kanalda micro-burst. **CANCELLED:** `restore_burst`'ün sabit 2400B dump'ı (MUST-NOT: convergence
    sonrası bant-kör 2×MTU grant).
  Zarf: [R8]'in "self-congestion imkânsız" iddiası artık 4-decade'de (0.25k/1k/100k/1M) gerçekten tutar —
  önceden yalnız ≥80 kbps + küçük iş-yükünde tutuyordu. (Kalibrasyon: §16.1 pacer satırları.)

**(B) Veri — programlı geometrik kopya takvimi (forward-redundancy sürücüsü):** [R1/§2.2]

> **[D-GAP-2 / AMENDMENT-9 BAĞI — 2026-09-23]** Bu yolun emisyonu, transport-layer-tdd [AMENDMENT-9]'daki
> "hop-ACK almadan ardışık tüketilen PN ≤ K" tavanına TABİDİR (K ≪ 2^15, değer ~~[KALİBRASYON] KS-PN-1~~ **K = 1024** — KS-PN-1 KAPANDI 2026-10-03, DR-CONNECT-1 H5).
> Bugün bu yolda SAYI TAVANI YOKTUR (ölçüldü: identity §28.1-A) — tavan SPEC-ONLY'dir, kod bu turda değişmedi.
```
İlk birim t=0'da gider; ACK beklenmez. Kopyalar geometrik: t=0, k_base·RTT_floor, ×COPY_MULTIPLIER, ...
  (taban/çarpan serialization fiziğinden — §16 [CAL]; UYDURULMAZ). Tetik: "henüz hiçbir ölçüm yok".
Stop: birimin kopyaları o birim Noise-authenticated SACK ile teyit edilince DURUR (RX_DUP_SUPPRESS §4).
  Akışın İLK SACK'i cadence-exempt (§12 ACK_CADENCE'i atlar, HEMEN emit) → ilk SACK ~RTT_floor'da →
  k_base·RTT_floor > RTT_floor temiz kanalda ilk kopyayı ateşlemeden iptal → tx_wire/altsınır ≈ 1×.
Sayı-tavanı YOK: aralık COPY_MAX_INTERVAL'e kadar büyür, deadline'a dek düşük tempoda sürer (§12 no_ack(T)).
[K-D2] Kopya-takvimi t₀'ı = birimin PACER'DAN WIRE'A ÇIKTIĞI an (ScheduleTransmit'te pacer.try_send başarısı),
  sunum/enqueue anı DEĞİL → copy_due = wire_emit + k_base·RTT_floor. Gerekçe: RTT_floor "birim yalnız yolda"
  varsayar; burst kuyruğu bunu bozar → saat wire-emit'e bağlanınca serialization/kuyruk gecikmesi otomatik
  hesaba girer (§2b pacer-tek-kapı ilkesinin kopya takvimine simetrik uygulanması). Kopyalar da AYNI pacer'dan
  geçer (§13); pacer'ı baypas eden gönderim yolu YOK (DATA/retransmit/kopya tek token bucket'ı paylaşır).
```

**(C) PRE_CONVERGE yasakları (NORMATİF — mekanizma seviyesinde):**
```
(1) loss ilanı YOK — MEKANİZMA-AGNOSTİK: §5'in TÜM loss-ilan tetikleri DEĞERLENDİRİLMEZ:
    §5(a) PACKET_THRESHOLD (≥K=3 yüksek-seq ACK) + §5(b) TIME_THRESHOLD (9/8×srtt). (PTO ayrıca (5).)
    "loss İLANI yok" tetik-ayrımı yapmaz → ilk geçerli SACK (kilit) öncesi hiçbir birim "kayıp" sayılmaz.
(2) emergency duplication YOK (§11 escalate DONDURULUR; TDD §10 "kör escalate ETME").
(3) sessizlikten estimator besleme YOK (sessizlik DAVRANIŞ seçer — geometrik tempo — VERİ üretmez).
(4) [R14] ack_confidence (§5.1) DEĞERLENDİRİLMEZ — FULL (1000) sabitlenir; TÜM tüketicileri
    (§3 ACTIVE→RECOVER_FWD, §8.4/§15 fuzzy, §11 emergency, §12 decimation) ilk gerçek dönüş örneğine
    kadar DONDURULUR. Gerekçe: threshold=ACK_SILENCE_MULT×srtt ≈300ms ≪ RTT_floor → çöküş değil
    taban-bw serialization gecikmesi → yanlış-pozitif.
(5) PTO ARM EDİLMEZ (§3.1 arm_pto_timer ATLANIR) — srtt BOŞ (§3.3-A) → §12 PTO formülü tanımsız
    (Ek-B srtt-seed CANCELLED). Retransmit/probe TEK sürücüsü §3.3-B geometrik kopya (RTT_floor-tabanlı,
    srtt-bağımsız); yoksa PTO ≈ min_pto RTT_floor'dan önce ateşler → geometrik kopyayla çift retransmit
    (Ek-B spurious-storm'unu geri getirir). STARTUP'ta (ilk SACK sonrası) PTO normal armanır.
```

**(D) Kademeli tahliye — sinyal başına AYRI kilit:**
```
İlk geçerli Noise-authenticated SACK/echo → RTT + loss kilidi açılır (gerçek örneklerle) → STARTUP.
bw kilidi AYRI: ölçülmüş bw örnekleri BİRİKENE kadar taban varsayımı sürer (tek örnek "bant VAR" der,
  "ne kadar" DEMEZ) → FEC/redundancy agresifliği bw-kanıtıyla kademeli açılır (min_rate dersi; §8.5
  budget clamp bw-kilidine bağlı).
```

**(E) Return-path ACK cadence — cold-start (firm co-requisite):** [K2/§3.C]
```
Receiver de cold-start yapar: dönüş bw'sini bilmez → taban-önselle başlar
  ACK_CADENCE ≥ sack_wire / (RETURN_BUDGET_FRACTION × return_bw_önseli)   # §12
DATA arrival-rate'iyle hızlanır. İLK SACK cadence-exempt (yukarıda B). FULL adaptif decimation
(reverse-path-impaired N↑ eskalasyonu) ADIM-6 ana gövdesi — burada yalnız minimal taban.
```

**(F) Ölçüm birimi (probe/echo) — hızlandırıcı, opsiyonel:** [K-KEYSTONE/§2.3b]
Probe/echo = authenticated hop-control frame (§11.1, `CTRL_PATH_PROBE` genişletmesi). **SoC-guard:**
probe güvenlik katmanının birimidir; transport yalnız içeriği (probe_seq/ölçüm) üretir, güvenlik
dikişinden opak geçer — AEAD'i güvenlik katmanı uygular (INV-ENGINE-SOC; QUIC-tipi bütünleşmeye
YAKLAŞTIRMAZ, keystone yalnız zarfı değiştirdi, katman sınırını değil). *"Probe hızlandırıcıdır; hiçbir
şeyi bloklamaz, receiver'da YENİ state açmaz, yokluğu VEYA geçersizliği otoriter sonuç doğurmaz."*

---

## 4. Receiver State Machine (NORMATİF)

```
Durumlar:
  RX_NORMAL       → in-order/near-order teslim; windowed ACK
  RX_REORDER      → gap var, reordering_threshold içinde → bekle (spurious-rtx önle, §9.3)
  RX_FEC_DECODE   → block/shard framing; eksik data shard'ları parity'den reconstruct (§10)
  RX_DUP_SUPPRESS → duplicate/retransmit geldi, zaten teslim → drop + ACK teyit

Akış:
  on_datagram(d):
    # GÜVENLİK katmanı (Noise+kompakt, §0.2) d'yi çözer/verify eder; transport VERIFIED opak birim + metadata alır.
    # (verify FAIL → güvenlik katmanı silent drop, TDD §15; transport bunu hiç görmez.)
    if SESSION_DATA: inner-E2E teslim path (E2E verify güv. katmanında)
    if FEC_SHARD:
        # per-shard AEAD verify (code-then-encrypt-authenticate-shards) GÜVENLİK katmanında yapılır;
        # unauthenticated shard FEC decoder'a ASLA girmez (INV-FEC-1, authenticate-before-decode).
        # transport yalnız AUTHENTICATED plaintext shard + (block_id/shard_index/k/r) metadata'sını alır:
        buffer shard; if block reconstructable: RX_FEC_DECODE → reconstruct → teslim
    update rx_bitmap (alınan seq'ler — metadata)
    if gap ∧ within reordering_threshold: RX_REORDER (bekle)
    if gap ∧ beyond threshold: mark missing → ACK'te SACK range
    schedule_ack()   # §2.1, §12
    duplicate → RX_DUP_SUPPRESS: teslim etme, yalnız ACK (§9.4)
```

- Teslim sırası: hop transport in-order teslim GARANTİ ETMEZ zorunlu değil (E2E envelope seq'i uygulama sıralar); ama block reconstruction için shard toplama gerekir. Reorder toleransı §9.3.

---

## 5. Loss Detection (NORMATİF — loss = recovery sinyali, RATE sinyali DEĞİL)

QUIC RFC 9002 packet-threshold(3)+time-threshold(9/8) **loss-based CC** içindir; biz loss'u rate'e bağlamayız (INV-RATE-1). Loss'u yalnız **recovery** (ARQ/FEC) ve **residual ölçüm** için tespit ederiz.

```
Packet kayıp SAYILIR (recovery için) eğer:
  (a) PACKET_THRESHOLD: daha yüksek seq'li ≥ K paket ACK'lendi (reordering toleransı).   K=[STD:BBR/QUIC 3]
  (b) TIME_THRESHOLD:   send_time + max(TIME_THRESH × srtt, min_delay, K_c × EAI) < now
                        ∧ LATER-ACK KANITI (seq < largest_acked) VAR.                    [TUR-1a/1b, D-1]
                        TIME_THRESH=[STD:9/8]; min_delay=[CAL] (reordering tail).
      [TUR-1a / D-1/1] KANIT-ŞARTI (RFC 9002 §6.1'in atlanmış yarısı): higher-ACK kanıtı olmadan
      time-threshold KESİN loss YAZMAZ. Saf sessizlik loss metriğini ARTIRMAZ; fakat liveness
      amacıyla probe/retransmit tetikleyebilir (§5.1 ackconf → RECOVER_FWD spekülatif kopya —
      Duplicate sayılır, loss sayılmaz). Kanıtsız-eşik K-1 kompozisyonunun köküydü (ölçüldü: 27/27).
      [TUR-1b / D-1/2] CADENCE-FARKINDA TABAN (ZM-4): K_c × EAI;
      EAI = ACK_FREQ_N × ort_inflight_birim / send_rate_fiili + sack_wire/return_bw_önseli.
      Sender-yerel + deterministik; K_c=1.5 [CAL matris {1.25, 1.5, 2.0}]; return_bw_önseli
      §3.3-E-FULL'de ölçülen return_bw ile değişir [CAL]. "SACK'i henüz gelmedi" ≠ "kayıp".
[TUR-2a / D-1/3, D-1/5] W_loss OLAY-LEDGER'ı (NORMATİF): kayıp bir OLAY değil HİPOTEZdir.
  Her ilan seq-başına kayıt: Suspected → Confirmed (un-loss ufku kanıtsız geçti — ufuk =
  cadence-taban, geç-SACK'in makul varış penceresi) | Spurious (geç-ACK geldi → GERİ-ALINDI:
  pending-retransmit İPTAL + ReorderTracker.note_spurious beslenir + packet-threshold
  k = max(K, ρ×recent_max_reorder) TÜKETİLİR (§9.3 kablolandı) + retx_count==0 ise orijinal
  RTT örneği GEÇERLİ [kol-1] **[v1.1: retx_count'tan bağımsız, yalnız §6 KABUL KURALI sağlanırsa — kayıt ilan
  anında largest_acked'in altındadır, alıcının en büyüğü monoton büyür (INV-003): ürün yolunda yalnız ters-yol ACK
  yeniden sıralamasında erişilir]**); pencere-dışına yaşlanan olay düşer (Expired → doğal decay).
  Ledger + pencere-paydası bounded: LEDGER_MAX=4096, en-eski-önce eviction + audit (SR-AUDIT-001).
[TUR-2b / D-1/4] ACTIVE-FLIGHT / TOMBSTONE AYRIMI (NORMATİF): kayıp-ilanı "inflight'tan silinir"
  DEĞİLDİR — İKİ ayrı kavram: (a) *active-flight muhasebesi*: şüpheli paket bytes_in_flight/cwnd'den
  DÜŞER (gönderim durmaz); (b) *retained transmission record (TOMBSTONE)*: gönderim kaydı
  {unit, send_ns, bw_sample, retx_count, origin} SAKLANIR — geç-ACK eşleştirme için.
  TTL = K_tomb×T_loss(ilan-anı) [zeyl: onaylı `min(mesaj-deadline,…)` kolu motorda uygulanamaz —
  per-unit deadline engine'e taşınmıyor]; tavan TOMBSTONE_MAX, en-eski-önce eviction +
  `tombstone_evicted` audit. Geç-ACK çarparsa: kol-1 (retx_count==0) → BW-örneği NORMAL + Data'ysa
  RTT **[v1.1: köken Data, Retransmit ya da Duplicate; yalnız §6 KABUL KURALI sağlanırsa (TUR-2a erişim notu);
  BW-örneği koşulsuz]** (sansür biter — K-1 sansür-kolu kapanışı); **kol-2 [TUR-2c, NORMATİF]** (superseded —
  retransmit EMİT edilirken orijinalin tombstone'u işaretlenir, kayıt ilan-anında DONMAZ):
  RTT ~~ALINMAZ~~ **[v1.1: §6 KABUL KURALI'yla — superseded işareti RTT'yi dışlamaz; örnek o iletimin gönderim
  anından]** + bu ACK'ten rate-örneği ÜRETİLMEZ + **delivered-KREDİSİ girer** (`credit_delivered`:
  byte'lar wire'ı gerçekten geçti → sampler kümülatifine; gelecek örnek-delta'larına dürüst yansır)
  + `sample_skipped_ambiguous` sayacı. ZEYL: per-transmission-seq tasarımında (retransmit YENİ seq)
  klasik Karn-belirsizliği zayıftır — seq iletimi tekil tanımlar; kurallar yine MUHAFAZAKÂR tutulur
  ~~(superseded iletimin RTT'si güven-dışı).~~ **[v1.1: RTT için kalktı — §6 KABUL KURALI; muhafazakâr kural
  BW-örneği için DEĞİŞMEDİ]** Retransmit-seq'in KENDİ ACK'i inflight-yolundan tam örnek
  verir (belirsizlik yok) **[v1.1: RTT yalnız §6 KABUL KURALI sağlanırsa]**. Süresi/tavanı dolan kaydın **[v1.1: ya
  da birim başına K_MAP budamasıyla (K4G-05c) düşen kaydın — transport-layer-tdd §12 AMENDMENT-12; birimi TEYİT ETMEZ]**
  geç-ACK'i → Expired/Unknown (örneksiz;
  teslim-damgası app/bench-katmanında bağımsız).
  ZEYL-2 [review]: superseded-işaret TEKDÜZEDİR — birimin HERHANGİ yeniden-emisyonu (ARQ retransmit,
  FWD-dup pending-drain, CapExceeded→HandToForward) orijinalin tombstone'unu işaretler; kol-1/kol-2
  seçimi recovery-yoluna bağımlı OLAMAZ. Kol-2 geç-ACK'i kanonik spurious-teyididir → §9.3
  `note_spurious` BURADAN DA beslenir (un-loss yolu yalnız retx-öncesini yakalar).
  ZEYL-3 [review]: `delivered` WIRE-BYTE muhasebesidir (kapasite ölçümü — her iletim ayrı sayılır;
  spurious-retransmit'te iki kopya da wire'ı geçtiyse İKİSİ de sayılır, BBR semantiği). D-1/3'ün
  "teslimat BİR KEZ muhasebe" cümlesi BİRİM-teslim düzlemindedir (kopya-iptal + app/bench idempotent
  damga) — iki düzlem karıştırılmaz. Kredinin delta-etkisi (başka paketin penceresine düşen kredi o
  örneğin delta'sını büyütür) stok-BBR eşdeğeridir; interval-tarafı snapshot-eşleşmesiyle yansızdır.
loss_pre_fec (fuzzy girdisi) = loss_windowed_ppm = Confirmed_units / pencere-içi-gönderim;
  W_loss = K_wl × ack_epoch; ack_epoch = max(corrected_srtt, EAI).  [TUR-2a — ömür-boyu
  monoton ratchet CANCELLED (K-1 fuzzy-zehirlenmesinin köküydü); ömür-boyu sayaç yalnız diag]
residual_loss = unrecovered_units / sent_units                  (FEC/ARQ SONRASI)
→ loss_rate fuzzy girdisi = fuzzify(loss_windowed_ppm)  (LOW/MED/HIGH, §15)
→ NOT: bu değerler send_rate'i düşürmez; fec_parity_ratio ve arq_aggressiveness'ı besler.
```

**Reordering threshold (adaptif, §9.3):** `reorder_thresh = max(K, ρ × recent_max_reorder)` — yüksek jitter'da spurious retransmit önler.  ρ=[CAL].

### 5.1 ack_confidence — reverse-path canlılık sinyali (NORMATİF)

`ack_confidence` §3 (state geçişleri) · §8.4/§15 (fuzzy girdi) · §11 (forward-recovery tetiği) tarafından
TÜKETİLİR ve §8.1 SrcState'te tutulur; bu alt-bölüm hesabını KİLİTLER (önceden tanımsızdı — [AMENDMENT-2]).

```
ANLAM: reverse (ACK/dönüş) yolunun canlılığı, per-mille. YÜKSEK = ACK'ler bekleneni sağlıyor;
       DÜŞÜK = ACK SESSİZLİĞİ (dönüş yolu bozuk/çökmüş → ARQ'ya güvenilemez → forward-recovery).

TANIM (sessizlik-tabanlı, NORMATİF):
  [R14] PRE_CONVERGE (§3.3) → ack_confidence = 1000 (FULL) SABİT; aşağıdaki hesap DEĞERLENDİRİLMEZ.
        Gerekçe: threshold=ACK_SILENCE_MULT×srtt ≈300ms ≪ RTT_floor (1.4s@1kbps) → cold-start sessizliği
        reverse-path çöküşü DEĞİL taban-bw serialization gecikmesi → yanlış-pozitif emergency'yi keser.
  active = (inflight ∨ pending_recovery ∨ tx_queue) boş DEĞİL      # bekleyen teyit var mı
  active DEĞİLSE → ack_confidence = 1000 (FULL)                    # outstanding iş yok, sessizlik anlamsız
  aksi:
    silence   = now − max(last_ack_time, first_send_time)          # hiç ACK yoksa ilk gönderimden
    threshold = ACK_SILENCE_MULT × srtt                            # [CAL: ACK_SILENCE_MULT ~3]
    silence ≤ threshold → 1000
    silence > threshold → clamp(threshold·1000 / silence, 0..1000) # monoton azalan
```

**NEDEN oran (sent/received) DEĞİL:** yeni gönderilmiş paket "düşük güven" DEĞİLDİR — henüz ACK'lenme
zamanı gelmemiştir; yalnız SESSİZLİK reverse-path çöküşüdür. Naif sent/received oranı taze gönderimi
yanlışlıkla LOW'a çeker → STARTUP'ta anında sahte-emergency. Sessizlik modeli bunu ayırır (last_ack
gelince silence sıfırlanır). Somut `ACK_SILENCE_MULT` [CAL] netem baseline'dan; mekanizma [STD].

> **[AMENDMENT-2] (impl-driven, gerekçeli — kod-spec tutarlılığı):** Sinyal §8.1 state alanı +
> §3/§8.4/§11 tüketicisiydi ama HESABI hiçbir yerde tanımlı değildi. İmplementasyonda somutlaşınca
> (naif oran → sessizlik modeli düzeltmesi) semantiğiyle spec'e eklendi. `residual_loss` ile birlikte
> §11 `health = f(ack_confidence↑, residual_loss↓)`'i besler.

---

## 6. RTT / Jitter / Queue-Delay Estimator (NORMATİF — formüller)

```
Girdi: ~~her acked paket için~~ **[v1.1: aşağıdaki KABUL KURALI'nı sağlayan iletim için; ACK çerçevesi başına ≤ 1]**
       rtt_sample = ack_recv_time − pkt.send_time − ack.ack_delay
       (ack_delay çıkarılır; negatifse ack_delay untrusted → sample invalid, §7.3)
       [TUR-1x] min-rtt guard (RFC 9002 §5.3 semantiği, NORMATİF): düzeltme yalnız
       `raw − ack_delay ≥ gözlenen_min_rtt (rtprop)` ise uygulanır; underflow ya da min-rtt-altı
       → RAW sample işlenir **[v1.1: rtprop VARKEN ack_delay ≥ rtprop ise örnek ALINMAZ — KABUL KURALI (e)]**, örnek
       ATLANMAZ (sample-skip = estimator açlığı, K-1 sınıfı YASAK) **[v1.1: yasak kabul edilmiş örneğe aittir;
       KABUL KURALI reddi beyanlı estimator-donmasıdır, tazelik ölçütüyle izlenir]**.
       Bozuk/kötü ack_delay path-RTT'yi gözlenen-min altına çökertEMEZ. Saat-sınıfı: gözlenen-min
       (kanal-türevi) — mutlak wall-clock taban (1ms) 4-decade'de GEÇERSİZ çıktı (loopback gerçek
       RTT sub-ms; bench loopback ÖLÇÜMÜYLE yanlışlandı) → CANCELLED, ZM-2 örneği.
       [v1.1 · K4G-05b, NORMATİF — sahip kararları CEVAP PAKETİ 5, 2026-10-16; CEVAP PAKETİ 9, 2026-10-24] KABUL
       KURALI: ACK çerçevesi başına en çok BİR örnek; yalnız şunlar birlikte sağlanırsa alınır — (a) çerçevedeki
       gönderilmiş (sender'ın ürettiği) seq'lerin EN BÜYÜĞÜ ele alınır (en yeni iletim, §3.1; ack_delay onundur,
       §2.1); (b) bu seq bu çerçeveyle İLK KEZ onaylanır (uçuştaki kayıt, kol-1 ya da kol-2 tombstone, un-loss
       eşleşmesi) — değilse bu çerçeveden örnek ALINMAZ; daha küçük yeni-onaylı kayda GERİ DÜŞÜLMEZ; (c) kaydın kökeni
       Data, Retransmit ya da Duplicate'tir ve gönderim zamanı O İLETİMİN gönderim anıdır — her iletim taze seq alır,
       eşleşme tek anlamlıdır, Karn dışlaması gerekmez; FEC onarım sembolü örnek VERMEZ; (d) ack_delay kodlama
       tavanında DEĞİLDİR (V2: 65535 × 256 µs ≈ 16.78 s, §2.1 satürasyonu: alıcının tutma süresi bilinmez; V1 çerçeve
       yalnız decode-uyumluluk içindir, üründe üretilmez); (e) rtprop varken min-rtt guard düzeltmeyi reddederse
       ack_delay < rtprop olmalıdır — aksi hâlde RAW, tutma süresiyle en az rtprop kadar şişmiştir (FLOOR kodlama:
       decoded ≤ gerçek tutma, §2.1). Bayat örnek alınmaz. Kural srtt yokken (ilk örnek) de AYNEN geçerlidir. Kesinti
       sonrası ilk taze örnek döngüyü yeniden başlatır: DEĞİŞMEYEN kuantum döngüsü Δt ≥ 64·T iken n = 64 uygular.
       Beyanlı sonuç: çerçevenin en büyüğü FEC onarım sembolü ya da daha önce onaylanmış bir seq olduğunda örnek
       alınmaz — srtt donar, şişmez. TAZELİK ÖLÇÜTÜ (yalnız gözlem; karar girdisi DEĞİL): son kabul edilen örneğin
       yaşı ≤ max(10 × srtt, RTO_MAX) (RTO_MAX = 30 s, transport-layer-tdd §12 [AMENDMENT-12]); aşılırsa `rtt_stale`
       sayacı artar (eşiği aşan her dönemde bir kez). Teslim teyidi, BW-örneği ve delivered-kredi bu kurala BAĞLI
       DEĞİLDİR (§5 kol-1/kol-2 aynen). Karar metinleri: `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`.

srtt / rttvar (RFC 6298 türevi, EWMA):
  ilk sample: srtt = rtt_sample;            rttvar = rtt_sample / 2
  sonraki:    rttvar = (1−β)·rttvar + β·|srtt − rtt_sample|      β=[STD:1/4]
              srtt   = (1−α)·srtt   + α·rtt_sample                α=[STD:1/8]

RTprop (propagation, min-filter — BBR-türevi):
  RTprop = min(rtt_sample) over window W_rtprop
  [TUR-3a D-2/1, NORMATİF] W_rtprop = max(10s [STD taban], K_rtprop × corrected_srtt);
  K_rtprop=8 [CAL matris {6,8,10}]. Saat-sınıfı: kanal-türevi (ZM-2 — 10s sabiti sub-kbps'te
  1-4 örneğe düşüyordu). BtlBw penceresi simetrik: W_BTLBW = max(10s, K_bw × ack_epoch),
  ack_epoch = max(corrected_srtt, EAI); K_bw=8 [CAL] (§7.2).
  [TUR-3a D-2/2, NORMATİF] KEEP-LAST: max/min-filter SON örneği ASLA expire etmez —
  budama filtreyi boşaltamaz (btlbw=0 flap'i yapısal biter; geçersiz-güncelleme yolunda ölçüldü).
  (pencere kayan; DRAIN_RTPROP probe ile tazelenir, §8.3)

queue_delay (congestion sinyali — delay-based):
  [TUR-3a.5-α, NORMATİF] queue_delay = min_{W_qd}(rtt_sample) − RTprop   (≥0; AYAKTA-kuyruk)
  W_qd = K_qd × ack_epoch (K_qd=2 [CAL {1,2,4}]); W_qd ≤ W_rtprop invariantı; keep-last;
  [STD] 10s tabanı BİLİNÇLİ YOK. ESKİ tanım (srtt − RTprop) CANCELLED: srtt-EWMA lag'ı boş
  kuyruğu dakikalarca dolu gösteriyordu (ölçüldü — 42-kilidi). Kuyruk-fiziği: örnek kuyruğu
  YAŞAMIŞ paketten gelir → gürültü tek-taraflı-yukarı → kısa-min anlamsal-doğru + salıverme
  ANINDA (srtt-clock-v1 §2.2; ölçüm-α: 42→45, salıverme tek-örnekte qd 17.3s→0.74s).
  → fuzzify(qd_ratio = queue_delay×1000/RTprop) LOW/MED/HIGH (§15). Congestion sinyali, loss DEĞİL.
  [TUR-3a.5-β, NORMATİF] srtt/rttvar KUANTUM-DÖNGÜ: güncelleme aynı örnekle n=clamp(Δt/T,1,64)
  kez; T = rtprop/K_CONV (K_CONV=4 [CAL {2,4,8}]) — yakınsama KANAL-ZAMANLI (ZM-1 estimator-
  katmanı fix'i; ack_epoch öz-referans tuzağı REDDEDİLDİ). Floor-1: Δt≤T → RFC-6298 bit-özdeş.

jitter / reordering score:
  jitter_score     = rttvar_normalized = rttvar / max(srtt, ε)
  reordering_score = EWMA(out_of_order_arrivals / total_arrivals)     [CAL: EWMA α_reorder]
  → fuzzify(max(jitter_score, reordering_score)) → jitter girdisi
```

---

## 7. Delivered-Rate Bandwidth Estimator (NORMATİF — "ana yetenek"; BBR-türevi)

Bu bölüm belgenin çekirdeği: **bandwidth loss'tan türetilmez, delivered-rate'ten ÖLÇÜLÜR.**

### 7.1 Rate sample (paket-bazlı, BBR delivery-rate)
```
Her paket gönderilirken sakla:
  pkt.delivered      = total_delivered_bytes (o an)
  pkt.delivered_time = now
  pkt.first_sent_time, pkt.is_app_limited

ACK gelince (paket p için):
  total_delivered_bytes += p.size
  delivered_now = total_delivered_bytes
  ack_elapsed   = now − p.delivered_time            # ACK-tarafı geçen süre
  send_elapsed  = p.send_time − p.first_sent_time    # gönderim-tarafı geçen süre
  interval      = max(ack_elapsed, send_elapsed)     # BBR: iki taraftan büyüğü (ACK-compression'a dayanıklı)
  delivered     = delivered_now − p.delivered
  if interval < MIN_RATE_INTERVAL: sample GEÇERSİZ (çok kısa)    MIN_RATE_INTERVAL=[STD:BBR ~1ms/1RTT]
  bw_sample = delivered / interval
```

### 7.2 BtlBw (bottleneck bandwidth, max-filter)
```
BtlBw = max(bw_sample) over window W_btlbw     W_btlbw=[STD:BBR 6-10 RTT]
  (max-filter: gerçek bottleneck'i yakalar; app-limited/geçersiz sample'lar BtlBw'i DÜŞÜRMEZ)
bw_est için raporlanan = BtlBw × estimator_confidence_gate  (düşük confidence'ta muhafazakâr, §7.3)
```

### 7.3 Sample geçerlilik (NORMATİF — estimator'ı ŞİŞİRME/çökertme koruması; TDD §5.1 validity)
```
bw_sample GEÇERSİZ (BtlBw'e KATILMAZ) eğer:
  app_limited            : tx_queue boştu (inflight < cwnd) → gönderim hızı bw'yi yansıtmaz
  ack_compression_score > ACK_COMP_THRESH : ACK'ler toplu geldi → interval yanıltıcı (send_elapsed devreye)
  reverse_path_impaired  : ack_confidence LOW → ACK zamanlaması güvenilmez
  ack_delay_untrusted    : rtt_sample negatif / ack_delay > srtt anomali

estimator_confidence (EWMA, 0..1):   [CAL]
  geçerli sample → confidence ↑ ;  geçersiz/anomali → confidence ↓
  düşük confidence → SRC muhafazakâr pace (BtlBw'e tam güvenme), fuzzy "bilinmiyor" moduna kayar

app_limited işareti:
  paket gönderilirken (tx_queue boş ∧ inflight < cwnd) → pkt.is_app_limited=true
  bu paketten türeyen bw_sample BtlBw max-filter'a GİRMEZ (yalnızca "≥ mevcut BtlBw" ise günceller)

ack_compression_score:
  W penceresinde: beklenen_ack_spacing (paket pacing'inden) vs gerçek_ack_spacing oranı;
  gerçek ≪ beklenen → compression yüksek. score = clamp(1 − actual_spacing/expected_spacing).   [CAL]
```

### 7.4 "ana yetenek" invariant'ı
```
INV-ENGINE-BW: bandwidth DAİMA delivered-rate'ten (BtlBw); loss'tan ASLA türetilmez.
INV-ENGINE-SEP: bandwidth (BtlBw) ⟂ congestion (queue_delay) ⟂ loss (loss_pre_fec) — üç bağımsız ölçüm.
INV-ENGINE-RATE: loss send_rate'i düşürmez (INV-RATE-1); yalnız queue_delay + inflight_cap pace eder.
```

---

## 8. Congestion / Rate Control Algorithm — SRC (NORMATİF)

BBR ölçüm çekirdeği + hostile-uygun gentle probe + delay-based backoff (loss DEĞİL).

### 8.1 State
```
struct SrcState {
  btlbw, rtprop, srtt, rttvar, queue_delay,
  delivered_rate, estimator_confidence,
  send_rate, pacing_gain, cwnd_gain,
  inflight_bytes, cwnd_bytes, tx_queue_delay,
  loss_pre_fec, residual_loss, ack_confidence,
  state: SenderState, cycle_idx, rtprop_stamp,
}
```

### 8.2 Temel denklemler
```
BDP        = BtlBw × RTprop
cwnd_bytes = max(cwnd_gain × BDP, MIN_CWND)          cwnd_gain=[STD:BBR 2]; MIN_CWND=[CAL] (≥4×MTU)
send_rate  = pacing_gain × BtlBw                      (§13 pacing)
inflight cap: inflight_bytes ≤ cwnd_bytes            (üstündeyse gönderme, kuyrukta beklet)
```

### 8.3 Probe döngüsü (gentle — BBR ProbeBW gain-cycle DEĞİL)
```
STEADY'de:
  pacing_gain = 1.0 varsayılan.
  BW-probe: her PROBE_BW_PERIOD'da kısa süre pacing_gain = 1.0+g_up (ör. 1.25) → BtlBw taze;
            ardından pacing_gain = 1.0−g_dn ile biriken kuyruğu boşalt.   g_up,g_dn=[CAL]
  (BBR'ın 8-fazlı pace_gain cycle'ı ALINMADI — hostile path'te agresif probe zararlı; tek up/down yeter.)

DRAIN_RTPROP (RTprop tazeleme):
  RTprop_stamp > RTPROP_WINDOW eskiyse: cwnd_gain'i kısa süre düşür (inflight drain) → min rtt_sample ölç → RTprop güncelle.
  RTPROP_WINDOW=[STD:BBR 10s]; drain süresi ≥ 1 RTT.
```

### 8.4 FEC/ARQ arbitrasyonu (FÜZYON — aynı state, §15 fuzzy)
```
Girdi:  loss_rate, jitter, bandwidth, queue_delay, ack_confidence   (§15 fuzzified)
Çıktı:  fec_parity_ratio, arq_aggressiveness, send_rate_gain_hint, compress   [AMENDMENT-4: compress KALINTI — üretilir, hiçbir aktüatör tüketmez; sıkıştırma app-rpc §3'ün]
Kanonik kurallar (TDD §5.3 ile birebir):
  loss LOW                         → fec=0,    arq=light
  loss HIGH ∧ bw HIGH ∧ jitter LOW → fec=high, arq=moderate           (RS erasure, RTT-beklemeden recovery)
  loss MED  ∧ bw MED               → fec=med,  arq=med
  loss LOW  ∧ jitter HIGH          → fec=0,    arq=reorder-tolerant (RTO↑, fast-rtx↓)
  bw LOW ∧ queue_delay HIGH        → send_rate↓ (congestion; fec CAP)  [delay-driven, loss DEĞİL]
  loss HIGH ∧ bw LOW ∧ ackconf LOW → EMERGENCY_FORWARD_REDUNDANCY (§11)
```

### 8.5 Budget clamp (NORMATİF — §5.5 TDD)
```
available_redundancy_budget = f(BtlBw, mevcut tx ihtiyacı)   [CAL]
parity_final = min(fuzzy_parity, budget_cap, profile_cap)
  → düşük bantta yüksek loss bile parity'yi patlatamaz (dar bandı FEC israf etmez).
  → [AMENDMENT-3] budget_cap YALNIZ KÜÇÜLTÜR, SIFIRLAMAZ: budget_cap == 0 ∧ fuzzy_parity > 0 ise
    clamp UYGULANMAZ (fuzzy_parity aynen geçer) ve `diag_parity_budget_would_zero` sayılır.
    Gerekçe + §11 ilişkisi: bu bölümün sonundaki AMENDMENT-3 notu ve
    docs/transport-v1/adr/ADR-minimal-parity-exception.md.
send_rate_final = clamp(fuzzy_send_rate, [MIN_RATE, min(BtlBw_based_cap, profile_cap)])   MIN_RATE=[CAL]
  → [R8 işaret levhası] PRE_CONVERGE'de MIN_RATE floor UYGULANMAZ (MUST-NOT-APPLY) → cold-start pacing
    floor = PRE_CONVERGE_FLOOR_BW (§3.3-A / §16.1 / Ek-B). (Kural burada da işaretli — uzakta yazılıp
    sessizce çiğnenmesin; R8 dersi.)
```

---

## 9. ARQ Selective Retransmit Algorithm (NORMATİF)

```
RECOVER_ARQ state'inde (residual_loss>0 ∧ ack_confidence≥MED):
  missing = rx tarafından SACK'te bildirilen gap seq'leri (§2)
  for seq in missing (öncelik: en eski deadline'a yakın):
     if arq_mode_active (fuzzy arq_aggressiveness > 0):
        retransmit(seq) — pacing token bucket'tan (§13); inflight cap'e tabi
     if fec_dominant (fuzzy fec yüksek ∧ ackconf düşük): retransmit BASKILA — residual FEC ile kapanacak
  spurious önleme: §9.3
```

### 9.1 Retransmit budget
```
retransmit'ler DATA ile aynı pacing bütçesini paylaşır (ayrı queue, deadline-öncelikli).
ARQ_RETX_CAP: bir seq için max retransmit sayısı; aşılırsa FEC/forward'a devret veya connection-fail.   [CAL]
```

### 9.2 RTO/PTO ile ilişki: §12.

### 9.3 Spurious retransmit detection
```
reorder_thresh = max(K, ρ × recent_max_reorder)     (§5)
retransmit'ten SONRA orijinal ACK gelirse → spurious işaretle → reorder_thresh ↑ (adaptif) → gelecekte bekle.
DSACK-benzeri: rx duplicate aldığını ACK'te bildirir (§9.4) → sender spurious teyidi.
```

### 9.4 Duplicate suppression
```
rx zaten-teslim seq alırsa: teslim ETME (RX_DUP_SUPPRESS); ACK'i teyit et (cumulative ilerlet).
Bu, spurious retransmit'i sender'a bildirir + gereksiz uygulama teslimi önler.
```

---

## 10. FEC Block Format & Parity Selection (NORMATİF — INV-FEC-1 uyumlu)

> **SoC (§0.2) [AMENDMENT-5]:** FEC **POLİTİKA** (k,r; §8.4 fuzzy) ve **EXECUTION** (RS encode/reconstruct) TRANSPORT'un; **per-shard AEAD** SEAM'in (tel kenarı). ~~Eski: EXECUTION GÜVENLİK katmanının~~. Transport parity ORANINI verir; güvenlik katmanı encode+AEAD yapar; transport opak shard'ları pace/deliver/track eder + framing metadata'sını (block_id/shard_index/k/r) reliability için okur. Aşağıdaki kripto sırası GÜVENLİK katmanının davranışıdır — transport onu EXECUTE ETMEZ, referanstır.

**Kripto sırası (KİLİTLİ — TDD INV-FEC-1, §4.4.7): CODE-THEN-ENCRYPT-AND-AUTHENTICATE-SHARDS.** [AMENDMENT-5: AEAD sırası seam'in davranışıdır; RS'yi transport execute eder.]
```
Gönderen (güv. katmanı): block_plaintext → RS-erasure encode (k data + r parity) → (k+r) PLAINTEXT shard →
          HER shard AYRI AEAD encrypt+authenticate → wire'da yalnız ciphertext shard.
Alıcı (güv. katmanı):    shard al → ÖNCE per-shard AEAD verify/decrypt → YALNIZCA authenticated plaintext shard
          FEC decoder'a girer → eksik data shard'ları parity'den reconstruct.
          unauthenticated/tampered shard decoder'a ASLA girmez (authenticate-before-decode).
Transport: yukarıdakini ÇAĞIRMAZ; yalnız opak shard'ları taşır + metadata okur.
```

### 10.1 FEC_SHARD wire (fixed-width)
```
FEC_SHARD {
  # --- framing metadata (TRANSPORT okur — reliability/reconstruction; güv. katmanı AAD olarak authenticate eder) ---
  frame_type   : u8        # 0x20
  block_id     : u32_be    # blok kimliği (hop-scoped, wrap→rekey)
  shard_index  : u8        # 0..k+r−1
  k            : u8        # data shard sayısı (bu blok)
  r            : u8        # parity shard sayısı
  shard_len    : u16_be    # bu shard'ın payload uzunluğu
  # --- seam çıktısı (AMENDMENT-5: RS encode TRANSPORT'ta yapıldı; bu iki alanı tel kenarındaki SEAM üretir, transport yorumlamaz; INV-FEC-1) ---
  ciphertext   : shard_len bytes    # per-shard AEAD ciphertext  [SEAM — session E2E anahtarı]
  aead_tag     : 16 bytes           # per-shard AEAD tag         [SEAM]
}
# Kripto yapı (ct+tag) ccr §35/TDD §0.1'e aittir; bu belge onu transport-owned REDEFINE ETMEZ.
# reed-solomon-erasure (ADR-16); ErasureCodec trait arkasında (SIMD Beta geçişi trait ile) — encode/decode güv. katmanında.
# IMPL NOTU (INV-ENGINE-SOC tip-kilidi): transport kodunda ct+tag AYRI alan DEĞİL, TEK OPAK BLOB
#   (opaque_secured = ct||tag) olarak taşınır. Ayrı `aead_tag` alanı transport struct'ında BULUNMAZ →
#   tag'e dokunmak DERLENEMEZ; SoC yorum değil, TİP garantisi olur.
```

> **⚠️ AUDIT-FINDING METADATA-LEAK-01 (Alpha'da KABUL, Beta/Beta+'ta MİNİMİZE — kayıtlı karar):**
> FEC_SHARD framing metadata (`frame_type · block_id · shard_index · k · r · shard_len`) wire'da **PLAINTEXT** — yol üzerindeki gözlemciye (pratikte hub'a) açıktır. Kısmen kaçınılmaz (hub reconstruction/forward için framing'i bilmek zorunda; TCP seq'in açık olması gibi) ve **Alpha varsayımıyla tutarlı** (sealed sender Alpha'da YOK; hub metadata görür — federated model kabulü; cover traffic Beta+).
> **Bilinen sızıntılar (Alpha'da bilinçli kabul):**
> - `k, r` → **parity oranı = kayıp/emergency koşulunu sızdırır.** Global passive observer (DPI/EW) "bu bağlantı ne zaman EMERGENCY_FORWARD_REDUNDANCY'ye girdi"i r/k'dan okuyabilir. **En hassas alan.**
> - `block_id` → aynı bloğun shard'ları **linkable** (bir mesajın parçaları gruplanır) → trafik analizi sinyali.
> - `frame_type` (0x20) → trafik **sınıfı** sızar; tek-opak trafik indistinguishability hedefiyle (audio codec PT=96 kararı) gerilim yaratır.
> - `shard_len` → uzunluk sızıntısı (E2E'de kısmen kaçınılmaz).
>
> **Beta/Beta+ minimizasyon yönü (BURADA ÇÖZÜLMEZ, unutulmasın diye kayıtlı):** (a) `k,r` her shard'da DEĞİL, **blok-başına bir kez** (hub ilk shard'da öğrenir + cache) → r/k sızıntı yüzeyi daralır; (b) `frame_type` ayrımını SESSION_DATA ile tek trafik sınıfında gizle (indistinguishability); (c) cover traffic + padding (Beta+). Alpha'da §10.1 framing-açık implemente edilir; benchmark geciktirilmez.

### 10.2 Parity seçimi (k, r) — fuzzy-driven
```
r = round(fec_parity_ratio × k)     (§8.4 fuzzy çıktısı; §8.5 budget-clamp'li)
k = block'taki data shard sayısı (payload / shard_size; shard_size=[CAL], MTU-uyumlu)
kısıt: k + r ≤ RS_MAX_SHARDS (GF(2^8) → 255).   loss profiline göre r/k oranı §8.4.
tek-datagram / tiny payload: RS uygulanamaz → duplication (§11) doğru araç.
```

---

## 11. Emergency Forward Redundancy Algorithm (NORMATİF — ACK-bağımsız; TDD §5.6)

> **[D-GAP-2 / AMENDMENT-9 BAĞI — 2026-09-23]** Bu yolun emisyonu, transport-layer-tdd [AMENDMENT-9]'daki
> "hop-ACK almadan ardışık tüketilen PN ≤ K" tavanına TABİDİR (K ≪ 2^15, değer ~~[KALİBRASYON] KS-PN-1~~ **K = 1024** — KS-PN-1 KAPANDI 2026-10-03, DR-CONNECT-1 H5).
> Bugün bu yolda SAYI TAVANI YOKTUR (ölçüldü: identity §28.1-A) — tavan SPEC-ONLY'dir, kod bu turda değişmedi.

```
Tetik (fuzzy): loss HIGH ∧ bandwidth LOW ∧ ack_confidence LOW
  Gerekçe: %80 çift-yönlü loss → P(data∧ACK)=0.2×0.2≈%4 → ARQ ~25× round-trip'e sürünür.
           Bu köşede kurtarma ACK dönüş yoluna BAĞIMSIZ olmalı.

Davranış (payload tipine göre):
  tek-datagram / tiny (block/shard yok):
     → forward duplication: aynı payload D× gönder (D=2..3), ACK beklemeden.   D=[CAL]
  segment-block / multi-shard:
     → minimal parity shards (low-rate forward FEC), r/k düşük ama ACK-bağımsız.
  compress = ON (dar bantta bytes azalt).   [AMENDMENT-4: transport SIKIŞTIRMAZ — sahibi app-rpc §3; TDD §8]
  send_rate = minimal-reliable (MIN_RATE clamp; §8.5) — path'i boğma.

Geri çekilme (NORMATİF):
  path düzelirse (ack_confidence ↑ / residual_loss ↓) fuzzy redundancy'yi AZALTIR:
     3× → 2× → ARQ'ya iniş (delivered-rate confidence yükseldikçe).
  → EMERGENCY kalıcı DEĞİL; fuzzy tarafından ölçümle geri alınır.

GEÇİŞ KARARLILIĞI — RETREAT HYSTERESIS (NORMATİF, KİLİTLİ — asimetrik):
  Sorun: fuzzy girdiler SÜREKLİ (trapezoidal), emergency LEVEL DISCRETE (Dup3/Dup2/ARQ). Hostile
  rejimde (%50-95 loss) ACK seyrek → ack_confidence örneklem varyansı YÜKSEK → tek-eşikli sistem
  eşik etrafında 3×↔2× SALINIR → kıt bantta duplication israfı + duplication→ölçüm→mode pozitif
  geri-besleme riski. Çözüm iki katman:
  (1) ASİMETRİK Schmitt-trigger eşikleri: escalate (redundancy ARTIR) health < X;
      de-escalate (AZALT) health > Y; Y > X. Bant [X,Y] gürültüyü soğurur (bant içinde geçiş YOK).
      health = f(ack_confidence↑, residual_loss↓) per-mille.
  (2) ASİMETRİK dwell (recovery-obsesif taban gereği — yönler eş maliyetli DEĞİL):
      escalate dwell   = 0 / çok kısa → kötüleşmede ANINDA koru (geç escalate = korumasız kayıp =
                          "veri ULAŞSIN" tezinin ihlali; Schmitt bandı escalate'i gürültüden zaten korur).
      de-escalate dwell = min_dwell [CAL] → iyileşmede TEMKİNLİ (osilasyonun asıl kaynağı erken
                          de-escalate→re-escalate döngüsü; dwell bunu zamanla ayırır, duplication'ın
                          ölçüm etkisi sindirilene dek mode kilitli).
  Marjlar (X, Y, min_dwell) = [CAL] fixture (netem baseline'da kalibre; mekanizma [STD:Schmitt/kontrol-teorisi]).
duplication/parity yine available_redundancy_budget ile clamp (§8.5).   [AMENDMENT-3: BAĞLAYICI DEĞİL — aşağı bkz.]
```

> **[AMENDMENT-3] (impl-driven, gerekçeli — kod-spec tutarlılığı; Faz-6 dondurma, 2026-09-11):**
> §8.5 NORMAL adaptif FEC bütçesini yönetir. §11 MinimalParity, **dar ve kendi kapılarıyla sınırlı bir
> acil-durum istisnasıdır**; §8.5 bütçe kısıtına TABİ DEĞİLDİR (yukarıdaki "yine §8.5 ile clamp" satırı
> bu istisna için bağlayıcı değildir). İstisnanın kapıları — doğrulanmış uygulama, `sender.rs`
> `fec_controller_latch` emergency dalı: (1) fuzzy mod = EMERGENCY_FORWARD_REDUNDANCY
> (loss HIGH ∧ bw LOW ∧ ack_confidence LOW), **VE** (2) retreat-hysteresis merdiveni ≥ Dup2
> (health < X), **VE** (3) framing = segment-block. Üçü birden sağlanınca parity oranı
> `EmergencyConfig` sabitidir (Dup2 → 100‰, Dup3 → 200‰), yalnız profil `r_code_max` ile kırpılır ve
> NORMAL oranın YERİNE geçer (toplanmaz) — tek latch yolu, ikinci bir FEC emitörü YOKTUR. §8.5'in
> sıfırlama-reddi (yukarıdaki AMENDMENT-3 satırı) ise NORMAL yolun ayrı bir kuralıdır; iki mekanizma
> aynı köşede farklı kapılarla çalışır ve birbirini tetiklemez. Sınırlar: (a) §8.5 sıfırlama-reddi ve
> §11 istisnası birbirinden BAĞIMSIZDIR — "§8.5 sıfırlayınca §11 devreye girer" DEĞİLDİR; (b) Faz-5
> T0–T7 kampanyasında (990 koşu) ~~RECOVER_FWD/§11 yolu HİÇ ateşlemedi (`recover_fwd_entries = 0`,
> 330/330 Kama koşusu)~~ **[ERRATUM-2 (2026-10-18)]** 330 Kama koşusunun 15'inde RECOVER_FWD'e
> giriş var (`recover_fwd` > 0: T1a 2, T1b 7, T6 6; ham veri ve koşu listesi
> `reports/transport-v1/phase5/ERRATUM-2-recover-fwd-count.md`); §11 MinimalParity emisyonu ayrı
> sayaçla ölçülmedi — istisna tel-doğrulanmıştır (tv_emergency), saha-doğrulanmamıştır.

---

## 11.1 Cold-Start Probe/Echo — CTRL_PATH_PROBE genişletmesi [K4/K-KEYSTONE/R3/revize-D0 §4.2]

> **[D-GAP-2 / AMENDMENT-9 BAĞI — 2026-09-23]** Bu yolun emisyonu, transport-layer-tdd [AMENDMENT-9]'daki
> "hop-ACK almadan ardışık tüketilen PN ≤ K" tavanına TABİDİR (K ≪ 2^15, değer ~~[KALİBRASYON] KS-PN-1~~ **K = 1024** — KS-PN-1 KAPANDI 2026-10-03, DR-CONNECT-1 H5).
> Bugün bu yolda SAYI TAVANI YOKTUR (ölçüldü: identity §28.1-A) — tavan SPEC-ONLY'dir, kod bu turda değişmedi.

> **CANCELLED/SUPERSEDED [DR-CS-1-AMENDMANI; Abkarada-onaylı]:** Bu bölümün `0x12`-codepoint'i otoritatif
> `kama-noise-wire-tdd-v2.0 §2.56` registry'siyle ÇAKIŞIR (0x12=RECOVERED; runtime-assert). Probe/echo
> semantiği DR-CS-1 ile **0x19 CTRL_CS_PROBE / 0x1A CTRL_CS_ECHO**'ya taşındı; authoritative-byte-layout
> artık noise-wire-§2.57'dedir (PROBE_WIRE_EXACT=44B; ECHO_WIRE_MAX=43B). Bu bölüm ARŞİVDİR (semantik-
> kaynak olarak-okunabilir; byte-layout/codepoint için OKUNMAZ). Otorite-sırası: §2.56-registry >
> sealed-control-byte-layout > engine/runtime > bu-draft.

> Hızlandırıcı ölçüm birimi. **Noise İÇİNDE** (authenticated hop-control frame — INV-CRYPTO-1 / §1
> taksonomisi; below-Noise DEĞİL, Ek-B). `CTRL_PATH_PROBE (0x12)` versiyonlu genişletmesi — mevcut 0x12
> semantiği KORUNUR (spec'te 0x12 gövdesi hiç tanımlı değildi; ilk tanım burada, `subtype` ayrımıyla).
> **INV-ENGINE-SOC:** AEAD'i güvenlik katmanı uygular; transport opak metadata (probe_seq/seen_count/
> arrival_gap) okur, anahtar TUTMAZ. Yalnız KURULMUŞ akış içinde üretilir (Noise transport-anahtarı zaten
> var → +16B tag, yeni handshake YOK, INIT-öncesi pencere sorunu YOK).

```
CTRL_PATH_PROBE_v2 (probe) {           # sender→receiver, kurulmuş akış içinde
  [L2] hub_handle : u32_be             # demux (cleartext L2, SESSION_DATA ile aynı)
  [outer-AEAD zarfı içinde:]
    frame_type    : u8                 # 0x12 (§1.1: outer-AEAD sonrası İLK byte = frame_type; ACK/ACK_V2 ile aynı konvansiyon)
    subtype       : u8                 # cold-start probe alt-tipi (0x12-içi versiyonlama)
    probe_seq     : u16_be             # echo eşleştirme
    pad           : u8[N]              # amp≤1: probe_wire ≥ echo_wire olacak N (zarf-İÇİ)
  [AEAD tag : 16B]
}
CTRL_PATH_PROBE_v2 (echo) {            # receiver→sender, stateless (akışın MEVCUT demux/SACK state'i)
  [L2] hub_handle : u32_be
  [outer-AEAD zarfı içinde:]
    frame_type    : u8                 # 0x12
    subtype       : u8                 # echo alt-tipi
    probe_seq     : u16_be             # yansıtılan
    seen_count    : u32_be             # şu ana kadar görülen DATA
    arrival_gap   : u32_be             # [R3] son iki DATA/probe arası µs — u32 (u16=65ms kbps'te taşardı)
  [AEAD tag : 16B]
}
# [R4-fix] echo AEAD-gövdesi=12B (frame_type1+subtype1+probe_seq2+seen_count4+arrival_gap4) (+handle 4 +tag 16 = 32B wire).
# CANCELLED — omitted 11-byte echo body overhead [DR-CS-1]: sealed-control-yolu zorunlu (EXTENDED-outer 26B
# [wire_format+wire_flags+pn32 dahil] + control-inner 6B [inner_version+inner_kind+control_id]) → authoritative
# hesap ECHO_WIRE_MAX = 26+6+11 = 43B (noise-wire-§2.57). 32B-rakamı KULLANILMAZ.
# pad, echo'yu (32B wire) domine → amplification ≤ 1. nonce/token YOK: AEAD anti-forge/replay + proof-of-knowledge'ı kapsar.
```

**Davranış (NORMATİF):**
```
(i)   silent-drop — geçersiz/AEAD-çözülemeyen probe'a TEK BYTE cevap yok (aktif-tarama savunması AEAD-doğrulaması).
(ii)  amplification ≤ 1 — pad zarf-içi, echo wire-boyutunu domine eder (reflection kazanç sağlamaz).
(iii) responder rate-limit [CAL] — garbage'a AEAD-decrypt ucuz ama bedava değil (DoS taban).
(iv)  otorite: echo AUTHENTICATED → RTT/loss kilidini AÇABİLİR (§3.3-D); bw kilidi yine AYRI (birikim gerek).
      echo/arrival_gap tek örnek "bant VAR" der, "ne kadar" DEMEZ.
Statü: "Probe hızlandırıcıdır; hiçbir şeyi bloklamaz, receiver'da YENİ state açmaz, yokluğu VEYA
       geçersizliği otoriter sonuç doğurmaz. Yokluğunda sistem yalnız daha yavaş öğrenir."
```

---

## 12. Timers (NORMATİF)

```
ACK_DELAY_TIMER (receiver):
  max_ack_delay içinde ACK gönder; ama ACK_FREQ_N DATA alınınca hemen (hangisi önce).   [CAL]
  reverse-path impaired → ACK decimation: N ↑, budget ↓ (§11, §2.1).
  [K2/§3.3-E] BANDWIDTH-AWARE cadence (cold-start + genel): ACK_CADENCE ≥ sack_wire/(RETURN_BUDGET_FRACTION
    × return_bw) — dönüş kıt iken SACK seyrekleşir (kbps'te 20ms sabit → SACK-seli/self-congestion; §16).
  [R1] Akışın İLK SACK'i cadence-exempt: ilk DATA gelişinde ACK_CADENCE'i ATLAR, HEMEN emit edilir
    (yalnız sonraki SACK'ler rate-limitli) → PRE_CONVERGE geometrik ilk-kopya iptali (§3.3-B).

PTO (Probe Timeout — retransmit/probe tetik; RFC 9002-türevi ama loss≠congestion):
  PTO = srtt + max(RTTVAR_MULT × rttvar, min_pto) + max_ack_delay    RTTVAR_MULT=[STD:4]; min_pto=[CAL]
  PTO firing → probe/retransmit; PTO backoff exponential (2^n) ardışık firing'de.
  NOT: PTO send_rate'i DÜŞÜRMEZ (loss-decoupling); yalnız recovery tetikler.
  NOT: PRE_CONVERGE'de hem time-threshold loss-ilanı (§5) hem PTO ARM DONDURULUR (§3.3-C/1,5); srtt BOŞ
    olduğundan PTO tanımsız — retransmit/probe yerine §3.3-B geometrik kopya (RTT_floor-tabanlı) geçer.

RTPROP_PROBE_TIMER: RTPROP_WINDOW (§8.3) dolunca DRAIN_RTPROP.
IDLE_TIMER: idle > IDLE_TIMEOUT → IDLE (session korunur; TDD blackout-survival).   IDLE_TIMEOUT=[CAL]
  [R4] ROLÜ DEĞİŞMEZ: IDLE_TIMER yalnız IDLE'a geçirir (session korunur) — failover TETİKLEMEZ.
RETRY_BUDGET: [K6/R4] SABİT-SAYI DEĞİL, DEADLINE-TÜREVİ (deadline/COPY_MAX_INTERVAL) — sayı-tavanı YOK,
  zaman-tavanı var; geometrik kopya deadline'la sınırlı (bounded store-and-forward).   [CAL]
PATH_DEAD_TIMER: [R4] no_ack(T) → PATH_DEAD; T = IDLE_TIMEOUT'tan AYRI [CAL]. silence=now−max(last_ack,
  first_send) (data-pending iken monoton artar → erişilebilirlik ispatlı; §5.1 active). İki sınır:
  geometrik-takvim-span < T < deadline − failover_marjı (alt: kopyalar şans bulmadan ateşlenmesin; üst:
  failover'ın davranma süresi kalsın). PATH_DEAD = YOL-vazgeçişi (failover'a eskalasyon), TESLİM-vazgeçişi
  DEĞİL — mesaj kuyruktan yalnız deadline'la düşer. Tek-yol (alternatif hub yok) → geometrik kuyruk düşük
  tempoda sürer, TAM SESSİZLİK YASAK (blackout-survival). Failover sorumluluğu RETRY_BUDGET'tan buraya taşındı.
```

---

## 13. Pacing / Token Bucket (NORMATİF)

```
pace_rate = send_rate_final (§8.5)     # bytes/sec
Token bucket:
  refill: her Δt'de tokens += pace_rate × Δt
  cap:    tokens ≤ BURST_CAP           BURST_CAP=[CAL] (≈ 1-2 × MTU; micro-burst sınırı)
  gönder: paket ancak tokens ≥ pkt.size ise; değilse pacing kuyruğunda bekle (Tokio timer)
inter-packet gap = pkt.size / pace_rate    (düzgün pacing; ACK-clock değil, rate-clock — BBR pacing)
priority: retransmit(deadline) > FEC parity > yeni DATA > ACK-only (ACK-only ayrı küçük budget)
[v1.1 · K4G-05b, NORMATİF — sahip kararları CEVAP PAKETİ 5, 2026-10-16; CEVAP PAKETİ 7, 2026-10-18 (DUR-9 (a));
  CEVAP PAKETİ 7-EK, 2026-10-18 (B1); CEVAP PAKETİ 9, 2026-10-24; CEVAP PAKETİ 10, 11, 11-EK, 12 ve 13, 2026-10-24
  (DUR-24, DUR-31); CEVAP PAKETİ 15 v3 ve 15 EK-1, 2026-10-25 (DUR-38: B′, K1–K3; DUR-40) —
  `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]
  KOPYA KUYRUĞU İÇİ SIRA (PRE_CONVERGE DIŞINDA): vadesi gelmiş kopya kayıtlarından EN ESKİ ONAYSIZ birimin kaydı
  önce gönderilir (ölçüt: en küçük birim kimliği — ürün yolunda kimlik oturum içi gönderim sırasıdır; eşitlikte
  erken vade, sonra kuyruk sırası); YAŞ TAVANI (DUR-40) — vadesinden bu yana en az RTO_MAX geçmiş kaydı olan birim
  varsa sıranın başı, bu birimler arasında son hizmeti (birimin herhangi bir sınıftan son iletimi) EN ESKİ olan
  birimin en erken vadeli kaydıdır (eşitlikte küçük kimlik); bir birim bir planlama turunda — hangi sınıftan olursa
  olsun — en çok bir kez gönderilir; token yetmezse o tur DURUR — arkadaki kayıt sıranın başını GEÇEMEZ, istisnasız.
  Teyit edilmiş birimin kaydı gönderilmez. Bir birimin bekleyen tek seferlik kopya kaydı ≤ D − 1 tutulur (ekleme
  anında sayılır, fazlası eklenmez); geometrik kaydı varken yeni kayıt açılmaz, yalnız onun vadesi öne çekilir
  (öne çekme sıklığı: zincir dönemi başına en çok bir kez — transport-layer-tdd §12 [AMENDMENT-12]).
  BURST_CAP TUR BAŞI TABANI (PRE_CONVERGE DIŞINDA): her planlama turunun başında, İLK kırpmadan önce, BURST_CAP alt
  sınırı = max(kuyruk başındaki DATA biriminin boyutu — kuyruk boşsa DEFAULT_MTU, v1 aynen —, ARQ planının ilk
  kaydının boyutu, bekleyen kuyruk-probu hedefinin boyutu, yukarıdaki ölçüte göre ilk vadesi gelmiş kopya kaydının
  boyutu); üst sınır en az alt sınırdır; aynı turda oluşan kayıt sonraki turun tabanına girer. Token ÜRETİLMEZ;
  token tavana dek birikebilir; pace_rate DEĞİŞMEZ (INV-RATE-1). Yol seçimi ve bir sonraki uyanma aynı başları
  kullanır. PRE_CONVERGE'de taban v1'dir (max(kuyruk başındaki DATA biriminin boyutu, ilk-uçuş bütçesi)): ilk-uçuş
  bütçesi kaydın boyutundan küçükken BURST_CAP'i aşan kayıt bu kayıtla kapsanmaz (beyanlı istisna,
  transport-layer-tdd §12 [AMENDMENT-12]); çıkış turu tek turdur. Geometrik aralık ×2, üst sınır RTO_MAX
  (transport-layer-tdd §12 [AMENDMENT-12]). PRE_CONVERGE'de §3.3-B sırası ve takvimi DEĞİŞMEZ; PRE_CONVERGE
  çıkışında bekleyen kopyaların aralığı ve vadesi RTO_MAX'a kırpılır. SINIFLAR ARASI SIRA ~~tek istisnayla~~
  istisna ve karşılık kuralıyla DEĞİŞMEDİ: v1 tahkimi kod sırasıdır (DATA → ARQ → kuyruk-probu → kuruluş probu →
  kopya kuyruğu → FEC parity; RECOVER_ARQ'da her K_ARQ_SHARE'inci tur ARQ ile başlar). İSTİSNA — PRE_CONVERGE hariç
  bütün durumlarda (PRE_CONVERGE'de v1 sırası ve §3.3-B takvimi değişmez; hak ve sınıf rezervi yoktur; bu kapsam
  rev4 notunun "PRE_CONVERGE dahil" önerisini DEĞİŞTİRİR; CEVAP PAKETİ 10 (2026-10-24) "her durumda" ifadesi CEVAP
  PAKETİ 11-EK (2026-10-24) §6 ile geri alındı): UYGUN İŞ (tur başında) — DATA: kuyrukta birim; ARQ: RECOVER_ARQ'da
  vadesi gelmiş, tavan altı kayıp kaydı; kopya: vadesi gelmiş, teyitsiz, o turda gönderilmemiş kopya kaydı (tur başında; bir emisyondan
  sonraki KARŞILIK sınamasında tur içi birim dışlaması uygulanmaz: birimi bu turda başka sınıftan giden kaydın
  sınıfı da hak ve rezerv alır, kayıt bir sonraki turda gider); token
  yetersizliği uygun işi kaldırmaz, vadesi gelmemiş iş uygun iş değildir. (i) SEÇİLME — karşılık hakkı yokken (ve
  aynı turda ARQ payı rezerv almamışken) gönderilmeye uygun (vadesi gelmiş) kopya kuyruğu başı en geç
  K_ARQ_SHARE'inci `schedule_transmit` çağrısında DATA'dan ÖNCE hizmet hakkı kazanır; aynı K_ARQ_SHARE; kopya payı
  RECOVER_ARQ'nun ARQ payı sayacından bağımsız sayılır, yeni sayı ve ayrı token kovası YOK; aynı turda ARQ payı da
  vadeliyse ARQ önce, sonra kopya başı. ARQ PAYI REZERVİ (CEVAP PAKETİ 15 v3 K3) — ARQ payı turunda ARQ planının başı
  token'a yetişemezse ve rezerv yoksa REZERV ARQ başı için açılır. REZERVASYON — hak kazanan baş token'a
  yetişemezse REZERV açılır; rezerv tek ve sahiplidir (kopya kuyruğu başı, ARQ planı başı ya da karşılık hakkındaki
  DATA başı): sahibin başı dışındaki her token tüketicisi (DATA, ARQ, kopya kuyruğu — drenaj dahil —, kuyruk-probu,
  FEC parity, kuruluş probu) rezervin altına inemez, rezervin ÜSTÜNDEKİ token'ı kullanabilir; rezerv ≤ bir kayıt boyu,
  aynı anda tek rezerv; token ÜRETİLMEZ; rezerv sahibin başının gönderiminde, birim teyit edilince, hedef ya da
  sahibin uygun işi geçersizleşince ve oturum kapanınca temizlenir; baş değişirse yeni başın boyuna güncellenir;
  artık rezerv testle sınanır. KARŞILIK — bir kopya ya da ARQ emisyonundan sonra (her yoldan) o anda uygun işi olan
  diğer sınıfların her biri bir karşılık hakkı alır; sınıf başına en çok bir hak; DATA emisyonu hak üretmez; hak,
  sınıfın kendi emisyonuyla ya da uygun işi bitince düşer, başka düşme yolu yoktur; hak sahibi varken ve rezerv yokken rezerv ARQ >
  DATA > kopya sırasıyla ilk hak sahibinin başı için kurulur ve kopya payı rezerv kurmaz. (ii) GÖNDERİLME — rezervi
  alan baş ≤ boy / pace_rate + bir tur içinde çıkar: sabit pace_rate, çalışan zamanlayıcı ve yeterli BURST kapasitesi
  koşuluyla; değişken hızda biriken token üzerinden hesaplanır (test iki kipte). ~~(ii) dışında gecikme sınırı
  VERİLMEZ.~~ (ii) ve (iii) dışında gecikme sınırı VERİLMEZ. (iii) HİZMET GARANTİSİ — PRE_CONVERGE dışında, uygun işi
  olan her sınıf (DATA, ARQ, kopya) ve ~~kopya sınıfı içinde her teyitsiz birim,~~ her iletilmiş teyitsiz birim (kopya kuyruğuna alınmadan önceki bekleme dahil; DUR-45 (C), sahip kararı bekliyor), teyitten bağımsız olarak, en geç
  ~~[kendi başının ve kendisinden önce rezerv alan hak sahiplerinin baş boyları toplamı] / pace_rate + K_ARQ_SHARE tur
  (birim için: T_birim)~~ [kendisinden önce rezerv alan her sahibin başının (aynı sınıf birden çok kez sayılır) ve kendi
  başının boyları toplamı + P_pf] / pace_rate + (K_ARQ_SHARE + aşama sayısı) tur (DUR-43) içinde bir gönderim fırsatı
  alır — aşama = rezervi alan bir sahibin emisyonu, aşama sayısı ≤ 4; en kötü diziler (S_c, S_a, S_d kopya, ARQ,
  DATA baş boyu üst sınırı): DATA ← kopya, ARQ, DATA (S_c + S_a + S_d; 3 aşama); ARQ ← DATA, kopya, ARQ (S_d + S_c
  + S_a; 3 aşama); kopya ← DATA, ARQ, DATA, kopya (2·S_d + S_a + S_c; 4 aşama); P_pf = beklemede rezerv yokken ya da
  rezervin üstünde token alan sınırlı tüketicilerin (kuyruk-probu, FEC parity, kuruluş/CS probu) baytı (tasarım notu
  B.6 h.2); tur = `schedule_transmit` çağrısı. Birim için T_birim = max(H_maks, RTO_MAX) + RTO_MAX + N_live ·
  T_max(kopya) + bir tur; N_live = iletilmiş, teyitsiz birim sayısı (kabul edilmiş, en az bir kez iletilmiş, teyitsiz
  birimlerin sonlu kümesi); N_live beklemenin başında — birimin son hizmeti anında — sayılır, açık üreticiyle oturum süresiyle büyüyebilir (DUR-45 (2′), sahip kararı bekliyor); H_maks = sahipsizlik süresi H'nin (transport-layer-tdd §12 [AMENDMENT-12]) bekleme
  boyunca en büyük değeri; ~~H'nin çapası son ilerlemedir:~~ H'nin çapası max(son ilerleme, birimin son iletimi)'dir: ~~birimden~~ birimin son iletiminden ÖNCE gönderilmiş bir kaydın ilk onayı çapayı yeniler ve T_birim'e en çok bir H_maks ekler — sayısı beklemenin başında onaylanmamış eski kayıtlarla sınırlıdır; bu ilerleme RTT örneği verirse H_maks beklemede kabul edilen en büyük RTT örneğiyle sınırlıdır —, ~~birimden~~ birimin son iletiminden SONRA gönderilmiş kaydın onayı birimi kayıp ilanıyla ~~ARQ yoluna alır~~ ARQ ya da kopya yoluna (RECOVER_FWD'de kopya dönüşümüyle) alır ~~(DUR-45 (C));~~ (DUR-45 (C), sahip kararı bekliyor); T_max(kopya) = kopya sınıfının yukarıdaki sınırı; sabit pace_rate ≥ taban, çalışan zamanlayıcı ve yeterli
  BURST kapasitesi koşuluyla; değişken hızda biriken token üzerinden. Sabit bir saniye değeri vaat edilmez; fikstür
  değerleri (31 B/s, 100 B/s) notta türetilir ve testte üst sınır olarak doğrulanır. Yukarıdaki priority satırıyla
  ayrışması bu kaydın kapsamı dışındadır.
```

---

## 14. Inflight Cap / Memory Cap (NORMATİF)

```
inflight_bytes ≤ cwnd_bytes (§8.2) — congestion/hafıza koruması.
Buffer pool (bounded — SR-AUDIT-001):
  tx_unacked_pool:   ≤ TX_POOL_MAX bytes (retransmit için tutulan);  aşılırsa app back-pressure.   [CAL]
  rx_reorder_pool:   ≤ RX_POOL_MAX;   fazla out-of-order → en eski drop (SACK ile kurtarılır).      [CAL]
  fec_shard_pool:    blok başına ≤ (k+r) shard; blok tamamlanınca/timeout'ta serbest.
  MAX_SKIP (E2E ratchet skipped-key, TDD): ayrı sınır; bounded.
Tüm pool'lar sabit-üst-sınır (unbounded alloc YOK; PARSER-02 bounds-before-alloc).
```

---

## 15. Fuzzy Membership Functions (NORMATİF — DETERMINISTIK, ADR-17)

```
Girdiler (§5.1 TDD): loss_rate, jitter, bandwidth, queue_delay, ack_confidence
Her biri 3 membership: LOW / MED / HIGH — trapezoidal (deterministik, süreklilik → cliff/osilasyon yok).

Örnek (loss_rate; eşikler [CAL: netem baseline]):
  μ_LOW(x)  = trap(x; 0, 0, a1, a2)
  μ_MED(x)  = trap(x; a1, a2, b1, b2)
  μ_HIGH(x) = trap(x; b1, b2, ∞, ∞)
  (a1,a2,b1,b2 netem'de kalibre; UYDURULMAZ.)

Defuzzification: kurallar (§8.4) → ağırlıklı-ortalama (centroid) → fec_parity_ratio, arq_aggressiveness,
                 send_rate_gain, compress (kalıntı, tüketilmez — AMENDMENT-4).

DETERMINIZM (ADR-17, ZORUNLU):
  aynı input snapshot (loss,jitter,bw,qdelay,ackconf) → aynı output. RNG YOK.
  golden test vector seti (§16) her build'de doğrulanır; sapma = build fail.
```

---

## 16. Deterministic Test Vectors (NORMATİF — conformance)

```
Amaç: engine'in her deterministik parçasını golden vektörle kilitle (implementer drift + regresyon).

VEKTÖR SETLERİ:
  TV-ACK:    (rx_bitmap) → beklenen ACK_FRAME byte'ları (fixed-width, §2). Wrap senaryoları dahil.
  TV-SEQ:    (expected, wire_seq) → çözülen full_counter (§2 wrap decode).
  TV-RTT:    (rtt_sample dizisi) → beklenen srtt/rttvar/RTprop/queue_delay (§6).
  TV-BW:     (send/ack olay dizisi + app_limited/compression bayrakları) → beklenen bw_sample geçerliliği +
             BtlBw (§7). "ana yetenek" kanıtı: loss enjekte edilir, BtlBw DEĞİŞMEZ (bandwidth loss'tan bağımsız).
  TV-LOSS:   (ack dizisi) → beklenen lost set (packet/time threshold, §5) + loss_pre_fec/residual.
  TV-FEC:    (block, k, r, silinen shard'lar) → reconstruct doğru; tampered shard → decoder RED (INV-FEC-1).
  TV-FUZZY:  (input snapshot) → beklenen (fec_ratio, arq_aggr, send_rate, compress) (ADR-17 determinizm). [AMENDMENT-4: `compress` golden vektörde KALIR — kod alanı üretiyor; kaldırılması v1.x, EVOLUTION §9.8]
  TV-EMERGENCY: (health trace) → beklenen level geçişleri (§11 hysteresis): Schmitt bant-içi gürültüde
             SIFIR flip; escalate ANINDA (dwell'siz); de-escalate geçişleri ≥ min_dwell aralıklı;
             framing-ayrımlı RecoveryAction (Tiny→Duplication D×+spacing, SegmentBlock→MinimalParity).
  TV-SRC:    (senaryo trace) → beklenen send_rate/cwnd trajektorisi (loss send_rate'i düşürMEDİĞİ dahil).
  TV-COLDSTART: [K5/K6/R1/R8/R14/revize-D0 §7] PRE_CONVERGE (§3.3) conformance:
    -1 (D2): temiz 1 kbps → 10/10 teslim; tx_wire ≤ 1.15·altsınır (altsınır=N·(50+payload)); worst-phase:
             her birim boyutunda (64B@floor dahil) ilk SACK k_base·RTT_floor'dan ÖNCE → spurious ilk-kopya YOK;
             [R14] ilk gerçek SACK öncesi state≠RECOVER_FWD ∧ emergency=OFF ∧ decimation=OFF;
             [R8] hayatta kalan min_rate=80kbps floor'u bu çift kapı (tx_wire≤1.15× + 10/10) mekanik yakalar.
    -2 (D3): %95 loss'ta tek SACK'ten RTT + yön-ayrımlı loss (seen_count); bw kilidi AÇILMAZ (§3.3-D).
    -3 (D4): hızlı kanal (100kbit+%90 loss) ilk saniyelerde yakalanır (probe hızlandırma).
    -4 (D4): 0.25 kbps'te probe bütçesi Z·forward_bw'yi AŞMAZ; amp≤1; geçersiz-probe'a sessizlik.
    -5 (D5): echo'suz koşuda motor asla tam sessizliğe düşmez; [K6] "tek-yol + echo'suz" (failover yok) varyantı.

Format: ccr §30 tarzı gerçek byte / sayısal dosyalar; Rust referans ≡ değer.
Tunable'lar [CAL] → baseline ölçümünden sonra vektörlere GERÇEK değerle işlenir (placeholder değil).
```

### 16.1 PRE_CONVERGE [CAL] sabitleri — türetme + kalibrasyon-domaini (NORMATİF) [K5/revize-D0 §5]

> Her sabit [CAL] (formül+domain) ya da [STD] (kaynak). Sayılar PLACEHOLDER — netem lowband baseline'ından
> kalibre; "tüm bw" satırlarının 4-decade geçerliliği ayrıca doğrulanır (0.25k/1k/100k/1M). Fizik: TDD §13
> SESSION_DATA=50B+payload. 1 kbps=125 B/s; 0.25 kbps=31.25 B/s.

| sabit | formül (türetme) | kalibrasyon-domaini | PLACEHOLDER |
|---|---|---|---|
| `PRE_CONVERGE_FLOOR_BW` | ürün floor; operatör `bandwidth_class` yalnız önseli ezer | önsel, tüm bw | 31.25 B/s |
| cold-start pacing floor [R8] | = PRE_CONVERGE_FLOOR_BW; `min_rate_bps`=10000 PRE_CONVERGE'de MUST-NOT-APPLY (Ek-B) | PRE_CONVERGE | 31.25 B/s |
| `min_rate_bps` [R2-ADIM6] | kanal-hayatta-kalma floor = `PRE_CONVERGE_FLOOR_BW`, TÜM state; rate→0 önle ∧ overdrive-yok. **CANCELLED:** 10000 (80 kbps) mutlak | tüm bw (4-decade) | 31 B/s |
| `BURST_WINDOW` [R3-ADIM6] | burst_cap = clamp(rate·BURST_WINDOW, 1 birim, ~2×MTU); bant-kör 2×MTU dump CANCELLED | tüm bw (4-decade) | 40 ms |
| refill precision [R1-ADIM6] | sub-byte kesir-kalan taşınır; per-tick tamsayı-truncation (rate<1000 B/s'te stall) YASAK | tüm bw | ns-ölçek integer |
| `sack_wire` [R15] | 28(UDP/IPv4)+22(handle4+seq2+tag16)+ACK_FRAME_V2 gövde(15+4·range), 0 range | tüm bw | 65B |
| `ACK_DELAY_UNIT` [TUR-1x] | V2 ack_delay birimi; tavan 65535×birim ≈ 16.78s > ACK_CADENCE floor 8.39s (satürasyon beklenmez, sayaçla denetlenir) | tüm bw (4-decade: floor-encoding hızlı kanalda ≤256µs yuvarlama = ihmal) | 256 µs [STD-karar D-4/1] |
| min-rtt guard [TUR-1x] | `raw−ack_delay ≥ rtprop` değilse düzeltme REDDedilir, RAW işlenir (örnek atlanmaz; §6/§7.3, RFC 9002 §5.3). **CANCELLED:** mutlak `MIN_RTT_FLOOR=1ms` — loopback sub-ms gerçek RTT'de tüm örnekleri reddedip estimator açlığı yaptı (ZM-2 ihlali, bench-loopback ölçümü) | tüm bw (4-decade: gözlenen-min kanal-türevi) | — (sabitsiz) |
| `K_c` cadence-eşik çarpanı [TUR-1b] | loss time-threshold tabanı = K_c×EAI (§5b); EAI = ACK_FREQ_N×ort_birim/send_rate + sack_wire/return_bw_önseli(31 B/s [CAL], §3.3-E-FULL ölçümle değişir) | tüm bw (EAI kanal-türevi; önsel-kolu worst-corner sabit — hızlı kanalda eşiği ~3.1s'e taşır, deadline≫ olduğundan kabul; ölçülen return_bw gelince daralır) | 1.5 [CAL matris {1.25, 1.5, 2.0}] |
| `K_wl` W_loss pencere çarpanı [TUR-2a] | W_loss = K_wl×ack_epoch, ack_epoch = max(srtt, EAI) (§5 ledger; D-1/5) | tüm bw (kanal-türevi) | 8 [CAL, K_bw/K_rtprop merkeziyle hizalı] |
| ledger confirm-ufku [TUR-2a] | Suspected→Confirmed = cadence-taban (K_c×EAI) kanıtsız geçince — geç-SACK'in makul varış penceresi; saat-sınıfı = eşikle AYNI (ZM-4) | tüm bw | = K_c×EAI (ayrı sabit YOK) |
| `LEDGER_MAX` [TUR-2a] | olay-ledger + pencere-paydası kayıt tavanı; en-eski-önce eviction + audit sayacı (SR-AUDIT-001; tombstone MAX ailesi) | tüm bw (bellek-tavanı sabit) | 4096 |
| `K_TOMB` tombstone TTL çarpanı [TUR-2b] | TTL = K_tomb×T_loss(ilan-anı); T_loss = max(9/8·srtt, cadence-taban) — eşikle AYNI saat (ZM-4). Onaylı formülün `min(mesaj-deadline,…)` kolu motorda UYGULANAMAZ (per-unit deadline engine'e taşınmıyor; bench/app-katmanı) → TTL tek-kollu ZEYL | tüm bw (kanal-türevi) | 8 [CAL] |
| `TOMBSTONE_MAX` [TUR-2b] | geç-ACK eşleştirme kayıt tavanı; en-eski-önce eviction + `tombstone_evicted` audit (evict edilenin geç-ACK'i Expired/Unknown — örneksiz) | tüm bw (4-decade doluş: 0.25k≈11, 1M-heavy≈450 ≪ 4096; bellek ≤~350KiB) | 4096 [CAL] |
| `K_bw` / `K_rtprop` [TUR-3a] | W_BTLBW = max(10s, K_bw×ack_epoch); W_RTPROP = max(10s, K_rtprop×corrected_srtt); ack_epoch = max(srtt, EAI); + KEEP-LAST (D-2/2) | tüm bw (kanal-türevi; 10s [STD] taban hızlı kanalda korunur) | 8 / 8 [CAL matris {6,8,10}] |
| `K_QD` qd kısa-penceresi [TUR-3a.5-α] | W_qd = K_QD×ack_epoch; W_qd ≤ W_rtprop; keep-last; [STD]-taban YOK (bilinçli — srtt-clock-v1 §2.2) | tüm bw (kanal-türevi) | 2 [CAL matris {1,2,4}] |
| `K_EPISODE` epizot-dedektör çarpanı [TUR-5 r3] | robust-baseline (0→oturum-ilk-özel; <k→max; ≥k→median(son k, k [CAL {3,5}])) + `current_gap > K_EPISODE×baseline` (KESİN >, pinli); SİMETRİK CLAMP'Lİ-YAZIM: pencere-yazımı clamp(gap, b/K, K×b) — zehirlenme-koruması + livelock-ölümü + mikro-gap tabanı; tetikleyen-gap dahil her gap yazılır; değerlendirme güncelleme-ÖNCESİ | tüm bw (BOYUTSUZ kanal-türevli oran — ZM-yasağı mutlak-ZAMAN sabitleri, boyutsuz çarpan izinli) | 4 [CAL {3,4}] |
| `R` epizot-kotası [TUR-5 r3] | epizot-başı TOPLAM emisyon-kredisi (muaf-ilk dahil); T_SPACING-takvimli dekorele kopyalar (aynı kümülatif-SACK); her emisyon-yolu (freq/timer/tail) kredi düşürür; `quota_emits ≤ R×episode_count` SERT-INVARIANT; kopyalar yeni-varış BEKLEMEZ (retry-semantiği) | tüm bw (sayaç — saat değil) | **4 [CAL {4,8} — R=8 ELENDİ: 0.25k dar-dönüş kota-doyumu 13/20, §0.24]** |
| `T_SPACING` kota-aralığı [TUR-5 r3] | SACK_WIRE/RETURN_BW_PRIOR ≈ 2.1s — **STATİK-ÖNSEL-TÜREVİ** (dürüst-etiket; E-FULL'de ölçülen return_bw'ye bağlanır — statik-prior borcunun yeni-tüketicisi). Varış-türevli yapılamaz: receiver p5-açlığı(throttle) ile 0.25k'yı(serialization) varış-ritminden AYIRT EDEMEZ (sensing-ön-şartı, §0.24-karar-2) | tüm bw | 65/31 ≈ 2.1s |
| `K_ARR` minimal-taban çarpanı [TUR-5 r3] | §3.3-E donuk-cümle iner: cadence_eff = clamp(K_ARR×last_gap, T_SPACING, FLOOR), yalnız beklemede-yeni-bilgi varken; referans = last_emit_any (tail-dahil birleşik-saat). Yapısal-gölgeleme beyanı: K_ARR=2'de accel-ritmi freq/tail'le örtüşük — ayrık-katkı dar-pencerede (ölçüldü 0-4/koşu) | tüm bw (kanal-türevi) | 2 [CAL {2,4}; {1} elendi: paket-başı-ACK] |
| `base_phase` recovery-overlay [TUR-7 β1] | recovery-durumları pacing-fazı DEĞİL (INV-RATE-1 ek-cümlesi): gain = base∈{Startup,Steady}; çıkış base'e; plato base-üstünde (recovery-altında round'lar ilerler); recovery-dönüşü ≠ re-entry (reset yalnız taze-girişte). Kilit-golden: inv_rate_1_loss_declaration_preserves_phase_gain | tüm bw (saat YOK — gain-seçimi) | — (sabitsiz) |
| `K_ARQ_SHARE` ARQ token-payı [TUR-7 β2] | pending>0 ∧ RecoverArq iken her K'inci schedule-grant retx-ÖNCE (BOŞ-NÖBET biter: DATA-önce + tek-birim-cap ARQ-payını yapısal-0 yapıyordu — 120s retx=0 ölçüsü); DATA-önce ölçülü-kararı diğer grant'lerde korunur | tüm bw (sayaç — saat YOK) | 4 [CAL {4,8}] |
| ~~`K_SHRINK`~~ **[ARŞİV — MUST NOT IMPLEMENT: TUR-D1-shrink; Ş4-kanıt-zinciri mini-D0; kod turd1-reconciliation-branch-DO-NOT-MERGE]** shrink-ateşleme [TUR-D1] | K ardışık-eligible-evidence-OLAYI (LOW_EVIDENCE; SKIP-köprülü) → stale-btlbw demotion. Rol: persistent-underdelivery reconciliation — "kesin-daralma-tespiti" DEĞİL (m45 rol-daraltması). Atomik-event-verdict (HIGH-1) + generation-izolasyonu (HIGH-2) + olay-aralığı ≥ ack_epoch (P6) | olay-sayacı (saat YOK) | 3 [CAL {3,5}] |
| ~~`K_RATIO`~~ **[ARŞİV — MUST NOT IMPLEMENT: TUR-D1-shrink; Ş4-kanıt-zinciri mini-D0; kod turd1-reconciliation-branch-DO-NOT-MERGE]** underdelivery-oranı [TUR-D1] | eligible-örnek < B_ref/K → LOW-kanıt; ≥ → COUNTER_EVIDENCE (yüksek-örnek-dominance). Boyutsuz bw-oranı | bw-domain (saatsiz) | 2 [CAL {2,4}] |
| `W_BTLBW` delivered-EAI-türetimi [Y3, K6R-mühürlü] | Pencere-saati srtt-DEĞİL delivered-cadence: w_epoch = 2×unit/last_full_bw + return-prior(2.097s [CAL, E-FULL-borcu]); W = K_BW×w_epoch (10s-taban korunur). Kök: MAX-filtre+kuyrukla-büyüyen-saat = pozitif-geri-besleme → bayat-max-ölümsüz (DV-3 DESEN-1, srtt-eğim 0.92 s/s > 1/K_BW). Çok-adımlı-expiry; kopmasız-daralmada kanal-bandına-yakınsar (netem: 4114→308); kopma-frozen-pencere sınır-vakası BEYANLI (TUR-S-tamamlar). last_full_bw K3-pinleri: yalnız-tam-geçerli-örnek (!app_limited), zero→son-geçerli-tekrar | delivered-cadence (kuyruk-değişmez) | UNIT_SIZE_HINT=267B [CAL] |
| ~~`K_FRESH`~~ **[ARŞİV — MUST NOT IMPLEMENT: TUR-D1-shrink; Ş4-kanıt-zinciri mini-D0; kod turd1-reconciliation-branch-DO-NOT-MERGE]** taze-snapshot kapısı [TUR-D1 P1] | ack_elapsed ≤ K×olay-başı-ack_epoch; aşan örnek = kopma-yayılımlı → daralma-kanıtı DEĞİL (TUR-S-teşhis-sayacına). FAIL-OPEN (şişkin-epoch gevşetir; ana-koruma verdict+olay-başına-max-1) | ack_epoch-oranı | 2 [CAL {2,4}] |
| **[TUR-D1 EK-3 AÇIK-BORÇ]** W_BTLBW daralma-anti-fix'i | W=K_BW×ack_epoch'un srtt-dalı daralmada cutoff'u GERİLETİR (eğim>1/K_BW → bayat-max ölümsüz; DV-3 ölçülü 0.92 s/s). Shrink-reset SEMPTOMU keser, KÖK DURUR → dedektör-reset'e kalıcı-bağımlılık-riski; W-türetim/M3-decoupling turunda kapanır. "Dedektör-var-diye-W-fix-gereksiz" SANILMAZ | — | BORÇ (S1-ertelendi) |
| `K_CONV` kuantum-döngü böleni [TUR-3a.5-β] | T = rtprop/K_CONV; n = clamp(Δt/T, 1, CAP=64); floor-1 hızlı-decade bit-özdeş; kalan düşer (muhafazakâr) | tüm bw (kanal-zamanlı; ADR-18 4-decade kapısında kalibre) | 4 [CAL matris {2,4,8}] |
| `K_PROBE_PERIOD_EPOCH` [TUR-3c] | ProbeUp giriş-periyodu: now−last_probe_end ≥ K×ack_epoch(**CANLI** — Steady'de gain=1 → epoch kirlenmez); giriş ayrıca qd_ratio≤500‰ histerezis ∧ backlog; tazelik-kapısı BİLİNÇLİ YOK (rtprop min-örnek yaşı sürekli-akışta bile büyür → kapı probu yapısal bloklar, testle ölçüldü; DrainRtprop yalnız Steady'den girer → probe-dwell'i kadar bekler, sınırlı) | tüm bw (kanal-türevi) | 4 [CAL matris {2,4,8}] |
| ProbeUp volume/dwell/eşikler [TUR-3c] | volume = btlbw_e×PROBE_VOLUME_EPOCH×epoch_e; max_dwell = PROBE_MAX_DWELL_EPOCH×epoch_e — İKİSİ DE **DONUK** giriş-snapshot'ından (öz-referans tuzağı: probe gain'i canlı-epoch'u ~%20 küçültür, kuyruk-srtt dwell'i UZATIR = pozitif geri-besleme); success: delivered≥volume ∧ btlbw>btlbw_e×(1+ε); abort: qd>500‰ ∨ prob-atıflı kayıp (send_ns≥entry_ns — giriş-öncesi Confirmed-lag/rastgele-kayıp vetosu) | tüm bw (kanal-türevi) | volume=2×, dwell=4×, ε=25‰, qd=500‰ [CAL — placeholder, golden'a GİRMEZ] |
| `RTT_floor(unit)` | (unit_wire+sack_wire)/floor_bw + PROP_MARGIN | pace edilen taban | 256B@0.25k≈11.9s; 64B@1k≈1.43s |
| `k_base` (COPY_BASE_FACTOR) [R1] | >1; ilk-SACK cadence-exempt → RTT_floor ilk-SACK modeli. Fallback (exempt değilse): ≥1+ACK_CADENCE/RTT_floor | tüm bw | 1.3-1.5 |
| `COPY_MULTIPLIER` | geometrik backoff (RFC 6298 analoğu) | tüm bw | 2 [STD] |
| `COPY_MAX_INTERVAL` | deadline/MIN_COPIES_IN_DEADLINE | deadline-türevi | deadline/8 |
| `T_PATH_DEAD` [R4] | geometrik-span < T < deadline−failover_marjı; IDLE_TIMEOUT'tan AYRI; silence=now−max(last_ack,first_send) | tüm bw | — |
| `RETURN_BUDGET_FRACTION` (Y) | SACK+echo dönüş ≤ Y·return_bw | tüm bw | 0.20-0.30 |
| `ACK_CADENCE(return_bw)` [R15] | sack_wire/(Y·return_bw); ilk SACK cadence-exempt | tüm bw | 0.25k≈8.3s; 1k≈2.1s; fast~20ms |
| `PROBE_BUDGET_FRACTION` (Z) | probe forward ≤ Z·forward_bw | tüm bw | 0.10-0.20 |
| `PROBE_PAYLOAD_SIZE` (probe_wire) | probe_baş+pad, pad=echo_gövde−probe_baş+margin → amp≤1 (AEAD içinde) | tüm bw | ~50B wire (echo 32B'yi domine) |
| `MINI_BURST_COUNT` [R-mb] | min(TARGET, floor(Z·forward_bw·1s/probe_wire)) | tüm bw | fast:3-5; 0.25k:0 (amortize ~10-20s) |
| `PROBE_RATE_LIMIT` | responder kaynak-başına echo/s (DoS taban) | responder | — |

### 16.2 Zaman-modeli haritası — saat-sınıfı envanteri (NORMATİF) [TUR-A]

> **Neden var:** Motor 50-200 kbit'te yazıldı; orada pace-tick ≈ RTT-round (ikisi ms mertebesi) →
> saat-sınıfı karışıklıkları hiç patlamadı. Sub-kbps'te srtt 3-12 s, ACK cadence 2.1-8.4 s, birim
> serialization 2.3-9.1 s iken tick 1 ms — sınıflar 1000× ayrışır ve her gizli karışıklık ayrı patlar.
> Sınıfın ölçülmüş örnekleri: (1) INITIAL_RTT wall-clock [D2], (2) server ACK_TICK=20ms SACK-seli [D2],
> (3) startup_plateau round'u pace-tick sayması → STARTUP 3 ms'de biter, motor app-limited hızına
> kilitlenir [TUR-A trajektori], (4) W_BTLBW=10s wall-clock ("~10 RTT" yorumuna rağmen) → srtt=3s'te
> ~3 RTT → btlbw 88→4 çöküşü [TUR-A ölçüm]. Envanter: 94 mekanizma tarandı (74 + 20 verify-avcısı eki):
> **22 BREAKS / 35 SUSPECT / 17 SAFE**.

**Normatif kurallar (yeni kod dahil, İHLAL EDİLEMEZ):**
```
ZM-1  Olay-sayısı (on_event/tick çağrı adedi) ASLA zaman-vekili olamaz. "Round" = RTT-round
      (BBR delivered-anchoring) ya da açık bw-örneği; tick-frekansı değişince semantik DEĞİŞMEMELİ.
ZM-2  Wall-clock mutlak sabit ancak 4-decade (0.25k/1k/100k/1M) doğrulamasıyla girer; RTT/kanal-zamanı
      ölçen her eşik srtt/rtprop/serialization-türevi olmak ZORUNDA (ms sabiti değil).
ZM-3  Zaman ölçen her YENİ mekanizma bu tabloya saat-sınıfıyla eklenir; sınıfsız eşik CI-red gerekçesidir.
ZM-4  Karşılaştırmanın İKİ YANI aynı saati kullanmalı (örn. loss-threshold srtt-türevi iken elapsed'in
      içindeki receiver ACK-hold'u threshold'un saati GÖRMÜYORSA karşılaştırma geçersiz — aşağıda K-1).
```

> **ZM-3-KAYIT [DR-CS-1b-A2-F0.4; Abkarada-talimat-onaylı]:** A2 ürün-alıcı zaman-mekanizmaları bu
> envantere şu saat-sınıfıyla girer: **receiver-monotonic (CLOCK_MONOTONIC; kompozisyon-kökü
> `ReceiverHost`'ta örneklenir; `kama-engine::ColdRxState` saat TUTMAZ — `now_ns` enjekte).**
> Kapsanan eşikler: per-block patience-deadline `RX_COLD_ABANDON` (profil-türevi — ZM-2-uyumlu;
> non-renewable, first-authenticated-shard-anchor) · `SESSION_IDLE_HOLD` · `TOMBSTONE_GRACE` ·
> `T_ECHO_MIN(profile)` (üçü [CAL]-aday; wall-clock-sabit-girecekse ZM-2 4-decade doğrulaması ŞART) ·
> plateau-test-timer'ları. Karşılaştırmaların iki yanı aynı monotonic-domain (ZM-4).

> **ZM-2 AKTİF-KORUMA ÖRNEĞİ (ilk kanıt — TUR-1x, 2026-07-13; talimatla kayda geçirildi):** min-rtt
> guard'ın ilk tasarımı mutlak `MIN_RTT_FLOOR=1ms` wall-clock tabanıydı. ZM-2'nin şart koştuğu
> 4-decade doğrulaması (bench-loopback testi) tabanı ANINDA yanlışladı: loopback/LAN gerçek RTT
> sub-ms → TÜM örnekler reddedildi → estimator açlığı → PreConverge kilidi. Taban İPTAL edilip
> gözlenen-min (rtprop, kanal-türevi) guard'ıyla değiştirildi (§6, RFC 9002 §5.3). Bu, haritanın
> pasif envanter değil AKTİF koruma olduğunun ilk kanıtıdır: kural, hatayı üretim yerine testte
> yakaladı.

**Sınıf envanteri (BREAKS + kritik SUSPECT; tam liste denetim kaydında):**

| mekanizma | kod | fiili sınıf | sub-kbps davranış | gereken sınıf |
|---|---|---|---|---|
| time-threshold loss 9/8·srtt | loss.rs + sender on_timer | ~~rtt-türevi eşik ↔ ack-hold'lu wall-clock elapsed~~ → **FIXED [TUR-1a+1b]**: later-ACK kanıt-şartı + cadence-farkında taban max(9/8·srtt, K_c×EAI) | (eski) temiz kanalda SİSTEMATİK false loss (K-1, 27/27 ölçüldü) | ✓ eşik ≥ beklenen-ACK-cadence (§5b; K_c=1.5 [CAL]) |
| startup_plateau round | sender.rs | ~~event-count → bw-örneği-olayı~~ → **FIXED [TUR-6]**: DELIVERED-ANCHOR round (marker-sonrası gönderimin İLK ACK'i; ZM-1) + re-entry sayaç-sıfırlama. Arz-yoğunluğu-BAĞIMSIZ (birim-testli; TUR-5 zengin-arzı olay-sayacını kaydırıp 1k-44 regresyonu yapmıştı) | (eski) 3ms-STEADY / olay-sayımı arz-duyarlı. **YENİ-görünür (maske-soyma #6):** dürüst-round kuyruk-büyürken KENDİ-kuyruğu-üzerinden uzar (öz-referans) → Startup-çıkışı inflight-cap'e muhtaç — cap yokluğu (min_cwnd≫BDP) 1k'da eternal-Startup üretti; Little-kolu frenledi (teslim-kapısı ✓). Kök-iş: Startup cwnd-cap = MIN_CWND ailesi (D-4/2) | ✓ delivered-anchor; inflight-cap borcu AÇIK |
| W_BTLBW=10s | bw.rs | ~~wall-clock~~ → **FIXED [TUR-3a]**: max(10s, K_bw×ack_epoch) + KEEP-LAST | (eski) 0.8-3.3 RTT → btlbw 88→4 çöküşü; 0.25k tek-örnek-pencere (btlbw her örneği takip: 5 düşüş-adımı ölçüldü) | ✓ kanal-türevi (K_bw=8 [CAL]) |
| W_RTPROP=10s | rtt.rs | ~~wall-clock~~ → **FIXED [TUR-3a]**: max(10s, K_rtprop×corrected_srtt) + KEEP-LAST | (eski) 1-4 örnek; 10s+ SACK boşluğunda tek örneğe düşer → rtprop=şişkin RTT | ✓ kanal-türevi (K_rtprop=8 [CAL]) |
| **srtt-EWMA yakınsaması (α=1/8)** [ZM-estimator-katmanı] | rtt.rs observe | ~~örnek-SAYISI saati~~ → **FIXED [TUR-3a.5-β]**: kuantum-döngü n=clamp(Δt/T,1,64), T=rtprop/K_CONV(4 [CAL]) — kanal-zamanlı; floor-1 hızlı-decade bit-özdeş; rttvar eş-işlenir (ZM-4) | (eski) kuyruk boşken srtt=17.9s şişkin → gain 0.5 mıhı (42-kilidi); 9 tüketici miras alıyordu | ✓ kanal-zamanlı (öz-referans tuzağı: T=ack_epoch REDDEDİLDİ — srtt-clock-v1 §2.1) |
| **qd-payı** [TUR-3a.5-α] | rtt.rs queue_delay | kısa-pencere-min (W_qd=K_qd×ack_epoch, kanal-türevi; keep-last; [STD]-taban bilinçli yok) | qd = ayakta-kuyruk; salıverme tek-örnekte (ölçüm-α: 42→45; qd 17.3s→0.74s drain'de) | ✓ srtt'den AYRIŞTIRILDI (tek-sayı-sanma düzeltmesi; tablo srtt-clock-v1 §3) |
| ack_delay wire u16 µs (65.5ms tavan) | receiver.rs/ack.rs | ~~wall-clock tavan~~ → **FIXED [TUR-1x]**: V2 0x14 256µs-birim (tavan 16.78s), floor-encoding + satürasyon-sayacı + MIN_RTT_FLOOR guard | (eski) 8.4s ACK-hold 65.5ms'e doyardı → RTT örneği ~8.3s şişerdi; ölçüm-2: 1.43s hold decoded+subtracted, rtt_rej=0 | ✓ saniye-yetenekli encoding (D-4/1 Seçenek-A) |
| ACK_CADENCE_FLOOR (RETURN_BW_PRIOR=31 const) | receiver.rs | ~~tek şans-bağımsız arz-tabanı~~ → **[TUR-5 r3] kısmen-FIXED**: oturum-ilk/kopma-dönüşü kader-penceresi epizot-kotasıyla (R×T_SPACING-takvim) piyango-bağımsızlaştı — 18/18 koşuda ilk-teslimler takvim-noktalarında, k×8.39s-kafesi ÖLDÜ (§0.24); kalıcı-rejim cadence'i hâlâ statik-önsel (E-FULL borcu) + minimal-taban hızlanması (K_ARR) | (eski) koşu-kaderi tek 65B SACK piyangosu; 1kbps'te 4× fazla throttle | ölçülen return_bw'den adaptif (§3.3-E FULL — T_SPACING'in de bağlanacağı yer) |
| DrainRtprop dwell | sender update_state | ~~event-count(1) `_=>Steady`~~ → **FIXED [TUR-3b]**: açık kol + çıkış-invariantı (elapsed≥min_dwell ∧ qualifying-min-rtt) ∨ elapsed≥max_dwell; dwell = DRAIN_RTPROP_MIN/MAX_DWELL_EPOCH(1/3)×ack_epoch | (eski) probe 1 ms yaşar → rtprop HİÇ yenilenmez (N=0 aynı-event çıkış, VERIFIED) | ✓ kanal-türevi ölçüm-kapılı çıkış |
| **Drain (STARTUP-çıkış)** [TUR-3b] | sender update_state Drain kolu | çıkış = BYTE-eşiği (inflight ≤ cwnd_gain×BDP — saat-sınıfsız, bilinçli) + liveness-yedeği DRAIN_MAX_DWELL_EPOCH(3)×ack_epoch (ACK-açlığında sonsuz-Drain yok) | yalın-BDP sub-kbps'te TEK-pakete iner → 55s aşırı-drain ÖLÇÜLDÜ → cwnd_gain×BDP retune | ✓ byte-eşiği + kanal-türevi yedek |
| **ProbeUp periyot/dwell/volume** [TUR-3c] | sender update_state Steady/ProbeUp kolları | periyot = CANLI ack_epoch (gain=1'de güvenli); dwell+volume = **DONUK** giriş-snapshot epoch_e/btlbw_e (öz-referans tuzağı İKİNCİ red — srtt-clock-v1 §2.1 emsali: canlı-epoch probe'un kendi gain'iyle küçülür, kuyruk-srtt'yle büyür); kayıp-atfı send_ns≥entry_ns; 4 çıkış AYRI-raporlu (success/abort/timeout/preempt) | probe kendi ölçümünü kirletemez; giriş-öncesi Confirmed-lag probe'a yazılmaz | ✓ kanal-türevi (K_PROBE_PERIOD=4 [CAL]) |
| emergency escalate adımı | emergency.rs update() | event-count (çağrı başına) | 1ms pompada Arq→Dup3 2 ms'de (merdiven fiilen binary) | adım-başına RTT-türevi dwell |
| deescalate_dwell=1s | emergency.rs:95 | wall-clock | 1s < 1 srtt → tek ACK'te koruma tam iner (flap) | k·srtt veya ≥1 sağlıklı round |
| dup_spacing=20ms | emergency/sender | wall-clock | 20ms ≪ serialization → kopyalar wire'da bitişik (dekorelasyon amacı ölür) | k·birim-serialization |
| ack_confidence anchor `.or()` | sender.rs | — (semantik) | spec `max(last_ack, first_send)` DEĞİL → idle-resume'da bayat anchor → anında RecoverFwd | spec formülü + aktif-dönem re-base |
| fuzzy qd üyelikleri 50-400ms | fusion.rs | ~~wall-clock mutlak~~ → **FIXED [TUR-3a D-3/1]**: girdi qd_ratio = qd×1000/rtprop (per-mille; rtprop yoksa 0 + sayaç) — eşikler rtprop-göreli [CAL placeholder 250/500/1500/2000‰] | (eski) sub-kbps'te fiziksel-taban qd saniyeler → qd.high≡1 → gain 0.5× mıhı (ölçüldü) | ✓ rtprop-göreli (kanal-türevi) |
| fuzzy bw üyelikleri 50-200 kB/s | fusion.rs | ~~mutlak-lineer~~ → **FIXED [TUR-4]**: domain = log2_fp (ilog2·256 + 8-bit taşmasız-mantissa; x=0→0; monotonluk kanıtlı) — RE-DOMAINING ONLY: kesme-noktaları (50k/100k/150k/200k) KORUNDU ki sr1-throttle/emergency/compress tetik-alanları değişmesin (tek-değişken); decade-ayrıştırma kalibrasyonu DV-1'in işi (dv-plan-v2). Eşikler AYNI const fn'den (hardcode yasak — bit-özdeşlik) | (eski) 31-125 B/s'te bw.low≡1 + payda-0 → arq=0, pending sonsuza dek drain edilmez (ölçüldü, eski-era lifetime-lppm ile) | ✓ log-domain (4.5-decade tek ölçek) |
| **fuzzy sıfır-ağırlık fallback** [TUR-4] | fusion.rs decide() | Σw=0 → fec=fec_zero, arq=arq_light + `no_rule` bayrağı + sender `diag_fuzzy_no_rule` sayacı + TRAJ `fnr=` (fallback çıktısı R1-baskın çıktıyla bayt-özdeş — sayaçsız gözlemlenemez) | payda-0 dejenerasyonu (arq=0 → RecoverArq'ta pending drain edilmez) yapısal İMKÂNSIZ olur; NOT: arq consequent'leri ≥arq_light → arq.plan()'ın ==0 fec-dominant baskılama-dalı (§9) yapısal-erişilmez — sözleşme-notu | ✓ zamansal-eşik yok (saat-sınıfsız); membership-invariant covers_domain (KESİN >, tek-nokta deliği reddi) savunma-derinliği |
| loss_pre_fec_ppm ömür-boyu | loss.rs | ~~monoton ratchet (W_loss YOK)~~ → **FIXED [TUR-2a]**: W_loss olay-ledger'ı (`Suspected→Confirmed\|Spurious`, pencere-dışı Expired) + UN-LOSS (geç-ACK → Spurious: pending-retransmit iptal + `note_spurious` kablosu + kol-1 orijinal-RTT) + fuzzy girdisi `loss_windowed_ppm = Confirmed/pencere-gönderim` | (eski) false-loss kalıcı loss.high → fuzzy zehirlenirdi; ölçüm: p5-r4'te olaylar pencere-dışı decay (lppm→0), p5-r6'da canlı lppm=3% + ARQ tam draini | ✓ W_loss = K_wl×ack_epoch; confirm = cadence-taban (kanal-türevi, ZM-2) |
| MIN_CWND=4800B | rate.rs:93 | byte-mutlak | BDP'nin 3-48× → inflight-cap kolu hiç seçilmez | k×BDP tabanı |
| geometrik kopya deadline'sızlığı | sender dup_queue | deadline-türevi AMA tavansız | ölü kanalda sonsuz reschedule (COPY_MAX_INTERVAL/RETRY_BUDGET kodda yok) | §12 deadline tavanı DupRec'e |
| [v1.1] kopya kuyruğu aralık tavanı + sahipsizlik süresi + parite kaydı süre dolumu + RTT tazelik ölçütü | sender dup_queue / adopt_unowned / parite kaydı / RTT kabulü | RTO_MAX = 30 s wall-clock TAVAN (taban srtt-türevi); H = max(3·srtt, 2·EAI)·1000/escalate_permille kanal-türevi; parite kaydı süre dolumu K_MAP × RTO_MAX (parite kaydı için bellek sınırı; birim geç-ACK garantisi değildir — K_MAP = 8 ADET sınırıdır, süre garantisi değildir); tazelik max(10·srtt, RTO_MAX) (yalnız gözlem, `rtt_stale`) | ölü kanalda tam sessizlik / sahipsiz birim / birim başına tavansız kayıt (adet sınırı K4G-05c) / görünmez estimator donması (v1) | 4-decade kanıtı BEKLİYOR (v1.1 T-hücreleri D = 20 s tavana ulaşmaz; uzun pencereli hücreler DEMO-01 öncesi ön-kayıtlı gece kartında) — transport-layer-tdd AMENDMENT-12 |
| SAFE (doğru sınıf, örnek): pacer refill+kalan [R1], BURST_WINDOW [R3], PC geometrik takvim (RTT_floor-türevi), ACK_SILENCE=3·srtt, K=3 packet-threshold, ilk-SACK cadence-exempt | — | — | — | — |

**K-1 — Bilinen ölümcül KOMPOZİSYON (TUR-A ölçüm, atıflama sayaçlı):** temiz 1 kbps'te 27/27 loss-ilanı
time-threshold (packet-threshold=0). Zincir: eşik=9/8·srtt≈3.38s < SACK-varış-fiziği (2·serialization+
cadence-hold 4.6-8.4s) → her birim "kayıp" → `queue_losses` seq'i inflight'tan DÜŞÜRÜR → gerçek (geç)
ACK artık RTT/BW örneği ÜRETEMEZ (censor) → srtt düşük-yanlı kalır + W_BTLBW boşalır (btlbw 88→4) +
loss_ppm doyar → fuzzy arq=0 → pending_lost drain edilmez → state RecoverArq'a MIHLANIR (Startup
re-entry yolu da yok). Tek false-loss tüm ölçüm katmanını zehirler. Fix-sınıfı: eşik ack-cadence-farkında
+ un-loss yolu + eviction yerine geç-ACK'e açık muhasebe.

**K-1 KAPANIŞ DURUMU (TUR-1a→2b, hepsi ölçülü):** eşik-kolu ✓ (TUR-1a kanıt-şartı + TUR-1b
cadence-taban); ratchet/un-loss kolu ✓ (TUR-2a ledger — §5); **sansür-kolu ✓ (TUR-2b tombstone):**
`queue_losses` artık kaydı DÜŞÜRMEZ — active-flight muhasebesinden düşer (gönderim durmaz) ama
gönderim-kaydı (bw_sample dahil) TOMBSTONE'da yaşar; geç-ACK kol-1'de (retx'siz) BW+RTT örneği
KURTARILIR (§5 D-1/4). Kalan kollar: fuzzy arq=0 dejenerasyonu (TUR-4) + Startup re-entry/ProbeUp
(TUR-3c).

---

## 17. QUIC Comparison Harness (NORMATİF — §4.4 TDD'ye bağlı)

```
Bu belge engine'i üretir; KABUL kapısı transport-layer-tdd.md §4.4'tedir (tekrar tanımlanmaz, REFERANS).
Harness girdileri (engine'in ölçülen çıktıları → §4.4 metrikleri):
  - deadline-bounded completion, blackout survival, operational floor, handshake/first-delivery (§4.4.3 birincil)
  - time-to-first-delivery, p50/p95 (deadline-altı), bytes/msg, ACK overhead, energy (§4.4.3 secondary-but-required)
  - estimator accuracy (§7 BtlBw vs netem ground-truth) + loss/congestion confusion matrix (§4.4.4 mekanizma-kanıtı)
  - FEC efficiency (§10), CPU/byte (§4.4.6), güvenlik-pariteliği (§4.4.7 code-then-encrypt-auth-shards)
Karşı taraf: best-available production-grade QUIC (en güçlü mevcut CC), §4.4.1 kurulum.
GO/NO-GO: §4.4.10 üç-tier (Tier-1/2/3) + guardrail'ler. Bu belge o kapıyı BESLER, yeniden tanımlamaz.
```

---

## Ek — Cross-Doc Tutarlılık (NORMATİF)

```
transport-layer-tdd.md   : WHAT/policy otoritesi (§0.1 kripto, §2 wire, §4.4 benchmark, §5 fuzzy, INV-*).
ccr-format-spec.md §35   : Session Model V3 canonical byte'lar (SESSION_INIT/DATA); ACK/FEC_SHARD hop-transport (E2E DEĞİL).
identity §11 / §0.1      : client↔hub Noise IK/KK hop anahtarı SEAM'de (ProtectionContext::HubChannel); shard AEAD'i SEAM session E2E anahtarıyla yürütür (transport anahtar tutmaz, §0.2 AMENDMENT-5; RS transport'ta); hot-path thin.
ADR-01/02/16/17/18       : Rust/Tokio; reed-solomon-erasure + ErasureCodec trait; deterministik fuzzy; netem gate.
Bu belge byte-layout'ta ccr §35'i EZMEZ; ACK/FEC_SHARD hop-transport frame'leridir (E2E envelope'un dışında, §0.2).
```

## Ek-B — CANCELLED / MUST-NOT-IMPLEMENT (PRE_CONVERGE devri, revize-D0 D1) [K-KEYSTONE/R8/deprecate]

> Sessiz silme YOK — eski modeller gerekçesiyle burada kalır (izlenebilirlik). §3.3 PRE_CONVERGE bunları supersede eder.

```
[CANCELLED] Sabit INITIAL_RTT-seed cold-start modeli:
  Neydi: ilk RTT örneği gelene kadar srtt = INITIAL_RTT (sender.rs ≈100ms) + time-threshold loss detection.
  Neden CANCELLED: kbps'te ilk ACK ~1-2s uzakta ama 112ms'de "kayıp" ilan → spurious retransmit fırtınası
    (temiz 1 kbps'te 3× byte, ölçüldü). MUST NOT IMPLEMENT — yerine §3.3 en-kötü-köşe önsel + PRE_CONVERGE'de
    loss-ilanı DONDURULMUŞ (§3.3-C/1) + geometrik kopya (§3.3-B).

[MUST-NOT-APPLY (PRE_CONVERGE)] min_rate_bps=10000 fixture (budget.rs):
  Neydi: send_rate clamp floor'u 10000 B/s (=80 kbps), her state'te uygulanıyordu.
  Kural: PRE_CONVERGE'de floor = PRE_CONVERGE_FLOOR_BW (§16.1); min_rate=10000 MUST-NOT-APPLY (1 kbps
    kanalı 64× floodlar → self-congestion). NOT: steady-state minimal-reliable floor'un bandwidth-aware
    hale gelmesi (ölçülen bw'nin fraksiyonu, 4-decade) = ADIM-6 ana gövde kapsamı; burada BLANKET-deprecate DEĞİL.

[CANCELLED] below-Noise unauthenticated probe (revize-D0 keystone-öncesi / task §2.3 "Noise ALTINDA"):
  Neden CANCELLED: INV-CRYPTO-1 / §1 taksonomisi (unauthenticated transport-emitted class YOK) + §2a
    plaintext-magic + on-path echo-forge ihlali. Yerine §11.1 Noise-İÇİ authenticated CTRL_PATH_PROBE.
```

---

**Statü:** IMPLEMENTATION DRAFT — bölüm iskeletleri + formüller + wire layout + state machine tam; `[CAL]` sabitleri netem baseline'dan (ADR-18) kalibre edilecek, ardından golden vektörlere (§16) işlenecek. Bu belge tamamlanınca `Transport Protocol Specification v1.0`'a terfi eder. **PRE_CONVERGE cold-start (§3.3/§11.1/§16.1/Ek-B) revize-D0'dan D1 patch'iyle eklendi (docs/design/pre-converge-v1.md).**
