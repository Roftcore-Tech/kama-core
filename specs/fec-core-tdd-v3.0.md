# kama-fec-core-tdd — Kripto-Agnostik FEC/Erasure Çekirdeği [v3.0 — dört-karar-boşluğu-KAPATILDI + öz-tarama-temiz; FULL-DESIGN-GO-onayı: Abkarada; + TRANSPORT-NON-REGRESSION]

> **STATÜ (MADDE-1 — tek-merdiven):** wire-tasarımı-KİLİTLİ (§2.6-noise-wire byte-layout, §11-PMTU — final). **GO:** FEC-
> crate-POC · canonical-source-symbol · metadata-recoverability · golden-vectors · profile-çalışması · **FEC-core-
> POC/codec-KODU** (kripto-agnostik-taraf-BAŞLANIR). **D1-ENTRY-NO-GO:** N1-N10-UYUM + POC-tamamlanmadan production-
> secure-wire-adapter + engine-integration-D1-koduna-başlanmaz. **PRODUCTION-NO-GO:** hostile/regresyon-testleri-
> tamamlanmadı. (final-wire-schema ARTIK-KİLİTLİ — §2.6; "final-schema-NO-GO"-ifadesi-KALDIRILDI.) D1-yalnız-
> ortak-D0-B-entegrasyon-gate-eksiksiz-geçilince. **EK-KISIT (§0.8): mevcut-mühürlü-transport-mimarisi (L1-L4 +
> SESSION_DATA-22B-envelope, ccr §35-canonical) DOKUNULMAZ — wire-freeze-NO-GO YALNIZ-L5-FEC-iç-wire-içindir;
> mühürlü-envelope-zaten-freeze (yeniden-tanımlanmaz).**

> **Statü:** TASARIM-SÖZLEŞMESİ (transport-layer-tdd.md L5-kardeşi). **v2→v2.1 (dördüncü-geçiş,
> kama-fec-core-noise-wire-full-scale-analysis):** SoC-bölünmesi-ONAYLANDI ("geri-alınmamalı") ama on-yeni-P0
> (arayüz-boşlukları). Bu-belgeye-düşenler: SoC-terim-sızıntı-temizliği (unique_auth→unique_admitted vb.) +
> metadata-recoverability (P0-2, kaybolan-source'un-message_id/len/flags'i) + R_esi_reserved-zinciri (R_sent-terimi-KALDIRILDI) +
> sealed_len-tabanlı-controller + zeroization-kapsam-genişletme (repair+scratch-de-hassas) + PMTU-mid-block +
> kapsam-alt-ayrımı (Codec-Core vs Recovery-Controller). **v1.2 → v2-BÖLÜNME:** Abkarada'nın
> crypto-separation-analizi (SoC) v1.2'yi İKİ-belgeye-böldü: (1) **bu belge = kama-fec-core-tdd** (kripto-
> AGNOSTİK, bağımsız-test-edilebilir) + (2) **kama-noise-wire-tdd** (Noise/AEAD/nonce/replay/key-phase/Cryptex).
> Tüm-kripto-normatifleri v1.2'den kama-noise-wire-tdd'ye TAŞINDI. Bu-belge YALNIZ transport-recovery.
>
> **NORMATİF-MİMARİ-KİLİT (SoC-analiz-§15):** Kama-FEC-Core confidentiality/peer-auth/AEAD/key-exchange/nonce/
> kripto-replay/key-phase/rekey GERÇEKLEŞTİRMEZ. Yalnız bounded-profile + logical-block/symbol-kimlikleri +
> OPAQUE-symbol-byte-dizileri. Production-wire-koruması çekirdek-DIŞINDA, tek-kapalı kama-noise-wire-profile'da.
> Bu, generic-crypto-provider-API OLUŞTURMAZ (TLS/DTLS/IPsec/3rd-party-crypto kapsam-DIŞI).
>
> **İKİ-BELGE-ARAYÜZÜ (yalnız-bu):** UnsealedTransportShard / OpenedTransportShard + ortak wire-budget/PMTU-
> sözleşmesi. Başka-bağımlılık-YOK. **Bağımlılık-yönü:** fec-core(bağımsız) ← transport-core ← engine → noise-wire.
> **YASAK:** fec-core→noise · fec-core→AEAD · fec-core→identity/trust · transport-core→NoiseState/AeadKey.
>
> **Zemin:** fec.rs-envanteri (FEC_SHARD-framing 0x20 VAR / RS-motor YOK / reed-solomon-bağımlılığı YOK).
> Standart: RFC 6363 (framework) · RFC 5510 (RS-erasure, MDS, on-demand-repair) · RFC 6865 (simple-RS) ·
> RFC 8681 (sliding-window, 2.aşama) · RFC 9265 (FEC+congestion). Crate: reed-solomon-simd (v3.1.0, SPDX
> MIT-AND-BSD-3, Leopard/Taylor-soyu=CCat-aynı-yazar, raw-shard-API, erasure-only=AEAD-bütünlük-ayrı-ideal).

## 0. Problem + niş (SoC-korunmuş)

Bu-fazın-tüm-recovery'si ARQ'ydu; FEC kodda-ölü. ARQ-tavanı: her-kayıp=bir-round; sub-kbps'te-saniyeler; dönüş-
%80-kayıplıysa retransmit-isteği-de-kayıplı. FEC-tavanı-kaldırır: **proactive-parite** önden, dönüş-bağımsız.
**Niş (SoC-doğrulanmış):** QUIC-DATAGRAM+app-FEC teknik-mümkün → "QUIC-giremez" İDDİA-KALDIRILDI. Fark: hostile/
asimetrik-recovery-policy + reverse-bağımsız-proactive-repair + custom-pacing + entegre-ARQ+FEC + sub-kbps-
survival. **Ve (SoC): codec-Noise-olmadan-test-edilir** (sim-first-korunur; QUIC-tuzağından-kaçış).

## 0.8 TRANSPORT-NON-REGRESSION INVARIANT (Abkarada-kısıtı — MÜHÜRLÜ-MİMARİ-DOKUNULMAZ) [NORMATİF, EN-ÜST-ÖNCELİK]

**FEC/noise-wire mevcut-KANITLANMIŞ-ve-MÜHÜRLENMİŞ transport-mimarisini BOZAMAZ.** Bu, tüm-diğer-FEC-hedeflerinin-
ÜSTÜNDE bir-kısıt: FEC-faydası-mühürlü-kazanımı-bozma-pahasına-KABUL-EDİLMEZ.

**DOKUNULMAZ-KATMAN + MÜHÜRLÜ-WIRE-BASELINE:** FEC = L5-opsiyonel (transport-layer-tdd §L5). Transport-çekirdeği
**L1(IP/UDP) · L2(hub-routing-handle) · L3(E2E-envelope) · L4(session-crypto/Noise-split) DEĞİŞMEZ.** ARQ-yolu
(RecoverArq/RecoverFwd/SACK/return-supply) DEĞİŞMEZ — FEC onun-YANINA, rejime-göre-seçilir.

**MÜHÜRLÜ-WIRE-FORMAT (ccr-format-spec §35 = canonical-otorite + transport-layer-tdd §0-2 — DEĞİŞMEZ, YENİDEN-
TANIMLANMAZ):**
```
L1  IP/UDP                    UDP/IPv4=28B · UDP/IPv6=48B
L2  hub_handle : 4B           cleartext, hub-okur/route (hub-assigned per-hub-counter, SESSION_INIT'te-atanır)
L3  E2E-envelope: seq 2B + ciphertext + AEAD-tag 16B   hub-OKUYAMAZ; length-IMPLICIT (UDP-sınırı, wire'da-0B)
L4  Noise-split               N-türevi-zero-RTT'den (§0.9)
SESSION_DATA hot-path (TEK-form): [hub_handle 4B][seq 2B][ct N][tag 16B] = 22B-overhead + payload
  UDP/IPv4 = 50B+pl · UDP/IPv6 = 70B+pl. link_id/cert/signature/length/KCP/FEC/outer-AEAD = YOK.
SESSION_INIT (zero-RTT + early_data, konuşma-başına-1) · SIGNED_CONTROL (+Ed25519 64B, nadir).
Invariant'lar (ccr §35): INV-FLAGS-1 · INV-MAGIC-1 (SSI1-vb-domain-sep) · INV-RESUME-1 · INV-WIRE-2 (per-mesaj-
  imza-YOK; N-türevi-ss_s-static-DH-auth). Magic-bytes-TBS'te. cert_mode-2-bit-flags. Test-vektörleri-VAR.
```
**FEC'in-wire-KATKISI YALNIZ L5-iç-wire'ı** (inner-FEC-header + shard-ESI-layout, §5.5/§9) VE noise-wire'ın-per-
shard-seal'i (kama-noise-wire-tdd). **L1-L4 + SESSION_DATA-22B-envelope MÜHÜRLÜ — FEC-bunları-SARMAZ** (L3-inner-
AEAD-tag-zaten-authentication; established-mesaj-L5-zarfına-DEFAULT-girmez). FEC-shard'ları AYRI-wire-tipi (SESSION_
DATA-değil); mühürlü-envelope-bozulmadan-yanında-taşınır.

**MÜHÜRLÜ-KAZANIMLAR (ölçümle-kilitli, FEC-varlığı/yokluğu-DEĞİŞTİREMEZ):**
```
P5-GATE {100×12, S5-lotarya}          — üç-kez-S5-PASS, netem-RNG-varyanslı (regresyon-kalkanı)
DESEN-1 kapanışı (PR#40+#41)          — uzun-daralma 4114→308, onset-p95=31.5s<35, tek-adım-%52-84, re-flood-yok
FAZ-0 qd-canlanma (PR#41)             — ack_epoch→w_epoch, qd-netem-52.2s
delivered-EAI (Y3) + INV-RATE-1       — loss send_rate'i-DÜŞÜRMEZ (yapısal); bayat-max-srtt-bağımsız-expire
ZAMAN-MODELİ (ZM-1..4)                — event-count≠zaman
DV-1/DV-2/DV-3                        — decade-tutarlılık + rejim-geçişleri (DESEN-1/2/3)
desen1_sim.rs 5-pin                   — kalıcı-regresyon-kalkanı
TUR-S-sensing-hibrit                  — sayaç-dedektör + q̂-kapılı-tüketiciler (onaylı, kod-pending)
```

**NON-REGRESSION-ŞARTI (D0-B-gate + her-FEC-PR-öncesi ZORUNLU):**
1. **FEC-KAPALI (q̂≈0):** hot-path allocation-yok/minimal-sabit · ekstra-block-latency-yok · mevcut-pacing/ARQ/CC-
   gate-BİT-BİT-DEĞİŞMEZ. desen1_sim.rs 5-pin BİREBİR-geçer.
2. **FEC-AÇIK:** FEC-devredeyken-DE mühürlü-kazanımlar-KORUNUR — FEC-controller-çıkışı (repair-emisyon) same-pacer-
   invariant-içinde (§8), yani FEC-açılınca P5-GATE/DESEN-1/qd-canlanma/delivered-EAI-davranışı-BOZULMAZ. (Örn: repair-
   burst delivered-EAI-flood-self-stop'u-geciktirmez; FEC-recovered-source BtlBw-şişirmez §10.)
3. **PAYLAŞILAN-KOD-DEĞİŞİKLİĞİ-YASAK-LİSTESİ:** FEC/noise-wire şunları-DEĞİŞTİREMEZ (yalnız-OKUR): bw.rs-delivered-
   EAI/last_sample_bw · sender.rs-w_epoch/W_BTLBW · send_rate-loss-modeli (INV-RATE-1) · ZAMAN-MODELİ-event/saat-
   ayrımı · SACK-V2/ACK-cadence · TokenBucket-restore_burst. Bunlara-dokunma-GEREKİYORSA → AYRI-DECISION + Abkarada-
   onayı + mühürlü-testler-yeniden-koşulur (sessiz-değişiklik-YASAK, §çalışma-disiplini).
4. **REGRESYON-KANITI:** her-FEC-PR K-FEC-OFF-1 + mühürlü-pin-seti (P5{100×12,S5} · DESEN-1-netem-4114→308 ·
   qd-52.2s · DV-decade · Y3-yakınsama) YEŞİL-olmadan-MERGE-EDİLMEZ. teşhis=SİM (desen1_sim.rs), doğrulama=NETEM-
   PR-öncesi-bir-kez.

**Bu-invariant K-FEC-OFF-1'i (§14) KAPSAR-ve-GENİŞLETİR:** OFF-1-yalnız-FEC-kapalı; bu-invariant FEC-açık-da-dahil
+ paylaşılan-kod-dokunulmazlığı + mühürlü-pin-regresyon-kanıtı.

## 0.5 HOSTILE-REJİM-HARİTASI + İDDİA-SINIRI (beşinci-geçiş-§9/§13 — NORMATİF-daraltma)
**Bu-mimari = hostile-ORIENTED block-FEC + recovery-controller; HENÜZ genel-hostile-channel-FEC / anti-jamming-
transport DEĞİL.** Ürün-dili-buna-göre (P2-iddia-riskleri: Cryptex-"tam-gizlilik"-DEĞİL · R_code_max-koruma-diye-
RAPORLANMAZ · blackout-çözümü-DEĞİL · sub-kbps-başarısı-payload-rate-üzerinden-DEĞİL-sealed-wire-byte-üzerinden ·
TLS/DTLS-"drop-in"-DENMEZ).
**GÜÇLÜ-rejimler (GO-adayı, ölçümle):** IID/random-loss · orta-burst (R-sınırı-içinde; repair-pacing+clumping-
testi-şartlı) · ÇOK-KÖTÜ-REVERSE (en-güçlü-niş: proactive-repair-feedback-beklemez; AMA bootstrap+feedback-
lifecycle-kapanmadan-KANITSIZ) · yüksek-reorder (tasarımda; impl-test-şart).
**ZAYIF/NO-GO-rejimler (beyanlı):** uzun-korelasyonlu-burst (burst>R → block-çöker; interleaving-ertelendi →
**MVP-uzun-burst-hostile-ready-DEĞİL**) · congestion-loss (controller-ölçümsüz-NO-GO) · adaptif-jammer (Cryptex-
optimizasyonu-ZORLAŞTIRIR, anti-jamming-GARANTİSİ-DEĞİL — jammer-indiscriminate-düşürerek-block-bozar) · TAM-
BLACKOUT (NO-GO-rejimi → S&F/alternate-path) · sub-kbps (teorik-mümkün; sealed-byte+deadline-ölçümsüz-KANITSIZ).
**Gerçek-dünya:** savunma/özel-ağ-uygun (link-bilinir+mode-sabit+operatör-cap+PMTU-kontrollü); kamu-internet/mobil-
zor (congestion-vs-radio + NAT-rebind + PMTU-blackhole + handover); uydu/düşük-bitrate (tag-amortizasyon+block-
latency+stale-feedback-kritik); jammed-RF (kısa/orta-burst-yararlı; uzun-burst/blackout-iddia-DAR).

## 1. FEC-CORE'un BİLDİĞİ / BİLMEDİĞİ (SoC-analiz-§3, NORMATİF)
**BİLİR:** fec_version · code_suite_id · profile_id · block_id · ESI/symbol_id · K_actual · R_code_max ·
R_initial · R_esi_reserved_unique · R_socket_submitted · symbol_length · original_length · deadline · OPAQUE-symbol-bytes.
**YAPAR:** opaque-bytes→source-symbols · source→repair-symbols · received-distinct→missing-source-reconstruction ·
block/profile-bounds-validation · logical-symbol-dedup · decoder-rank-tracking · block-lifecycle-integration.
**BİLMEZ (fec-crate-API/controller/state-machine'e SIZAMAZ):** NoiseState · AEAD-key · nonce · key-phase ·
REKEY() · auth-tag · crypto-epoch · handshake-transcript · current/prev-receive-key · kripto-replay-window ·
cipher-suite-selection.

## 1.5 SECURITY-ORDER-CAVEAT (beşinci-geçiş-§3/§8 — "agnostik"-iddiasının-doğru-sınırı) [NORMATİF]
**DOĞRU-İDDİA:** "FEC-Codec-Core kripto-algoritma/key/nonce/tag/handshake-BİLMEZ; opaque-symbols-üzerinde-
deterministik." **YANLIŞ-GENİŞ-İDDİA (KULLANILMAZ):** "Bütün-sistem-herhangi-kripto-katmanıyla-aynı-güvenlik/
performansı-korur." **Kripto-YERLEŞİM-SIRASI güvenliği-değiştirir:**
```
CRYPTO-ABOVE (TLS/DTLS/Noise-üstte, FEC-altta): pasif-dinleyiciye-payload-gizliliği-YETER;
  aktif-hostile-YETMEZ — repair-shard-üst-katman-tag'i-TAŞIMAZ → forged-repair-decoder'a-ulaşır (poisoning/DoS,
  nihai-record-doğrulama-tespit-eder-ama-CPU/RAM-tükenmiş-olur) + ACK/recovered/control-KORUMASIZ (sahte-feedback-
  ARQ/CC-bozar) + FEC-metadata-AÇIK (Cryptex-hedefi-sağlanmaz).
CRYPTO-BELOW (FEC-önce, per-packet-seal-sonra = kama-noise-wire): aktif-hostile-DOĞRU-VARSAYILAN —
  source/repair/control-bağımsız-doğrulanır + forged-decoder'a-ULAŞMADAN-düşer + metadata-ciphertext'te.
  Bedeli: unsealed-buffer-hassasiyeti + per-shard-tag-overhead + PN/replay/key-state-machine.
```
Alternatif-katman-notu (beşinci-geçiş-§4-7): TLS-packet-protector-NO-GO (stream-bekler) · TLS/DTLS-above =
fonksiyonel-adapter-araştırması-LIMITED-GO-ama-hostile-güvenlik-3-5/10 · DTLS-below-teknik-güçlü-AMA-drop-in-DEĞİL
(ayrı-KamaDtlsWireProfile+full-security-review-ister) · IPsec/WG-tünel lab-mümkün, normatif-profil-yerine-geçerse
sealed_len/PMTU/controller-doğruluğu-zayıflar. **Crypto-profile-DEĞİŞİRSE exact-sealed_len + security-admission
YENİDEN-GATE-EDİLİR.**

## 2. FEC-API-SINIRI (opaque-symbol, SoC-analiz-§4)
```rust
pub struct SourceSymbol<'a> { pub block_id: BlockId, pub esi: SourceEsi, pub bytes: &'a [u8] }
pub struct RepairSymbol   { pub block_id: BlockId, pub esi: RepairEsi, pub bytes: Vec<u8> }
pub trait FecCodec {
    fn encode(&self, profile: &FecProfile, source: &[SourceSymbol<'_>]) -> Result<Vec<RepairSymbol>, FecError>;
    fn reconstruct(&self, profile: &FecProfile, received: &[ReceivedSymbol<'_>]) -> Result<Vec<RecoveredSource>, FecError>;
}
```
API'de BULUNMAZ: NoiseTransportState · AeadKey · Nonce · KeyPhase · AuthenticationTag · CryptoEpoch.
**FEC-açısından symbol-byte'ı plaintext/ciphertext/compressed/file-chunk/protobuf/Noise-record/app-frame FARK-ETMEZ.**

## 3. BLOK-YAPISI + STANDART (RFC 5510-uyumlu-İLKE, wire-compatible-DEĞİL)
RFC 5510: sistematik-MDS, GF(2^m), erasure-konumu-BİLİNİR, on-demand-repair-izinli. reed-solomon-simd RFC-5510-
İLKELERİYLE-uyumlu ama wire-compatible-DEĞİL (Kama-özel-wire). **NORMATİF:** inter-packet-erasure; intra-packet-
kapsam-dışı (Kama-transport). shard-datagram PMTU-aşmaz, IP-fragmentation'a-güvenilmez.
**Alpha = BLOCK-RS** (async-first: mesaj/chunk-sınırı-doğal-block-sınırı; basit-test-mature-crate-bounded-memory).
Streaming = 2.aşama. **FEC-SHARD-TAŞIMA [NORMATİF — KARAR-VERİLDİ, PATCH-2]:** FEC-source/repair-shard'ları **SESSION_DATA-DEĞİLDİR**
ve-mühürlü-22B-SESSION_DATA-envelope'unu-SARMAZ. FEC-shard'ları **AYRI-EXTENDED-secure-wire-packet-class'larıdır**
(kama-noise-wire-tdd-§2.5-h_ext-layout). **▼KOD-ZEMİNİ-1 [UYUM-DOĞRULAMASI]:** mevcut-fec.rs-FEC_SHARD(0x20)-yolu
bu-karara-UYUYOR-mu (kararı-yeniden-seçmek-DEĞİL); INV-FEC-1-code-then-encrypt-authenticate-shards-hizası. **ALPHA-BLOCK-SINIRI [NORMATİF — KARAR-VERİLDİ, PATCH-3]:** block-sınırı **message/chunk-tabanlıdır.** Bir-FEC-
block birden-fazla-application-message-İÇEREMEZ; büyük-message birden-fazla-SIRALI-block'a-bölünebilir; cross-
message-block-YASAK. Streaming/convolutional-FEC = Beta.

## 4. R-ÜÇLÜ-AYRIM (SoC-analiz-§3 + go/no-go-§3.4) [NORMATİF]
```
K_actual · R_code_max(WIRE-CODE-SHAPE-immutable) · R_initial(controller-kararı, wire-shape-DEĞİL) ·
R_esi_reserved_unique(code-space-sayacı) · R_socket_submitted(gönderim) · R_extra_budget · E(symbol-byte-len)
decode-koşulu: unique_ADMITTED_source + unique_ADMITTED_repair ≥ K_actual
```
**CRATE-KRİTİK:** reed-solomon-simd original_count+recovery_count+shard-size-SABİT-alır → R_code_max-BAŞTAN-SABİT.
on-demand-repair = önceden-sabit-code-space'teki-GÖNDERİLMEMİŞ-ESI (yeniden-encode-YASAK). Profile-immutable-tuple:
(code_suite_id, K_shape, R_code_max, E, partial_policy, packet_schedule_policy). R_initial-wire-shape-DEĞİL.
**RUNTIME-KORUMA ≠ R_code_max** (R_code_max=6-ama-yalnız-2-wire'a-çıktıysa → 2-korumada). **R_SENT-TERİMİ-
KALDIRILDI (analiz-P0-4 — reservation↔send-yarışı):** code-space-hesabı **R_esi_reserved_unique**-üzerinden:
```
remaining_code_space = R_code_max − R_esi_reserved_unique     # ESI-reserve-monoton-idempotent, ANINDA
zincir: R_esi_reserved_unique → R_generated_unique → R_pacer_admitted → R_socket_submitted → [R_wire_sent-yerel-
        ölçülemez] → R_acked / R_received_reported → R_useful   # HER-halka-AYRI-telemetry
```
(Yarış-örneği-kapandı: ESI-0/1/2-üretildi-kuyrukta → controller-tekrar-koşar → reserved_unique=3-görür → aynı-ESI-
yeniden-reserve-EDİLEMEZ.) "gönderilmiş"-anlamı-gereken-yerde R_socket_submitted-kullanılır.

## 5. CODE-SUITE-NEGOTIATION (go/no-go-§3.6 — NEGOTIATION-BINDING engine'de, §12) [NORMATİF]
ÜÇ-AYRI-ID: fec_version(envelope) · code_suite_id(RS-matematiği/generator/ESI) · profile_id(K/Rmax/E/partial/
schedule). **GOLDEN-VECTOR-ZORUNLU:** her-suite/profile canonical-source→expected-repair→kayıp→reconstruction +
byte-order+padding+ESI-mapping; iki-bağımsız-impl-aynı-byte. **NOT (SoC):** negotiation'ın-AUTHENTICATED-binding'i
engine/session-orkestrasyon-işi (§12); FEC-Core yalnız-suite/profile-SEMANTİĞİNİ-tanımlar, transcript'e-KENDİ-
bağlanmaz.

## 6. KISMİ-BLOK (go/no-go-§3.5, tek-biçim) [NORMATİF]
MVP: arbitrary-partial-K-YOK, enumerated-profile (K_shape∈{2,4,8,..}, R_code_max+E-profil-içi-sabit). Mesaj-son:
en-küçük-uygun-profile → gerçek-source-count block-descriptor'da → kullanılmayan-slot canonical-zero-pad →
padding original_len-ile-kesilir → golden-vector-byte-order/filler → cross-message-YOK. RS-shortening-alternatifi
MVP-için-riskli (shortening-algoritması+ESI-mapping wire-standardı-olmalı).
**MODEL-A KNOWN-ZERO-INJECTION (analiz-P0-3 — KARAR, decode-matematiği-kesin) [NORMATİF]:** fixed-K_shape;
known_zero_count = K_shape − K_actual. **Sender:** zero-slot'ları-ENCODE'a-katar (repair-üretimi-için), wire'a-
GÖNDERMEZ. **Receiver:** K_actual'ı-öğrenir → known_zero_count-adet-canonical-zero-shard'ı-decoder'a-BİLİNEN-
SOURCE-olarak-ENJEKTE-EDER (rank-bedava). **K_ACTUAL-KAYNAĞI [NORMATİF — KARAR-VERİLDİ, PATCH-4]:** K_actual **HER-FEC-source-ve-repair-shard'ın-
authenticated-encrypted-inner-header'ında-AÇIKÇA-taşınır** (kama-noise-wire-§2.6-k_actual-u8). Tek-descriptor'a-
VEYA-dolaylı-türetime-BAĞIMLI-DEĞİLDİR. Inner-K_actual ile-active-profile/block-state-uyuşmazsa: packet-REJECT +
rank-ARTMAZ + allocation-GENİŞLEMEZ. Decoder-bootstrap-tek-biçimli. **Decode-koşulu-kesin:**
```
decode iff: received_source + received_repair + known_zero_count ≥ K_shape
(eşdeğer: unique_admitted_source + unique_admitted_repair ≥ K_actual — enjeksiyon-varsayımıyla; §4-koşulu-böyle-okunur)
```
Zorunlu-POC-testleri: K_shape=8×{K_actual=1,7} · yalnız-repair · mixed · known-zero-injection · wrong-K_actual-
fail-closed · sender/receiver-K_actual-mismatch-reject · final-block-padding-recovery.

## 6.5 METADATA-RECOVERABILITY (P0-2, dördüncü-geçiş — codec-wire-contract'ın-en-kritik-eksiği) [NORMATİF]

**PROBLEM:** RS yalnız-symbol-BYTES-reconstruct-eder. Kaybolan-source'un inner-metadata'sı (message/chunk_id ·
original_length · final/end-of-message-flag · offset · type) OTOMATİK-GERİ-GELMEZ. Block-descriptor-tek-shard'
daysa o-shard-kaybolunca decode/reassembly-İMKANSIZ.

**ÜÇ-TASARIM-SEÇENEĞİ:** (A) source-specific-metadata CODED-SYMBOL-İÇİNDE: canonical_source_symbol =
source_subheader(msg/chunk-id-veya-derivable-offset · actual_payload_len · source-ESI · final-flag · type/ver)
‖ application_chunk ‖ canonical_padding → RS-tamamını-kodlar, kayıpta-metadata-da-recover. (B) metadata
TAMAMEN-TÜRETİLEBİLİR: (session, block_id, profile_id, source_esi)-tuple'ından-hesaplanır (örn chunk_offset =
block_base_offset + ESI × E_payload). (C) block-descriptor-tekrarla/coded — TEK-descriptor-paketine-bağımlılık-
KABUL-EDİLMEZ.

**MVP-KARARI [TEK-BİÇİM — PATCH-5]:** **Block-common** metadata HER-shard'ın-encrypted-inner-header'ında:
protocol-version · packet-type · profile_id · block_id · ESI · K_actual (§2.6-noise-wire-layout). **Source-
specific** metadata RS-kodlanan-canonical-source-symbol-PREFIX'inde: message_id · chunk_offset · actual_payload_
len · final/message-flags. Repair-symbol'ları = prefix+payload+canonical-padding-bütününün-RS-çıktısı → kaybolan-
source-reconstruct-edildiğinde-metadata-da-reconstruct. Tamamen-derivable-alanlar YALNIZ-duplicate-azaltmak-için;
zorunlu-source-specific-metadata-için-genel-alternatif-SAYILMAZ. **GOLDEN-VECTOR yalnız-repair-
byte-DEĞİL, reconstructed-METADATA'yı-da-test-eder.** **D0-metadata-recoverability-raporu (18.5):** her-source-
specific-alan-için {repeated? / derived? / FEC-coded? / block-common? / decode-öncesi-gerekli? / app-teslim-
öncesi-gerekli?} tablosu; **hiçbir-gerekli-metadata tek-kaybolabilir-pakete-bağlı-KALMAZ.**
**PREFIX-DOĞRULAMA [NORMATİF — MADDE-9, yalnız-prefix'te-BULUNAN-alanlar]:** block_id/source_esi/profile_id
PREFIX'TE-YOK (§2.6-noise-wire: prefix = message_id/chunk_offset/actual_payload_len/source_flags). Bunlar decoder-
CONTEXT + recovered-source-INDEX'inden-alınır (inner-header-eşitliği-DEĞİL — imkansız-eşitlik-kaldırıldı). Prefix-
doğrulama YALNIZ: actual_payload_len ≤ (E−11) · source_flags-reserved-nonzero→reject · chunk_offset-block-
tutarlılığı. Mismatch → reject + rank-ARTMAZ + allocation-GENİŞLEMEZ + reason-code (içerik-sızdırmaz). K_actual-
uyumu inner-header'da (§2.6, block-common) — o ayrı-kontrol.
**ON-DEMAND-RECOMPUTE-NETLEŞTİRME (analiz-P0-11 — "yeniden-encode-yasak"-daraltıldı):** YASAK-olan = farklı-
profile/matrix/source-ordering/code-space-ile-yeniden-encode. **SERBEST** = aynı-immutable-source-set+aynı-profile'
dan kullanılmamış-repair-ESI'yi-sonradan-DETERMİNİSTİK-yeniden-hesaplamak (golden-vector-aynı-byte-şartıyla) —
aksi-halde-tüm-R_code_max-bellekte-tutulur=on-demand-anlamsız. POC-ölçer: full-encode-vs-tek-ESI-recompute-CPU/mem ·
crate-tek-ESI-API'si · recompute==golden-vector.

## 7. INTERLEAVING — 2.AŞAMA (go/no-go — zorunlu-DEĞİL)
RS-bloğu blok-içi-herhangi-R-erasure'ı-kurtarır (ardışık-farketmez). Interleaving-yalnız R'den-UZUN-burst'ü-çok-
bloğa-dağıtır. Bedeli: çok-açık-block+memory+completion-latency+scheduling+rekey-yarım-block. → Block-MVP'de-
ZORUNLU-DEĞİL; burst-ölçümü+latency-budget-ile-2.aşama. **GECİKME (sub-kbps-KRİTİK):** block_fill≈K×D×packet_
interval; sub-kbps'te-D=6-bile-kabul-edilemez. interleaving_depth q̂'dan-türetilemez (burst-dağılımı+interval+
deadline+memory-cap). **PARITY-CLUMPING:** systematic-encoder-sırası repair-kümeler → repair-zaman-serpiştirme
(pacer-içinde), latency+memory-ile-ölçülür.

## 8. CONTROLLER + WIRE-BUDGET (SoC: controller-transport-core'da, kripto-agnostik)
**P0-1 INVARIANT (RFC 9265, birim-doğru — go/no-go-§3.3):**
```
scheduling_horizon = min(block_deadline_remaining, controller_interval, pacing_planning_horizon)
allowed_wire_bytes = min(pacer_B, cc_B, operator_B, headroom_B)   # HEPSİ-BYTE (bps/window/byte-KARIŞTIRMA)
free_wire_bytes = allowed_wire_bytes − queued_new_source_B − queued_retransmit_B − queued_repair_B − reserved_control_B
INVARIANT: new_source_B + RETRANSMITTED_source_B + repair_B + control_B ≤ allowed_wire_bytes  (pacer-admission+burst-cap)
```
repair: pacer-bypass-YOK · cwnd-dışı-YOK · ayrı-sınırsız-kuyruk-YOK · control-aç-BIRAKMAZ · burst_cap. Model-A-
default (repair↑→source↓); Model-B (kanıtlı-headroom) Alpha-DIŞI.
**CONTROLLER-FORMU (birim-doğru — go/no-go-§3.3):**
```
profile = choose_profile(q_raw, burst_p95, reorder, reverse_health, deadline, target_block_failure_prob)
R_ratio_max = floor(K_actual × repair_ratio_cap); R_wire_max = floor(repair_budget_B / SEALED_LEN_per_repair)
R_send_target = min(R_requested, R_ratio_max, R_wire_max, R_code_max − R_esi_reserved_unique)
```
**SEALED_LEN-TABANLI-HESAP (P0-3-bağı):** controller payload-E-üzerinden-DEĞİL, noise-wire'ın-exact-sealed_len()-
sözleşmesi-üzerinden-hesap-yapar (kama-noise-wire-tdd-§sealed_len; seal-öncesi-KESİN-boyut). **R_SENT-SEMANTİK-
ZİNCİRİ (dördüncü-geçiş-§15.1 — "queue'ya-kondu"≠koruma):** R_generated → R_pacer_admitted → R_socket_submitted →
R_wire_sent → R_acked / R_received_reported → R_useful — telemetry-AYRI; "R_sent"-terimi-normatif-metinden-KALDIRILDI (reservation↔send-yarışı, analiz-P0-4/P0-2). **CONTROL-RESERVE (§15.4):** reserved_control_B statik-DEĞİL → minimum-servis-garantisi/weighted-
queue (yoksa high-FEC-state'te feedback-gecikmesi controller'ı-körleştirir).
K_FEC×q̂ (1.75-2.0): YALNIZ-başlangıç-tahmini, wire/kabul-kritere-GÖMÜLMEZ (binom: 2×q̂-reliability-sözleşmesi-değil).
**CAP-BOZUYORSA-SESSİZ-DEVAM-YOK:** R_send_target<R_required → profile_target_unmet=true → {lower-profile |
residual-ARQ | degraded/S&F | explicit-downgrade}, telemetry-görünür.
**DEADLINE-HEDEFİ (go/no-go-§5 — sub-kbps-KRİTİK):** serialization RTT'den-büyük (K=8,0.25kbps,96B→24.6s). Hedef =
deadline_miss_probability/expected_completion_time (target_residual_loss-DEĞİL). Deadline-sonrası-hedef-tutması-
BAŞARI-DEĞİL. Stale-q̂'da conservative-bounded-profile.
**ÇOK-BOYUTLU-SİNYAL (tek-q̂-yetersiz):** q_raw · burst_p50/95/max · reorder · rtt/queue-trend · ECN · reverse-
health · feedback_age · headroom · deadline · block-size. Forward/reverse-ayrı; proactive-repair-birincil (feedback-
sonrası-ek-repair reverse-çökükse-ARQ-round-ekler → yalnız-residual).

## 9. HYBRID ARQ+FEC + LIFECYCLE + RETENTION (transport-core, SoC-analiz + go/no-go)
**Durum:** OFF · ARQ_PRIMARY · HYBRID · FEC_PRIMARY · DEGRADED/S&F (histerezis: enter≠exit + min-dwell + emergency
+ staleness). **HYBRID:** proactive-R → deadline/yeterli-shard-decode → residual-compact-NACK/SACK → targeted-
source-retransmit-VEYA-unused-repair-ESI (deadline+wire-cost'a-göre, "residual→ek-repair"-OTOMATİK-DEĞİL) →
recovered-bitmap-raw-received'ten-AYRI. **BLOCK-LIFECYCLE:** PLANNED→OPEN→ENCODED→SENDING→DECODABLE→DECODED→
DELIVERED→{EXPIRED|ABORTED|EVICTED}. **RETENTION:** source-symbol-sakla-until{delivery/recovered-confirm | residual-
ARQ | deadline/TTL | abort | eviction}; sınırlar: max_retained_blocks/plaintext_bytes · retention_ttl · per-peer-
fair-share. **UNSEALED-SENSITIVE-SYMBOL-ZEROIZE (P0-9-kapsam-genişletme — 'plaintext'-terimi-SoC-sızıntısıydı):**
hassas = YALNIZ-source-DEĞİL: source-symbol-buffers + REPAIR-symbol-buffers (lineer-kombinasyon="parite-
plaintext-değil"-güvenli-KABUL-EDİLMEZ) + encoder/decoder-SCRATCH + retained-residual-source + reconstructed-
symbols + canonical-padding-buffers + Vec/clone/queue-geçiş-kopyaları. Politika: terminal-state-zeroize ·
reusable-bounded-pool · gereksiz-clone-YASAK · Zeroizing<Vec<u8>>-eşdeğeri · panic/error-path-zeroize ·
eviction/timeout-path-zeroize · crash-dump/swap-ürün-güvenlik-belgesinde · memory-cap-test · telemetry-içerik-
loglamaz. **ÜÇ-FEEDBACK:** network-
ACK (RTT/BW/loss) · recovered-delivery (retransmit-iptal, credit-YOK) · block-completion (retention-serbest). **LATE/
DUP:** FEC-recover-sonrası-original: wire-muhasebe + logical-ESI-duplicate + app'e-2.teslim-YOK + rank-artmaz.
**FEEDBACK-AGE:** sender-local time_since_last_ACCEPTED_feedback (SoC: 'auth'-değil — kabul-yöntemi-secure-wire'ın) + covered_range/epoch (clock-sync-YOK).

## 10. CONGESTION + ESTIMATOR (transport-core — Y3-delivered-EAI-etkileşimi) [NORMATİF]
**SAYAÇLAR:** sent_new_source/retrans_source/repair/control · received_source/repair · acked_source/repair ·
lost_source/repair_pre_fec · recovered_source · unrecoverable_source · wire_delivered_B · app_delivered_B ·
block/message_completion_latency. **BtlBw-ÇİFT-SAYIM-YASAK:** recovered-source-byte + repair-byte İKİ-KEZ-sayılmaz
(1000B-repair→1000B-recovered→delivered=2000B YANLIŞ; wire=1000B). **RECOVERED↔BtlBw [NORMATİF — KARAR-VERİLDİ, PATCH-6]:** FEC-ile-reconstruct-edilen-application/source-byte'ları
wire_delivered_B / BtlBw / delivered-EAI'ye **YENİ-teslim-edilmiş-wire-byte-olarak-EKLENMEZ.** BtlBw YALNIZ-
gerçekten-gönderilmiş-ve-network-ACK-ile-teslimi-kanıtlanmış-SEALED-wire-byte'lardan-kredi-alır. RECOVERED-
feedback yalnız: logical-delivery + retransmission-cancellation + retention-release; **ACK-yerine-GEÇMEZ, bandwidth-
credit-ÜRETMEZ.** [UYUM-DOĞRULAMASI]: kod-farklı-davranıyorsa-KOD-düzeltilir. (ACK-compression: cumulative-ACK-
arrival-anı-spike-yaratmaz — delivered-EAI-send-time-interval-uyumu-korunur.) **RAW-LOSS-DENOMINATOR:**
received/resolved_lost/unknown-in_flight-AYRI (unknown-erken-girmez). source/repair-loss-AYRI (farklı-zaman/size).
**LOSS→CC:** raw-loss-her-zaman-görünür; public-path'te recovered-source-kaybı-CC-için-KAYIP. Model-B-özel-mode.

## 11. WIRE-BYTE/PMTU [NORMATİF — KARAR-VERİLDİ, PATCH-16] (İKİ-BELGE-ORTAK)
**Normatif-girdi-birimi (MTU=1200-belirsizliği-KAPANDI): `max_udp_payload_bytes`.**
```
max_udp_payload_bytes = effective_ip_path_mtu − ip_header_bytes − udp_header_bytes − tunnel_overhead_bytes
WIRE-ADMISSION:  sealed_len(packet) ≤ max_udp_payload_bytes
sealed_len (EXTENDED)        = 26 + encrypted_inner_len        # h_ext4+wf1+flags1+pn4+tag16 (§2.6-noise-wire)
sealed_len (legacy-SESSION_DATA) = 22 + plaintext_payload_len  # AEAD-expansion=yalnız-tag-varsayımı;
                                                               # farklıysa-EXACT-formül-N7-raporunda-güncellenir
```
Kurallar: **E-hiçbir-zaman-doğrudan-1200-DEĞİL** · IPv4/IPv6/tunnel-AYRI-hesap · IP-fragmentation'a-GÜVENİLMEZ ·
profile-shard-size: even-byte + mümkünse-64-multiple (reed-solomon-simd) + sub-kbps-serialization + deadline.
**MID-BLOCK-PMTU-SHRINK:** Profile.E-block-boyunca-IMMUTABLE; küçülüp-sığmazsa yeni-shard-GÖNDERİLMEZ → block-
abort/expire → kalan-data-KÜÇÜK-profile-YENİ-block. N7-raporu: ICMP-black-hole-fallback + padding-size-class-etkisi.

## 12. ENGINE-ENTEGRASYON-SÖZLEŞMESİ (SoC-analiz-§13, orchestration-katmanı)
**Engine/session'da-kalır (fec-core-DEĞİL):** OpenedTransportShard/UnsealedTransportShard-handoff · transport-
parameter-hash-binding · production-secure-wire-requirement · error-propagation · session-shutdown/key-update-
orchestration · negotiation-authenticated-binding. **Ortak-tipler:**
```rust
pub struct UnsealedTransportShard { pub logical: LogicalShardHeader, pub symbol: Bytes }  // DTO — herkes-üretebilir (schema)
pub struct OpenedTransportShard { /* ALANLAR-PRIVATE — sealed (analiz-P0-5/P0-3) */ }
impl OpenedTransportShard {   // YALNIZ-kama-noise-wire::open()-üretir (_proof: PrivateAdmissionProof)
    pub fn logical(&self) -> &LogicalShardHeader; pub fn symbol(&self) -> &[u8];
}   // WireDeliveryMeta ayrı-getter'la-yalnız-transport-accounting'e; FEC-codec'e-VERİLMEZ.
```
**OWNERSHIP:** Unsealed=schema-crate (DTO); **Opened=noise-wire-crate** (sealed, public-constructor-YOK).
**TRANSPORT-PARAMETER-BINDING (SoC-analiz-§12):** negotiated-params → canonical-serialization → canonical-hash →
engine → Kama-Noise-handshake/prologue/channel-binding. FEC-modülü Noise-transcript'e-DOĞRUDAN-bağlanmaz.

## 13. DoS-SINIRI (kripto-agnostik — SoC-analiz-§14) [NORMATİF]
FEC-Core kriptodan-BAĞIMSIZ şu-sınırları-korur (Noise-doğruladıysa-sınırsız-güven-YOK; AEAD-authenticity resource-
trust-DEĞİL): max_open_blocks · max_block_bytes · max_K · max_R_code_max · max_symbol_size · max_decode_attempts ·
idle_timeout · retention_ttl · per-peer-quota · global-memory-cap · duplicate-ESI-rejection · strict-length-
validation · profile-allowlist. **Auth'lu-peer-de-compromised-olabilir.**

## 14. KABUL-KRİTERLERİ (kripto-agnostik + SoC)
- **K-FEC-SOC-1:** fec-crate-dependency-graph'ında Noise/AEAD/key-mgmt/identity-trust YOK.
- **K-FEC-SOC-2:** codec byte-plaintext/ciphertext-anlamını-BİLMEDEN aynı-test-vektörü-üretir (opaque-API).
- **K-FEC-1 (R-space-ayrımlı):** codec-doğruluğu K_actual+TÜM-R_code_max → herhangi-R_code_max-erasure. Runtime-
  decode unique_ADMITTED_source+unique_ADMITTED_repair≥K_actual (SoC: core-admission-yöntemini-BİLMEZ; 'auth'-terimi-secure-wire-katmanına-ait). Runtime-koruma=gönderilmiş+ulaşmış-distinct-repair-SINIRLI.
- **K-FEC-2 (controller):** q̂=0→parite=0 (temiz-sıfır-overhead); profile-selector (SABİT-FORMÜL-YOK); deadline-hedefli.
- **K-FEC-INV-1 (P0-1):** new_source+retrans+repair+control ≤ allowed_wire_bytes (birim-doğru, byte); repair-pacer-
  içinde+burst-cap+cwnd-dışı-YOK. **En-kritik-gate.**
- **K-FEC-UNIT-1:** ratio/bps-cap→shard-adedi-dönüşümü unit-test; farklı-birim-min()'de-YOK.
- **K-FEC-RSPACE-1:** R_code_max/R_initial/R_esi_reserved_unique/R_socket_submitted-ayrımı; code-space-hesabı-
  YALNIZ-R_esi_reserved_unique; sonradan-repair-aynı-code-space-decode; reserve-monoton-idempotent-testi.
- **K-FEC-WIRE-1:** iki-bağımsız-impl/golden-vector aynı-repair-byte **+ reconstructed-METADATA** (beşinci-geçiş: golden-vector-metadata-recovery'yi-de-kapsar).
- **K-FEC-DEADLINE-1:** profile-target-reliability-sağlasa-bile predicted/actual-deadline-aşarsa-SEÇİLMEZ.
- **K-FEC-PACER-2:** repair-average+burst-cap+pacing-gap; clumping-queue-spike-YOK.
- **K-FEC-LIFE-1 / K-FEC-RETENTION-1:** lifecycle-deterministik+memory-cap; retention+terminal-zeroize.
- **K-FEC-PARTIAL-1:** tiny/partial/final-padding+original_len-deterministik; cross-message-YOK; enumerated-shape.
- **K-FEC-CC-1:** raw-received/recovered/app-delivered-AYRI; recovered BtlBw-şişirMEZ (Y3-uyumlu).
- **K-FEC-ARQ-1:** FEC-recovered-source gereksiz-retx-YOK; congestion-loss-gizleMEZ (RECOVERED≠ACK).
- **K-FEC-EST-1:** unknown/in-flight raw-loss-denominator'a-erken-girmez; source/repair-loss-AYRI.
- **K-FEC-MTU-1:** IPv4/IPv6+tunnel'de tüm-datagram-PMTU-aşmaz (E≠1200; §11-ortak-sözleşme).
- **K-FEC-DOS-1:** auth-sonrası-da tüm-block/decoder/memory-sınırları.
- **K-FEC-OFF-1 (KIRMIZI-regresyon → §0.8-NON-REGRESSION'a-bağlı):** FEC-kapalı-hot-path allocation-yok/minimal +
  block-latency-yok + mevcut-pacing/ARQ-gate-DEĞİŞMEZ. **FEC-AÇIK-da:** mühürlü-kazanımlar-korunur (§0.8). Pin-seti:
  temiz-1k=45 · P5{100×12,S5} · DESEN-1-netem-4114→308 · qd-52.2s · DESEN-2-ramp · 0.25k-anchor · Y3-yakınsama ·
  desen1_sim.rs-5-pin BİREBİR. Paylaşılan-kod (bw.rs/sender.rs/send_rate-loss/ZAMAN-MODELİ) FEC-tarafından-YALNIZ-
  OKUNUR, değiştirilmez (§0.8-3). Her-FEC-PR bunlar-yeşil-olmadan-MERGE-EDİLMEZ.
- **K-FEC-NEG-1 (ZORUNLU-NEGATİF):** FEC-KÖTÜ-rejimleri-gösterilir (temiz-kapasite-dolu / congestion-loss / tiny /
  düşük-rate+derin-interleave / stale-q̂ / burst-R-aşan) → FEC-kapatır/kontrollü-split. Dürüstlük-kanıtı.
- **K-FEC-SUPPLYCHAIN-1:** exact-crate-version+checksum+license+SBOM+vendored/audited-source. **Registry-
  metadata-D0'da-EXACT-doğrulanır** (dördüncü-geçiş: belgedeki-sürüm/tarih-satırı-registry'yle-uyuşmalı).
- **K-FEC-META-1 (P0-2):** kaybolan-source'un-TÜM-gerekli-metadata'sı (msg/chunk-id, len, flags) recover-edilir
  (source-specific-metadata coded-prefix'te — TEK-BİÇİM §2.6, MADDE-12; derivable-YALNIZ-duplicate-azaltma); golden-vector reconstructed-metadata'yı-test-eder; tek-pakete-bağımlılık-YOK.
- **K-FEC-RSENT-1:** R_generated/pacer_admitted/socket_submitted/wire_sent/acked/received_reported/useful-zinciri
  telemetry-AYRI.
- **K-FEC-SEALED-1 (P0-3-bağı):** controller/pacer-hesabı exact-sealed_len()-üzerinden (payload-E-tahmini-DEĞİL).

## 15. fec-core-D0-A (dördüncü-geçiş-revize-plan: D0-A-paralel → D0-B-ortak-gate → D1-yalnız-gate-geçerse)
**GO (hemen, kripto-agnostik):**
- reed-solomon-simd saf-codec-POC (15-test): deterministik-repair · source/repair/mixed-erasure · R_code_max-space ·
  R_initial-runtime · unused-ESI-sonradan · duplicate-ESI-rank · wrong-profile-fail · shard-alignment · golden-
  vector · x86/AArch64-aynı · fuzz · CPU/memory-warmup.
- profile/golden-vector · block-lifecycle · logical-identity (block_id/ESI/dedup/rank) · resource-bounds.
- fec.rs-envanter (▼KOD-ZEMİNİ-1 [UYUM]: FEC_SHARD-0x20-yolu-EXTENDED-class-kararına-uyuyor-mu; block-sınırı=message/chunk-KARAR-P3) · wire-byte: §2.6-noise-wire-layout'tan-EXACT (tahmin-değil).
**FEC-tarafı-D0-A-ek (dördüncü-geçiş):** canonical-source-symbol-formatı (P0-2-prefix) · metadata-recoverability-
tablosu (18.5) · R-code-space-POC (unused-ESI-sonradan, canonical-recovery-index-mapping, non-64-multiple-shard-
uyumu, even-padding, small-K/R-init-maliyeti, duplicate-ESI, wrong-count-fail-closed, max-profile-memory, crate-
update-golden-drift-gate).
**WIRE-TASARIMI-KİLİTLİ (karar-VERİLDİ):** symbol-boyu (E, §2.6-noise-wire) · PMTU (max_udp_payload_bytes, §11) ·
sealed_len-formülleri · packet-enum (§2.56-inner_kind). D1-ENTRY-şartı = N1-N10-UYUM + POC (kilit-değil-doğrulama).
**D0-B-ORTAK-GATE:** normatif-liste = belge-sonundaki-KAPANIŞ-TABLOSU (16-gate, TÜMÜ-CLOSED — iki-belgede-byte-for-byte-aynı). **D1-yalnız-gate-geçerse:** codec-
adapter + fixed-enumerated-profiles + static-controller + no-interleaving + no-Model-B + targeted-residual-ARQ +
production-secure-wire-admission.

## 15.5 BELGE-KAPSAM-ALT-AYRIMI (dördüncü-geçiş-§13.4, P1 — SoC'yi-ileride-korumak)
Bu-belge iki-alt-alanı-birlikte-taşıyor; üçüncü-belge-ŞART-DEĞİL ama kapsam-ayrımı-AÇIK-olmalı (kod-modülleri-de-
mümkünse-ayrı): **FEC-Codec-Core** (RS-adapter / profiles / encode-decode / code-space — §2-6.5) vs **Transport-
Recovery-Controller** (ARQ-FEC-policy / pacing / lifecycle / feedback / CC-accounting — §8-10). Pacing/CC/hybrid-
ARQ saf-codec-DEĞİL, transport-recovery-orchestration.

## 15.6 CROSS-DOCUMENT-BAĞIMLILIK: HANDSHAKE-PAKET-SURVIVAL (beşinci-geçiş-§10 — kapsam-dışı-BIRAKILMAZ)
FEC+protected-data-path YALNIZ-session-kurulduktan-sonra-çalışır. **AYRIM:** handshake-PATTERN MÜHÜRLÜ (Noise-N-
türevi-one-way-zero-RTT — transport-layer-tdd, §0.8-non-regression-kapsamında, DEĞİŞMEZ); açık-olan yalnız o-mühürlü-
pattern'in hostile-PAKET-SURVIVAL'ı (A/B/C-ayrık: INIT-teslimi=FORWARD-loss-işi; confirmation/FS-upgrade=REVERSE; B-gecikebilir, C-tolere) = kama-noise-wire-tdd-§0.7-B-P0'ı.
Bu-belge-ona-BAĞIMLI (data-path-güçlü-olsa-da session-kurulamazsa-anlamsız). **Bootstrap-FEC = Alpha'da-REDDEDİLDİ** (survival=bounded-idempotent-INIT-flight-machine, kama-noise-wire-§0.7-B;
bootstrap-FEC → Beta/ayrı-araştırma). Handshake-paket-survival = mevcut-PRE_CONVERGE/KEYSTONE-cold-start-işiyle-
KESİŞİR — D0'da-birleşik-UYUM-doğrulaması (yeniden-tasarım-değil).

## 16. KAPSAM-DIŞI
Streaming/convolutional(RFC 8681) · dinamik-interleaving · real-time-media · multi-path · fountain(METTLE-referans) ·
unequal-error-protection · cover-traffic · sync-path-FEC. **TÜM-KRİPTO → kama-noise-wire-tdd** (Noise/AEAD/nonce/
replay/key-phase/Cryptex/verify-before-admission). 2.aşama/Beta/ayrı-belge.

---
**SONRAKI:** bu-belge-Abkarada-onay → fec-core-D0a (crate-POC+golden-vector, kripto-YOK) ∥ noise-wire-D0a (KRİTİK-
YOL) → ortak-entegrasyon-gate → D1. **fec-core bağımsız-test-edilebilir (sim-first-korunur).**

## D0-B ORTAK-GATE KAPANIŞ-TABLOSU [iki-belgede-BYTE-FOR-BYTE-AYNI — PATCH-G]
| Gate | Karar | Durum |
|---|---|---|
| Shared schema ownership | kama-transport-schema yalnız unsealed DTO/primitives | CLOSED |
| Opened ownership | Yalnız kama-noise-wire, private constructor | CLOSED |
| Packet enum | FEC source/repair + listed secure controls | CLOSED |
| Canonical source metadata | Source-specific metadata coded prefix'te | CLOSED |
| K_actual | Her authenticated FEC shard inner header'ında | CLOSED |
| sealed_len | Exact per-class formulas (EXT=26+inner, legacy=22+payload) | CLOSED |
| SealedPacketMeta | PN→logical deterministic mapping | CLOSED |
| Control protection | Bütün control AEAD encrypted (allowlist) | CLOSED |
| Effective PMTU | max_udp_payload_bytes | CLOSED |
| ID widths | profile u8, block/message u32, ESI u16, K_actual u8 | CLOSED |
| PN demux | YOL-1, legacy/ext handles | CLOSED |
| INIT replay/restart | Persistent init anti-replay; restart=new session | CLOSED |
| dh_epoch terminology | Key-phase/rekey dili Alpha'dan kaldırıldı | CLOSED |
| Parameter binding | DURUM-B, PARAM_SET/PARAM_ACK | CLOSED |
| Packet survival | Bounded idempotent INIT retransmission | CLOSED |
| AAD | Canonical EXTENDED_V1 serialization | CLOSED |
| HANDLE_ASSIGN | Wire-frozen layout; hub-authenticated; hub-sıralama-garantisi | CLOSED |
| PARAM admission | Bootstrap-profile; PARAM_ACK=confirmation, kapı-değil | CLOSED |
| dh_epoch transition | 5-adım confirmation-safe machine; grace=max(4×sRTT,64pkt) CAL | CLOSED |
| K-width invariant | 1≤K_actual≤K_shape≤255; E≥11; fail-closed | CLOSED |

**Tüm-gate'ler-CLOSED → FULL-DESIGN-GO.** (Önceki-"11-gate"-sayımı-alt-kararların-birleştirilmesiydi; normatif-
liste-BU-tablodur.)

## STATÜ-MERDİVENİ [PATCH-H]
**FULL-DESIGN-GO (BU-BELGELER):** üç-wire-blocker-kapatıldı (h_ext-HANDLE_ASSIGN-§2.55 · epoch-aware-PN/ACK-§4/§2.57 · control-ACK-türleri-§2.57) + 14-madde-kapanış (h_ext-§2.55 · inner_kind-registry-§2.56 · control-body-§2.57 · PN-pseudocode-§4 · symbol_len==E-§2.6 · INIT-byte-for-byte-retry-§P0-5 · key-phase-temizliği-§5) + PATCH-1..16+G işlendi · altı-Noise-kararı-CLOSED · normatif-belirsizlik
("veya/seçilebilir/MI?/KARAR-ÖNERİSİ")-kaldırıldı · gate-tabloları-aynı · byte-layout-tek-otorite. → **D1-
implementation-BAŞLAYABİLİR.**
**D1-IMPLEMENTATION-GO (kod-envanteri+POC-sonrası):** N1-N10-tamam · seq16-nonce-değil-full-PN-reconstruction-
uygulanıyor · RS-POC+golden-vectors-geçti · known-zero+metadata-recovery-testleri-geçti · seal(p).len()==
sealed_len(p)-tüm-class'larda · sealed-type-compile-time-enforce · PMTU/profile-tablosu-gerçek-byte-ölçümüyle ·
supply-chain-doğrulandı.
**PRODUCTION-GO (D1+testler-sonrası):** fwd/rev-hostile-bootstrap-matrisi · PN-wrap/reorder/replay-exhaustive ·
AEAD-failure-state-poisoning · forged-ACK/RECOVERED/control · restart+replay-persistence · IPv4/IPv6/tunnel-PMTU ·
FEC-ON/OFF-non-regression-pin-seti · deadline/burst/congestion/negative-regime · memory-cap/zeroization/dup-ESI/
compromised-peer-DoS · iki-impl-golden-vector-byte-eşitliği · security-review+fuzzing. **Bunlar-geçmeden
"production-ready/hostile-ready/anti-jamming/blackout-resistant"-ifadeleri-KULLANILMAZ.**
