# N1 — seq16 / AEAD-nonce UYUM-RAPORU (Noise-wire D0)

> **Statü:** RAPOR-ONLY (kod-envanteri; kod YAZILMADI). D0-N1 kritik-yol maddesi.
> **Otorite-spec'ler:** `specs/kama-noise-wire-tdd-v2.0.md` §4 (PN/nonce-pseudocode) + §2.5 (PN-namespace/width-demux) · `specs/ccr-format-spec.md` §35.8 (seq16=resync-hint) + §35.9 (AAD).
> **HÜKÜM (özet):** seq16 **doğrudan-AEAD-nonce-DEĞİL** → **KIRMIZI-DEĞİL** (production-DURDUR tetiklenmedi). Kripto/AEAD/nonce **katmanı henüz YOK** (crypto crate 3-satır iskelet); mevcut `seq16→full-counter` yeniden-kurulumu **yalnız transport-amaçlı** (ordering/ACK/loss) ve **ccr §35.8-uyumlu**. **AEAD-nonce-türetimi + anti-replay accept() = D1-eksiği.**

---

## (1) SPEC NE DİYOR (bölüm-referanslı)

### noise-wire §4 — NONCE/PN/ANTI-REPLAY [NORMATİF — KİLİTLİ]
- **Açık yasak (satır 340):** "**seq-2B doğrudan-Noise-nonce-ise KABUL-EDİLMEZ.**" Noise explicit-nonce taşımaya izin verse de truncated-PN-reconstruction / replay / dh_epoch-transition **Noise'un parçası değil** — Kama Secure-Wire Profile'ın kendi normatif datagram sözleşmesi.
- **State per-(session, direction, dh_epoch)** — epoch-aware zorunlu (epoch-0/1 aynı PN değerlerini kullanabilir):
  - `expected_pn[dh_epoch]` (u64, en-yüksek-kabul+1, HER dh_epoch AYRI)
  - `replay_window[dh_epoch]` (bitmap, **WINDOW = 2^14 = 16384**, HER dh_epoch AYRI)
- **reconstruct(low_bits, low_width_bits, dh_epoch)** (satır 350-358): `span = 1<<low_width_bits` (16→65536 | 32→2^32); `center = expected_pn[dh_epoch]`; `base = center & ~(span-1)`; `cand = base | low_bits`; en-yakın-aday **UNDERFLOW-SAFE** (checked, u64-wrap-yok): `if cand + span/2 < center: cand += span` · `elif cand > center + span/2 and cand >= span: cand -= span`.
- **accept(candidate, dh_epoch_wire)** (satır 360-368): epoch-in-{current, previous-if-grace} değilse REJECT · `candidate + WINDOW < ew` → TOO-OLD · `candidate < ew && window_seen` → DUPLICATE · `candidate > ew + FUTURE_GAP_MAX` (**FUTURE_GAP_MAX = 2^13**) → FUTURE-GAP · AEAD_open fail → REJECT (**window İLERLEMEZ**) · SUCCESS → COMMIT (yalnız ilgili epoch state: `window_mark_seen`; `if candidate>=ew: expected_pn=candidate+1`).
- **8-adım ORDER (satır 387-389):** (1) fixed-cost outer parse (2) handle→layout (§2.5) (3) low_bits+dh_epoch→candidate (4) too-old/dup/future precheck **window-İLERLETMEDEN** (5) dh_epoch-cipher-state AEAD attempt (6) success→COMMIT (7) inner-parse+bounds (8) OpenedPacket-handoff. **"Unauthenticated-paket replay-window-İLERLETMEZ."**
- **Kapanış (satır 389-390):** "**N1 = kod-bu-pseudocode'a uyuyor-mu (uymuyorsa KOD-düzeltilir).**"

### noise-wire §2.5 — PN-NAMESPACE + WIDTH-DEMUX [KARAR VERİLDİ]
- **TEK direction-local full-PN-space** tüm sealed-class'lar için (SESSION_DATA + FEC-source/repair + secure-ACK/SACK/…). Legacy-`seq16` (16-bit) ve extended-`pn32` (32-bit) **AYNI** full-PN'in düşük bitleri; packet-class nonce-namespace'ini **BÖLMEZ** (satır 203-204).
- **Encoding:** mevcut SESSION_DATA `seq16` **BYTE-FOR-BYTE korunur** (P0-9); yeni class'lar 32-bit truncated-PN. **YOL-1** seçildi (handle-space demux: `h_legacy`→legacy-parser / `h_ext`→extended-parser); mühürlü SESSION_DATA envelope DEĞİŞMEZ (satır 191-198).

### ccr §35.8 — seq16 + Full-Counter Reconstruction + Replay [NORMATİF — sayısal]
- **`seq16` güvenlik-counter'ı DEĞİLDİR** (satır 2442, 2468): wire'da taşınan **düşük-bit / resync hint**'tir. Gerçek AEAD nonce/counter = **full session counter** (receiver state'inden reconstruct). Wire-alan-yorumu satır 2396: "seq16 u16_be # düşük-bit / resync hint (Section 35.8 — **TEK BAŞINA nonce DEĞİL**)".
- **reconstruct_counter(expected_counter, seq16)** (satır 2445-2457): 3-aday `[base|seq16, (base−0x10000)|seq16, (base+0x10000)|seq16]`, `argmin |c − expected|`; ACCEPT-koşulları: `>= lowest_acceptable`, `<= expected + MAX_FUTURE_GAP`, replay-bitmap'te-yok, `|c−expected| <= MAX_REORDER_DISTANCE`, epoch-match, AEAD-verify.
- **Sabitler (satır 2459-2462):** `REPLAY_WINDOW_SIZE = 1024` · `MAX_FUTURE_GAP = 4096` · `MAX_REORDER_DISTANCE = 8192` · `MAX_MESSAGES_PER_EPOCH = 2^32` · `REKEY_THRESHOLD = 2^31`.
- **INVARIANTS (satır 2468-2470):** "seq16 TEK BAŞINA nonce DEĞİL. AEAD nonce/counter = reconstructed full counter." · `replay identity = (session_id_internal, direction, epoch, full_counter)` · "**replay window AEAD success'ten ÖNCE GÜNCELLENMEZ.**"
- **AAD (§35.9, satır 2478-2486):** `SSD1 || version || cipher_suite_id || direction || epoch || seq16 || session_id_internal(16B)` — seq16 AAD'ye girer ama tek-başına-nonce değil.

> **Not (iki-otorite pencere farkı):** noise-wire §4 `WINDOW=2^14/FUTURE_GAP_MAX=2^13` verirken ccr §35.8 `REPLAY_WINDOW_SIZE=1024/MAX_FUTURE_GAP=4096` verir. **N1'in otoritesi noise-wire §4'tür** (D0-B gate); ikisi arasındaki sayısal reconciliation D1-tasarım işidir, N1-bulgusu değildir. **Her iki spec de aynı çekirdek ilkede birleşir:** seq16 = düşük-bit-hint, nonce = reconstructed full-counter, replay-window yalnız-AEAD-success-sonrası-commit.

---

## (2) MEVCUT KOD NE YAPIYOR (dosya:satır)

### 2.a Kripto/AEAD/Noise/nonce katmanı — **YOK (iskelet)**
- `crates/crypto/src/lib.rs:1-3` — **3 satır**, yalnız doc-comment: *"Noise + compact crypto layer … API-first iskelet; trait imzaları sonraki iş."* **Hiçbir Noise/AEAD/nonce/PN kodu yok.**
- `crates/ccr-codec/src/lib.rs:1-3` — **3 satır**, iskelet: *"CCR wire codec … API-first iskelet."* SESSION_DATA parse / seq-reconstruction **burada değil.**
- `crates/kama-transport-schema/` — **yalnız `Cargo.toml`**; hiç kaynak dosya yok.
- **grep `nonce`/`Nonce`/`NONCE` tüm `crates/**/*.rs` → SIFIR eşleşme.** Kod tabanında hiçbir yerde nonce türetimi yok.
- **AEAD referansları yalnız:** (i) yorumlar (SoC sınırını anlatan: transport AEAD tutmaz), (ii) `crates/bench/src/kama.rs` — **security-stub**: `TAG_STUB` = +16B "AEAD-tag SİMÜLASYONU" (`kama.rs:36,59`), gerçek kripto değil; modül-yorumu `kama.rs:5` açıkça: *"payload düz `0x5A`, seq/unit_id **cleartext**; anahtar/AEAD/Noise **YOK**."*

### 2.b seq16 üretimi (SENDER, full-counter) — `crates/adaptive-transport/src/sender.rs`
- `sender.rs:294` — `next_seq: u64` (state); `sender.rs:388` — `next_seq: 0` (init).
- `sender.rs:1367-1368` — atama noktası: `let seq = self.next_seq; self.next_seq += 1;` → **monoton u64 full-counter**, direction-local (per-sender-instance).
- `sender.rs:116-117` — `TransmitDecision.seq: u64` doc: *"Atanan wire seq (**full-counter; wire'da u16, §2**)."* → sender full-counter tutar, wire u16 taşır (§2.5-seq16-korunumu ile tutarlı).
- `sender.rs:5` (modül-yorumu) — *"bytes/AEAD/anahtar TUTMAZ"* (INV-ENGINE-SOC).

### 2.c seq16 wire-truncation + tüketimi (RECEIVER) — `crates/bench/src/kama.rs` (bench harness)
- **Truncation (tx):** `kama.rs:125` — `encode_data((d.seq & 0xFFFF) as u16, d.unit_id, payload)` → full-u64 → **düşük-16-bit** wire.
- **Wire-encode:** `kama.rs:53-58` — `encode_data(seq: u16, …)`: `b.extend_from_slice(&seq.to_be_bytes())` → **big-endian u16** (ADR-05, varint-yok). Header = `frame_type(1) + seq u16(2) + unit_id u64(8) = 11B` (`kama.rs:34`), sonra payload, sonra `TAG_STUB` (+16B sim).
- **Decode (rx):** `kama.rs:64-70` — `decode_data` → `(wire_seq: u16, unit_id: u64)`.
- **Reconstruction (rx):** `kama.rs:325-327`:
  ```
  let expected = rx.largest_seq().map_or(0, |m| m + 1);
  let full = resolve_seq(expected, wire_seq);
  rx.on_datagram(RxDatagram { seq: full, unit_id, kind: RxKind::Data }, now);
  ```
  → wire-u16 + receiver-state(`largest_seq+1`) → **full-u64 counter**.

### 2.d Reconstruction çekirdeği — `crates/adaptive-transport/src/seq.rs`
- `seq.rs:9` — `const SEQ_BITS: u32 = 16;`
- `seq.rs:20-35` — `pub fn resolve_seq(expected: u64, wire: u16) -> u64`: `win=1<<16=65536`, `hwin=win/2=32768`, `candidate=(expected & !mask)|wire`; ileri-epoch dalı `candidate + win`, geri-sar dalı `candidate - win`, aksi `candidate`. Doc: *"QUIC packet-number decode (RFC 9000 App. A.3) 16-bit'e özelleştirilmiş hali."*
- **Underflow-guard'lar mevcut:** `expected >= hwin && … && candidate <= u64::MAX - win` (satır 26) ve `candidate >= win && …` (satır 29) — u64-wrap koruması (§4-UNDERFLOW-KURALI ile aynı niyet).

### 2.e Receiver ordering-state — `crates/adaptive-transport/src/receiver.rs`
- `receiver.rs:54` — `RxDatagram.seq` doc: *"full-counter transport seq (codec `resolve_seq` ile çözdü)."*
- `receiver.rs:123` — `largest_seq: u64` (state); `:384/:389-390` güncelleme; `:469` `pub fn largest_seq()`.
- `receiver.rs:4` (modül-yorumu) — *"güvenlik katmanı (Noise+kompakt, §0.2) datagram'ı **çözer/verify eder**; … per-shard AEAD + RS-decode GÜVENLİK katmanında (authenticate-before-decode, INV-FEC-1)."* → verify/AEAD transport'un DIŞINDA olarak açıkça beyanlı.

### 2.f ACK yolu da aynı reconstruction'ı kullanır — `crates/adaptive-transport/src/ack.rs`
- `ack.rs:167` — `let largest_full = crate::seq::resolve_seq(expected, self.largest_seq);` (ACK-range'leri full-uzayda türetilir).
- `ack.rs:50` — `AckFrame.ack_epoch: u8` doc: *"Rekey / counter-wrap epoch'u."* **DİKKAT:** bu **transport counter-wrap epoch'udur, kripto dh_epoch(0/1) DEĞİL** — `ack.rs:16`: *"`ack_epoch` yalnız rekey sınırını disambiguate eder — u16 sarması epoch değişimi DEĞİLDİR."* Şu an yalnız-taşınan-placeholder u8; §4-`dh_epoch`-state'ine bağlı DEĞİL.

### 2.g Anti-replay accept() yarısı — **YOK**
- grep `replay` / `bitmap` / `dh_epoch` / `FUTURE_GAP` / anti-replay-`WINDOW` tüm `crates/**/*.rs` → **çekirdek yok** (yalnız `loss.rs`/`bw.rs`/`rate.rs` içindeki alakasız *sliding-time-window* metrikleri; bunlar RTT/BW/loss pencereleri, replay bitmap'i değil). §4-`accept()` (too-old / duplicate / future-gap / AEAD-gate / commit-on-success / unauth-window-freeze) **hiçbir yerde uygulanmıyor.**

---

## (3) UYUMLU / UYUMSUZ — HÜKÜM

| Soru | Bulgu | Hüküm |
|---|---|---|
| **seq16 doğrudan tek-başına AEAD-nonce mu?** | AEAD/nonce kodu hiç yok; seq16 yalnız transport ordering/ACK için full-counter'a çözülüyor | **HAYIR → KIRMIZI-DEĞİL** (production-DURDUR tetiklenmedi) |
| **full-PN-reconstruction var mı?** | `seq.rs::resolve_seq` (16-bit trunc → u64 full-counter), receiver-state (`largest_seq+1`) merkezli | **VAR** (ama transport-amaçlı, kripto-nonce için değil) |
| **ccr §35.8-uyumu (seq16=resync-hint)?** | Sender full-u64 tutar; wire düşük-16-bit; receiver full'e reconstruct eder | **UYUMLU** (seq16 tek-başına-nonce olarak KULLANILMIYOR) |
| **§4 accept() anti-replay (WINDOW/FUTURE_GAP/dedup/AEAD-gate)?** | Hiç yok | **YOK → D1-eksiği** |
| **Epoch-aware (dh_epoch 0/1, expected_pn[epoch])?** | Yok; tek `expected`; `ack_epoch` transport-wrap-epoch (crypto-epoch değil) | **YOK → D1-eksiği** |
| **32-bit extended-PN yolu (§2.5 h_ext)?** | Yok; yalnız `SEQ_BITS=16` sabit | **YOK → D1-eksiği** |

**NET HÜKÜM:**
1. **KIRMIZI-durum tetiklenmedi.** "seq-2B doğrudan tek-başına AEAD-nonce" ihlali **yok**, çünkü **kripto/AEAD katmanı henüz kod olarak mevcut değil** (`crypto` crate 3-satır iskelet, `grep nonce`=0). Mevcut mimarinin tamamı **adaptive-transport (motor)** tarafı; Noise/AEAD katmanı stub. Bench gerçek kripto yerine **identity/AEAD-tag-simülasyonu** kullanır (`kama.rs:5,36,59`).
2. **Mevcut `seq16→full-counter` reconstruction ccr §35.8 ilkesine UYUMLU:** seq16 wire'da düşük-bit/resync-hint; gerçek full-counter receiver-state'inden yeniden-kurulur; **seq16 hiçbir yerde tek-başına-nonce olarak kullanılmıyor.** Reconstruction *matematiği* de §4-`reconstruct` ve §35.8-`reconstruct_counter`'ın 16-bit hali ile **algoritmik-eşdeğer** (center/base/span; underflow-guard'lı).
3. **Ancak reconstruction'ın yalnız yarısı var:** §4/§35.8-`accept()` (anti-replay window, too-old/dup/future-gap precheck, AEAD-gate, commit-yalnız-success, unauth-window-freeze) ve **AEAD-nonce türetimi tamamen yok** → bunlar D1-eksiği.

---

## (4) (KISMEN-UYUMSUZ DEĞİL — EKSİK) KOD-DÜZELTME/YAZIM PLANI (spec DEĞİŞMEZ)

> Bu bir "UYUMSUZ-düzeltme" değil, **eksik-katman-yazımı**dır: mevcut `resolve_seq` §4-`reconstruct`'ın transport-formu; onu KIRMAYA gerek yok. §4-`accept()` + AEAD-nonce türetimi kripto-katmanında **YENİ** yazılacak. **noise-wire §4 pseudocode + ccr §35.8 aynen uygulanır.**

1. **Kripto-katmanı (`crypto` / yeni `kama-noise-wire` crate) — AEAD-nonce türetimi:**
   - Nonce = **reconstructed full-counter** (seq16 değil), §35.8/§4 uyarınca. Nonce türetimi: `(dh_epoch cipher-state) + full_pn` → Noise cipher-state nonce-counter'ı (§2.8: "full-PN = truncated-reconstruction, seçili-Noise-cipher-state nonce-counter'ı").
   - **INV-FEC-1/INV-ENGINE-SOC korunur:** transport AEAD tutmaz; nonce/AEAD yalnız güvenlik katmanında.

2. **Epoch-aware state (§4, satır 342-347):** `expected_pn[dh_epoch]` + `replay_window[dh_epoch]` (HER dh_epoch AYRI). Mevcut `receiver.rs::largest_seq` (tek, epoch'suz) **transport-ordering için kalır**; kripto-`expected_pn` **ayrı** kripto-state olarak eklenir (ordering-high-water ≠ crypto-anti-replay-high-water).

3. **`reconstruct` genelleştirme (§4, satır 350-358):** `resolve_seq`'i width-parametrik yap (`low_width_bits` ∈ {16 legacy, 32 h_ext}; §2.5) veya kripto-katmanında §4-imzalı `reconstruct(low_bits, low_width_bits, dh_epoch)` yaz. Mevcut 16-bit math'i koru (golden-vector-uyumlu, `tv_seq.rs`).

4. **`accept()` (§4, satır 360-368) — YENİ:** epoch-membership → too-old (`candidate + WINDOW < ew`) → duplicate (`window_seen`) → future-gap (`> ew + FUTURE_GAP_MAX`) → **AEAD-attempt** → success→COMMIT. **WINDOW=2^14, FUTURE_GAP_MAX=2^13** (noise-wire §4 otoritesi). `replay_window` = bitmap; **AEAD-success'ten ÖNCE İLERLEMEZ** (§35.8 satır 2470).

5. **8-adım ORDER (§4, satır 387-389):** outer-parse → handle→layout → candidate → precheck (window-ilerletmeden) → AEAD → COMMIT → inner-parse → handoff. **Unauth-paket window'u ilerletmez** (K-WIRE-DOS-1).

6. **Retransmit-nonce (§3, K-RETX-1):** aynı logical (block_id, source_esi) → **yeni full-PN + yeni nonce** (aynı-nonce-retransmit yasak). Sender `next_seq++` bunu zaten yapısal sağlıyor (her tx yeni seq); kripto-nonce buna bağlanır.

7. **AAD-bağı (§2.8 / §35.9):** nonce-türetimi AAD'den bağımsız; ama full_pn + dh_epoch + wire alanları AAD'ye girer (golden-vector-assert).

---

## (5) D1'DE YAZILACAK EKSİKLER (kesin liste)

- **[D1-1] AEAD-nonce türetimi (KRİTİK):** kripto-katmanı hiç yok. `crypto` crate 3-satır iskelet; nonce = reconstructed-full-counter kodu yazılacak (§2.8/§35.8). Şu an bench yalnız 16B AEAD-tag-**simülasyonu** kullanıyor (`kama.rs:36`).
- **[D1-2] Anti-replay `accept()` yarısı:** WINDOW=2^14 bitmap, too-old/duplicate/future-gap(2^13) precheck, unauth-window-freeze, commit-on-AEAD-success (§4 satır 360-368). Kod tabanında hiç yok.
- **[D1-3] Epoch-aware state:** `expected_pn[dh_epoch]` + `replay_window[dh_epoch]` (dh_epoch 0/1 ayrı). Mevcut `ack_epoch:u8` transport-wrap-epoch'tur, kripto-dh_epoch değil (`ack.rs:16,50`) → ayrı kripto-state gerekli. dh_epoch-0→1 grace geçişi (§4 satır 373-383) de yok.
- **[D1-4] 32-bit extended-PN yolu (§2.5 h_ext):** `SEQ_BITS=16` sabit; FEC/secure-control class'ları için `pn32` reconstruction + width-demux (YOL-1: h_legacy/h_ext) yok.
- **[D1-5] SECURE-WIRE-IDENTITY:** `(session_id, direction, dh_epoch, full_pn)` (§3). Mevcut `next_seq`/`largest_seq` yalnız direction-local-monoton; session_id + dh_epoch boyutları yok.
- **[D1-6] Reconstruction golden-vector genişletmesi:** `tv_seq.rs` yalnız 16-bit transport-vektörleri; §4-`accept()` + 32-bit + epoch-transition + wrap/reorder/replay-exhaustive vektörleri (K-NONCE-1) yazılacak.
- **[D1-7] Sender-tarafı kripto-nonce ledger:** `SealedPacketMeta.full_wire_pn/dh_epoch` (§0.5-P0-4). Mevcut `TransmitDecision.seq:u64` var ama SealedPacketMeta/dh_epoch/sealed_len yok.

### Kama-Noise-Wire uyum durumu — özet tablo
| §4 / §35.8 bileşeni | Kodda | Nerede |
|---|---|---|
| seq16 = wire düşük-bit (resync-hint) | ✅ | `kama.rs:125`, `seq.rs`, `sender.rs:116-117` |
| full-counter reconstruction (16-bit) | ✅ | `seq.rs:20-35`, `receiver.rs:54`, `ack.rs:167` |
| seq16 tek-başına-nonce KULLANIMI | ❌ (yok = iyi) | — (AEAD katmanı yok) |
| AEAD-nonce = reconstructed-counter | ⛔ D1 | kripto-katmanı yok |
| anti-replay `accept()` (WINDOW/gap/dedup) | ⛔ D1 | yok |
| epoch-aware `expected_pn[dh_epoch]` | ⛔ D1 | `ack_epoch` transport-wrap, kripto değil |
| 32-bit extended-PN (h_ext) | ⛔ D1 | `SEQ_BITS=16` sabit |
| 8-adım open ORDER + unauth-freeze | ⛔ D1 | yok |

**D1-ENTRY-KAPISI (N1 açısından):** KIRMIZI-yok → engel yok. Ama "seq16-nonce-değil-full-PN-reconstruction-uygulanıyor" şartı (belge-sonu STATÜ-MERDİVENİ, D1-IMPLEMENTATION-GO) şu an **yalnız transport-yarısında** karşılanıyor; kripto-yarısı (AEAD-nonce + accept()) D1'de yazılacak.
