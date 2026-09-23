# kama-noise-wire-tdd — Kama Noise Secure-Wire Profile [v2.0 — dört-karar-boşluğu-KAPATILDI (HANDLE_ASSIGN-frozen · bootstrap-profile-admission · confirmation-safe-epoch · K-invariant) + öz-tarama-temiz; FULL-DESIGN-GO-onayı: Abkarada]

> **YAMA KAYDI — 2026-09-23 [D-GAP-1 bağı]:** §4 anti-replay sabit-kapsamı güncellendi — legacy-seq16 yolunun `MAX_FUTURE_GAP`/`MAX_REORDER` kapıları ccr §35.8'de KALDIRILDI (far-future reddi yok; yapısal sınır |cand−expected| < 2^15, gönderen kısıtı KS-PN-1). extended-pn32 yolunun `FUTURE_GAP_MAX = 2^13` kapısı DOKUNULMADAN durur — kapsam sorusu identity OQ-16. **[R-2 KAPANDI — 2026-09-23, sahip kararı]** İki kopya BİRLEŞTİRİLDİ: 19 Eylül içeriği otoriterdir ve bu dosyaya taşındı; `kama-noise-wire-tdd-v2_0.md` SİLİNDİ. Bu dosyanın önceki (21 Temmuz) içeriği SUPERSEDED'dir ve git geçmişinden geri alınabilir: `git show 311b5c2d:specs/kama-noise-wire-tdd-v2.0.md`. Kaybolan yalnız eski sürümlerdi (ör. `seal(..., wire_pn)` imzası — PN sahipliği KM-SEAM-TC-001 K2 ile değişti; eski GRACE formülü `max(4×sRTT, 64pkt)` — KS-GRACE-1 ile değişti). Repodaki tüm atıflar zaten bu adı kullanıyordu.


> **YAMA KAYDI — 2026-09-18 [AMENDMENT-6]:** 16 Eyl sökümleri GERİ ALINDI (yanlış öncül): §2.5 tek-PN-uzayı hükmü geçerli (session verisi + FEC shard'ları + E2E kontrol tek E2E cipher state); hub-yönelik trafik (HANDLE_ASSIGN, engine ACK, Plane 1) ayrı ProtectionContext::HubChannel · `wire_pn` API satırı geri alındı, PN sahipliği S3 kararı olarak işaretlendi (kod: caller-supplied, PeerSlot'ta sıfırlanır, wrapping_add; öneri: seam kendi tutar) · SealedPacketMeta notu kaldırıldı · kanun bandı kaldırıldı → transport-tdd §0 referansı. §6/§7 (verify-before-admission, per-shard AEAD) AYNEN — kanonik mimarinin kendisi.


> **Katman grafiği ve kanun:** transport-layer-tdd §0 INV-NEIGHBOR-1 (tek otorite; bu belge kopyalamaz).

> **YAMA KAYDI — 2026-09-07b:** Belge başındaki 09-02 yama kaydı §2.5'in askı durumuyla ÇELİŞİYORDU (ANAHTAR-ÖZDEŞLİĞİ / REPLAY-STATE-PAYLAŞIMI'nı normatif gibi özetliyordu) → askı işaretiyle hizalandı; S2-noise.2'nin kesin/açık bölümü açıkça yazıldı. Tasarım değişikliği YOK.


> **YAMA KAYDI — 2026-09-07 (CRYPTO-P0-FREEZE):** Bu belgede YENİ TASARIM YAPILMADI. Yalnız cross-reference notu: Identity/CCR'nin P0-6 (replay commit ordering) ve P0-9 (confirmation-safe dh_epoch) kararlarına hizalandığı kaydedildi.


> **YAMA KAYDI — 2026-09-02 (S2 blocker kapatma, B3):**
> - **§2.5'e iki normatif blok eklendi** (yeni karar DEĞİL; mevcut kararların açık yazımı):
>   **ANAHTAR-ÖZDEŞLİĞİ** ve **REPLAY-STATE-PAYLAŞIMI** blokları.
>   ⚠ **BU İKİ BLOK 2026-09-07'DE ASKIYA ALINDI — NORMATİF DEĞİL** (bkz. §2.5 ve aşağıdaki
>   2026-09-07 yama kaydı). Bu yama kaydındaki eski özet metin, §2.5'in askı durumuyla
>   ÇELİŞMEMESİ için buradan kaldırıldı; iddiaların kendisi §2.5'te askı işaretiyle durur.
>   **S2-noise.2 AÇIKTIR:** SESSION_DATA → E2E key (kesin) · HANDLE_ASSIGN/ACK → hub-channel key
>   (kesin) · transport ACK/SACK/RECOVERED/PARAM_* ve FEC source/repair shard → **AÇIK**.
> - İlgili karşı-yama: transport-layer-tdd §0.1.
> - §0.9 mühürlü kripto listesindeki hiçbir kalem değiştirilmedi.


> **STATÜ — DR-4 CLOSED (KOŞUL-A / LE64)** [Önceki "GEÇİCİ STATÜ OVERRIDE — DR-4 AÇIK" bloğu, DR-4-envanteri
> KOŞUL-A(2)-KESİN-sağladığı için otomatik-kilitle kaldırıldı — karar-paketi GÖREV-2.]
> - **Cryptographic-profile freeze: CLOSED** — AEAD-nonce96=LE64 (§4.5); FULL-DESIGN-GO yeniden-aktif (16-gate+nonce96=CLOSED).
> - **Sprint-4: GO** — DR-4-CLOSED sonrası (session/HANDLE_ASSIGN/dh_epoch-machine/INIT-flight/restart; D1-promptu-§5).
> - **Sprint-5: HOLD** (bağımsız-gate'ler: N1-legacy-interop + cross-crate-integration) · **Production: NO-GO** (testler-tamamlanmadı).
> - Sprint-durum-otoritesi: karar-paketi BÖLÜM-5. Bu-holdlar-tasarım-gate-değil-süreç-gatetir.

> **ONAY (Abkarada, resmî):** Bu-belge D0-çalışma-talimatı-olarak **ONAYLANDI.** Riskler-doğru-yere-taşınmış. **GO:**
> mevcut-Noise-kod-envanteri · PN/nonce/replay-analizi · hostile-handshake/bootstrap-tasarımı · exact-sealed-length/
> PMTU-ölçümü · secure-control-frame-tasarımı · shared-schema-tasarımı. **STATÜ (MADDE-1 — tek-merdiven):** wire-
> tasarımı-KİLİTLİ (§2.6/2.57-byte-layout, §2.8-AAD, §4-PN-pseudocode). **D1-ENTRY-NO-GO:** N1-N10-UYUM + POC-öncesi
> production-Noise-packet-koduna-başlanmaz. **PRODUCTION-NO-GO:** testler-tamamlanmadı. **TÜM-KARARLAR-CLOSED (FULL-DESIGN-GO):** PN-tek-space+32-bit (§2.5) · key-lifecycle=cap→re-handshake (§5) ·
> (1)survival=bounded-idempotent-INIT-flight-machine (§0.7-B) (2)PN-demux=YOL-1 h_legacy/h_ext (§2.5) (3)binding=
> DURUM-B PARAM_SET/ACK (§8) (4)byte-layout=§2.6-kilitli (5)control=§2.7-allowlist (6)AAD=§2.8-canonical.
> Handshake-PATTERN-MÜHÜRLÜ (§0.7-A). Normatif-gate-listesi=belge-sonu-KAPANIŞ-TABLOSU (16×CLOSED).
> **EK-KISIT (§0.9): mevcut-mühürlü-L4-Noise-split + N-türevi-handshake-pattern-DOKUNULMAZ.**

> **Statü:** TASARIM-SÖZLEŞMESİ (production-wire-koruma-katmanı). **v1→v1.1 (dördüncü-geçiş):** SoC-bölünmesi-
> ONAYLANDI ama arayüz-boşlukları-P0: bu-belgeye-düşenler = packet-ENUM (data+CONTROL — P0-1, en-keskin: sahte-
> recovered/ACK-saldırı-yüzeyi) + sealed_len-EXACT-sözleşmesi (P0-3) + SealedPacketMeta-send-record (P0-4) +
> PN-tasarımı (P0-5→§4-pseudocode-KİLİTLİ) + key-update-Alpha-KARARI (P0-6→§5: in-session-rekey-YOK, cap→re-
> machine, "sonra-senkronize-ederiz"-KABUL-EDİLMEZ) + binding-zamanlaması (P0-7) + kama-transport-schema-crate
> (P0-8).
>
> **v1.1→v1.2 (beşinci-geçiş, crypto-hostile):** EN-KRİTİK-YENİ-P0 = **HANDSHAKE/BOOTSTRAP-HOSTILE-AÇIĞI (§0.7)** —
> data-path-güçlü-olsa-da Noise UDP-handshake-loss/out-of-order/fragmentation/anti-amplification/jammer-direncini-
> HAZIR-ÇÖZMEZ → forward-loss-INIT-teslimini (A), reverse-loss-confirmation/FS-upgrade'i (B/C) etkiler; B-gecikebilir, C-gerçekleşmeyebilir — epoch-0-usable-kalır (§0.7-A/B/C). + Noise-"hazır-
> UDP-secure-transport-DEĞİL"-normatif-vurgu + AAD-canonicalization + session-restart/PN-persistence + key-trial-
> timing-side-channel + AEAD-failure-state-etkisi. **Köken:** Abkarada'nın crypto-separation-analizi
> (SoC) v1.2-FEC-TDD'sini böldü; TÜM-kripto-normatifleri (per-shard-AEAD, nonce, PN-reconstruction, replay, key-
> phase, REKEY, Cryptex-metadata-yerleşimi, verify-before-admission) buraya-taşındı. **Bu belge = Kama'nın TEK-
> KAPALI production-güvenlik-profili** — generic-crypto-provider-DEĞİL.
>
> **NORMATİF-KİLİT (SoC-analiz-§15):** Kama YALNIZ kendi-normatif-Noise-profilini, packet-number-modelini, replay-
> politikasını, Cryptex-inspired-inner-envelope'unu destekler. Alternatif TLS/DTLS/IPsec/rastgele-AEAD/3rd-party-
> crypto-provider KAPSAM-DIŞI. **Yasak-genişleme:** `trait CryptoProvider/CipherSuiteProvider/GenericSecureAdapter`
> — algoritma-çevikliği + downgrade-yüzeyi + denetim-matrisi-katlanması. Engine DOĞRUDAN-somut-KamaNoiseWireProtector-
> kullanır.
>
> **İKİ-BELGE-ARAYÜZÜ:** UnsealedTransportShard (seal-girişi) / OpenedTransportShard (open-çıkışı) + ortak-wire-
> budget/PMTU. fec-core bu-belgenin-iç-durumunu BİLMEZ. **KRİTİK-YOL:** fec-core-D0'a-paralel ama symbol-boyu/PMTU/
> admission/wire-controller/shard-arayüzü [tarihçe-notu: bu-kısıt-KAPANDI — wire-tasarımı-artık-KİLİTLİ, §2.5-2.8].
>
> **Zemin:** Noise-Protocol-Framework (noiseprotocol.org). Standart: RFC 9335 (Cryptex-ANALOJİ) · RFC 9001/9002
> (QUIC-PN/key-phase-emsali) · RFC 9265 (repair-congestion). Mevcut: Kama-Noise N-türevi-SR_SM3-one-way-handshake (§0.7-A; identity-and-trust-
> model), SESSION_DATA-zarfı (hub_handle-4B-cleartext + seq-2B + ciphertext + AEAD-tag-16B). **▼KOD-ZEMİNİ tüm-belge-
> boyunca: mevcut-Noise-envelope korunur.** [tarihçe-notu: wire-tasarımı-artık-KİLİTLİ §2.5-2.8; N1-N10=UYUM.]

## 0. Bu-katmanın-sorumluluğu (SoC-analiz-§5)
KamaNoiseWireProtector Kama-tarafından-SABİTLENEN: Noise-handshake-pattern · AEAD-algoritması · hash/KDF · wire-PN-
modeli · truncated-PN-reconstruction · replay-window · dh_epoch (0/1, in-session-key-phase-YOK §5) · Cryptex-inner-envelope-formatı ·
transport-parameter-binding ile-çalışır. **[PN SAHİPLİĞİ — KAPANDI 2026-09-19, KM-SEAM-TC-001 K2]** PN sayacını **Crypto tutar**; `SealArgs.wire_pn` **kaldırılır**, `SealArgs` → `{ctx: ProtectionContext, dh_epoch, send_generation}` (transport v1.x API değişikliği, wire değişikliği değil). Gerekçe (kod kanıtı): eski model PN'i çağırandan alıyordu (seam.rs:13), transport `tx_pn` PeerSlot yaşamına bağlı sıfırlanıyor (runtime.rs:2274, idle 105 s) ve `wrapping_add` ile dönüyordu (:1008) — gerçek anahtar altında nonce tekrarı. Sayaç Crypto'da olunca transport'un hiçbir reset/slot/wrap olayı nonce güvenliğine dokunamaz; `SEND_PN_CAP` (protector.rs:249) → `RenewRequired`. Transport `seq16/pn32`'yi kendi ARQ'su için wire'dan **okur**, üretmez. ~~Eski: "caller-supplied PN"~~. **API:** `seal(opaque_datagram) → packet` / `open(datagram)
→ OpenedTransportShard`. Engine: `let p=seam.seal(ProtectionContext::Session(h), shard)?` / `let o=seam.open(dg)?` — PN'i Crypto tutar, çağıran vermez (K2). ~~Eski: `seal(shard,pn)`~~

## 0.9 L4-NOISE-SPLIT NON-REGRESSION (Abkarada-kısıtı — MÜHÜRLÜ-KRİPTO-DOKUNULMAZ) [NORMATİF, EN-ÜST-ÖNCELİK]
noise-wire L4-session-crypto'ya-EN-YAKIN-katman → en-dikkatli-olunacak-yer. **MEVCUT-MÜHÜRLÜ-KRİPTO DEĞİŞMEZ:**
**Noise N-türevi one-way zero-RTT HANDSHAKE-PATTERN** (ss_e=DH(sender_eph,recip_static) + ss_s=DH(sender_static,
recip_static); cert-ref-~48B / cert-inline-+152B; SESSION_INIT_RESPONSE-asenkron-FS-upgrade; interaktif-KK/IK-DEĞİL;
kapalı-sistem-TLS-1.3-değil — §0.7-A) · Noise-Split() + asynchronous-dh_epoch-upgrade (epoch-0→1) · SESSION_DATA-
HOT-PATH-ZARFI ([hub_handle 4B][seq 2B][ct][tag 16B]=22B-overhead, length-implicit; ccr-format-spec §35 = canonical-
otorite; INV-FLAGS-1/INV-MAGIC-1/INV-RESUME-1/INV-WIRE-2; magic-bytes-TBS'te; nonce-64-bit-counter+window-1024) ·
SESSION_INIT/SIGNED_CONTROL-canonical-bytes · KEYSTONE-Noise-AEAD-probe · PRE_CONVERGE-cold-start. **Bu-envelope
ccr §35'te-MÜHÜRLÜ + test-vektörlü — noise-wire-onu-OKUR; FEC-shard-seal AYRI-wire-tipi (envelope-bozulmadan-yanında).** **noise-wire bunları OKUR + ÜZERİNE-FEC-shard-seal-katmanı-ekler; YENİDEN-TANIMLAMAZ.**
D0-N1-N10-raporları bu-mevcut-yapıyı-ENVANTERLER (değiştirmez); wire-değişikliği-GEREKİYORSA (örn seq-2B→32-bit-PN)
→ AYRI-DECISION + Abkarada-onayı + mühürlü-handshake/probe-testleri-yeniden. Sessiz-L4-değişikliği-YASAK.

**DR-CS-1b-A2-AMENDMAN A-4 [Abkarada-onaylı; dar-kapsam — protector send-state-retirement]:** Session-close
tombstone/grace-yaşam-döngüsü (K-1: "send-cipher grace'te YAŞAMAZ; recv-cipher yalnız-grace") için
`KamaNoiseWireProtector`'a YALNIZ şu additive cfg-gated API eklenir: **`drop_send_state()`**. Normatif
postcondition'lar (Abkarada; aynen):
```text
- yalnız lifecycle=Closing altında çağrılır
- tüm send-direction secret/key/cipher state zeroize edilir
- tüm send PN/nonce allocation state irreversibly retired edilir
- recv cipher ve recv ReplayWindow byte-for-byte korunur
- sonraki seal denemesi typed SendStateRetired döndürür
- sessiz reinitialization yasaktır
- tekrar çağrı idempotent veya typed AlreadyRetired olur
```
Yalnız `send_cipher=None` yazmak YETMEZ — **retired-state yapısal olarak temsil edilir** (typed-durum;
sessiz-reinit yapısal-imkansız). Wire-layout, codepoint, AEAD-algoritması ve **receive-path DEĞİŞMEZ**;
bu amendman §0.9-mühürlü-kripto listesindeki hiçbir kalemi yeniden-tanımlamaz.

## 0.7 HANDSHAKE-PAKET-SURVIVAL HOSTILE-AÇIĞI (beşinci-geçiş-§10 — YENİ-P0; handshake-PATTERN-DEĞİL) [NORMATİF]

**ÖNEMLİ-AYRIM (Abkarada-düzeltmesi — iki-farklı-katman-KARIŞTIRILMAZ):**

**(A) HANDSHAKE-PATTERN = MÜHÜRLÜ, AÇIK-KARAR-DEĞİL (§0.9-non-regression-kapsamında):** Transport-handshake
ZATEN-TANIMLI-ve-KANITLANMIŞ: **Noise N-türevi one-way zero-RTT** (interaktif-KK/IK-DEĞİL — tek-mesaj, Bob-offline-
tolere). ss_e=DH(sender_eph, recipient_static)→FS · ss_s=DH(sender_static, recipient_static)→sender-auth(possession,
per-mesaj-imza-yok) · cert-ref(~48B-core, static-cached) / cert-inline(+152B-CompactDeviceCert) · SESSION_INIT_
RESPONSE=Bob-online-olunca-asenkron-FS-upgrade(epoch-0→1, ilk-mesaj-RESPONSE-olmadan-teslim+okunur) · nonce-64-bit-
counter+window-1024+split()→k_send/k_recv. **Bu KAPALI-SİSTEM (TLS-1.3-açık-negotiation-DEĞİL) — peer-static-key-
önceden-bilinir(cert-pinned) → zero-RTT-mümkün.**
**CANONICAL-ALANLAR (ccr §30.4b-test-vektörü + §35.2, MÜHÜRLÜ):** SESSION_INIT_REF (121B-app-envelope; magic-
"SSI1"-AAD'de): flags-1B (bits0..1-cert_mode=00-CERT_REF_32; bits2..3-init_variant=00-**SR_SM3_AUTHENTICATED_
ONE_WAY_DEFAULT**; reserved=0) · recipient_dest_hash-32B (cleartext-routing) · sender_ephemeral-32B (e) ·
sender_cert_ref-32B (SHA-256-canonical-cert) · init_counter-u64_be (dh-epoch-0) · ct(early)+tag-16B. Token-dizisi-
Noise-diliyle `→ e, es, ss` + payload (K-benzeri; inline-cert=X-benzeri) AMA **INV-NO-NOISE-PATTERN-FLAG: generic-
Noise-pattern-selector-DEĞİL — Kama-özel-SR_SM3-schedule (ccr §35.2-otoriter).**
**FS-NÜANSI (KESİN-BEYAN, mislabel-düzeltmesi-mühürlü):** ss_e per-INIT-**ephemeral-confidentiality**-sağlar,
klasik-FS-DEĞİL — recipient-static-sonradan-ele-geçerse epoch-0-initial-packet-ÇÖZÜLÜR (e-public-wire'da). **FULL-
FS yalnız-epoch-1'de** (SESSION_INIT_RESPONSE, Bob-ephemeral). Ürün-dili-buna-uyar.
**BOOTSTRAP-BAŞARI-ÜÇ-AŞAMA (P0-7-kriteri — "tamamlanır"-tek-kavram-DEĞİL):**
```
A RECEIVER_ESTABLISHED : receiver-INIT'i-aldı+doğruladı+early-data-kabul → epoch-0-USABLE (ilk-mesaj-RESPONSE'suz-
                          teslim+okunur — mühürlü). FORWARD-loss'a-bağlı; reverse-%90-ETKİLEMEZ.
B SENDER_CONFIRMED     : sender-session-kuruldu-öğrendi (hayatta-kalan-ilk-ACK/RESPONSE). REVERSE-bağlı; %90'da-
                          GECİKİR — bounded-idempotent-INIT-retry + eventual-confirmation.
C FS_UPGRADED          : RESPONSE-ulaştı, dh-epoch-1-aktif. REVERSE-bağlı; gecikebilir/gerçekleşmeyebilir —
                          tasarım-TOLERE (epoch-0-usable; güvenlik-beyanı-yukarıda).
K-WIRE-BOOTSTRAP-1-hedefi: A-forward-bounded-retry · B-eventual · C-asenkron-tolere. Test-matrisi fwd/rev-AYRI:
{0/90, 90/0, 90/90, burst/clean, clean/burst} × {A,B,C}-beklentileri.
``` QUIC-üstünlüğü: QUIC-handshake-hostile'da-RTT-yer; N-türevi-zero-
RTT-yemez. **Bu-pattern DEĞİŞMEZ (§0.9); noise-wire-onu-OKUR, yeniden-seçmez.** (Senkron-PTT/voice-interaktif-IK/KK
= Beta-ayrı-konu.)

**(B) HANDSHAKE-PAKETLERİNİN-HOSTILE-SURVIVAL'ı — PROBLEM STATEMENT (CLOSED by (B)-MEKANİZMA-KARARI, PATCH-8, aşağıda) (pattern-değil, o-mühürlü-pattern'in-PAKET-teslimi):**
mühürlü-N-türevi-handshake'in-*paketleri* hostile-ağda (loss/reorder/fragmentation) nasıl-hayatta-kalır? Noise-
kriptografik-framework-sağlar ama PAKET-SURVIVAL'ı-hazır-çözmez: UDP-handshake-loss-recovery · out-of-order · frag ·
anti-amplification · jammer-direnci. → mühürlü-pattern-güçlü-olsa-da reverse-%90-loss'ta SESSION_INIT-paketi-hiç-
ulaşmayabilir → session-kurulamaz. **Bu, pattern-eksikliği-DEĞİL, o-pattern'in-hostile-PAKET-teslim-katmanı.**

**(B)-MEKANİZMA-KARARI [NORMATİF — KARAR-VERİLDİ, PATCH-8]: Bounded-idempotent-SESSION_INIT-retransmission-
flight-state-machine.** Bootstrap-FEC Alpha'da-KULLANILMAZ (Beta/ayrı-araştırma). Kurallar:
· Aynı-SESSION_INIT aynı-replay-identity-ile-yeniden-gönderilir; her-retry yeni-session-state-VEYA-ikinci-app-
  delivery-ÜRETMEZ. Receiver duplicate-INIT'i-mevcut-session'a-map-eder, confirmation'ı-yeniden-gönderebilir.
· Retry-sayısı + toplam-retry-süresi + backoff-profili SABİTLERLE-bounded (D0-raporu-sabitleri-ÖLÇEREK-kilitler;
  mekanizma-yeniden-seçilmez).
· SESSION_INIT IP-fragmentation'a-GÜVENMEZ; max-bootstrap-datagram < max_udp_payload_bytes.
· Unauthenticated-input sınırsız-response/amplification-ÜRETMEZ; cookie/return-routability YALNIZ-gerçek-
  amplification/reflection-yüzeyi-varsa-etkinleştirilir.
· Forward-loss A-aşamasını (RECEIVER_ESTABLISHED: forward-%90'da-INIT-hiç-ulaşmayabilir → session-kurulamaz),
  reverse-loss B/C-aşamalarını (SENDER_CONFIRMED-gecikir; FS_UPGRADED-gerçekleşmeyebilir) etkiler.
**BAĞ [DR-2-NETLEŞTİRME]:** PRE_CONVERGE/KEYSTONE bu-katmanla-yalnız-ANALOJİ-ilişkisindedir, ÖZDEŞ-DEĞİL —
PreConverge = CC-cold-start (data, tavansız, yeni-seq); INIT-flight = AYRI-güvenlik-kritik-katman (cached-byte-for-
byte, bounded, anti-amplification). KEYSTONE-probe INIT-flight'ın-parçası; PreConverge'e-bağlanmaz.
**D0-çıktısı = mühürlü-handshake'in hostile-paket-teslim-KARARI (pattern-DEĞİL).**

**EARLY-DATA-REPLAY/IDEMPOTENCY (analiz-P0-5 — init_counter-tek-başına-YETMEZ) [NORMATİF]:**
```
INIT-replay-identity : (sender_device_id, recipient_device_id, init_counter)   [TEK-KİMLİK — transcript-hash-
                       alternatifi-KALDIRILDI, MADDE-5]
```
· **INIT-RETRY = CACHED-BYTE-FOR-BYTE-DATAGRAM-RETRANSMISSION (MADDE-5):** SESSION_INIT bir-kez-oluşturulur-ve-
  seal-edilir; retry'larda cached-datagram-byte'ları-AYNEN-yeniden-gönderilir. **Aynı-init_counter-ile-yeniden-
  serialization/padding-değişimi/yeniden-encryption YASAK** (nonce-reuse-riski). Payload-değişecekse: YENİ-
  init_counter + YENİ-ephemeral + YENİ-session-attempt. [Bu-bootstrap-istisnası established-FEC/data-retransmit'in
  "yeni-PN+yeni-nonce"-kuralından (§3) AYRIDIR.] · session-state-YENİDEN-KURULMAZ; app-message-İKİNCİ-KEZ-teslim-
  EDİLMEZ · established-session'da-duplicate-INIT → mevcut-session'a-map + confirmation-YENİDEN-gönderilebilir
  (yeni-state-YOK) · aynı-init_counter-FARKLI-sender-cert → REJECT · **RESTART-POLİTİKASI [NORMATİF — KARAR-VERİLDİ, PATCH-15]:** process-restart-sonrası mevcut-transport-
  session'lar RESUME-EDİLMEZ; yeni-handshake+yeni-session_id-ZORUNLU · sender init_counter'ı-cihaz-başına-
  KALICI+MONOTON-tutar · receiver peer/device-başına bounded-PERSISTENT-anti-replay (high-water/bitmap) tutar ·
  aynı-(sender_device_id, recipient_device_id, init_counter)-İKİNCİ-app-side-effect-ÜRETMEZ · replay-persistence-
  kullanılamayan-deployment'ta production-early-data-DEVRE-DIŞI · session-full-PN-diske-persist'ine-GÜVENİLMEZ
  (restart=her-zaman-yeni-session) · eski-session-datagramları yeni-handle/session_id-ile-admission-ALAMAZ ·
  replay-cache+TTL-tanımlı · revoked/eski-cert-replay → cert-doğrulama-reddeder.
**Production-early-data-admission NO-GO (bu-kurallar-kod-doğrulanana-dek); bootstrap-POC-GO.**

## 0.5 SEALED_LEN-SÖZLEŞMESİ + SEND-RECORD (P0-3 + P0-4, dördüncü-geçiş) [NORMATİF]

**P0-3 EXACT-SEALED-LENGTH (total-wire-invariant-bunsuz-TEORİK):** pacer-admission seal-ÖNCESİ-kesin-boyut-ister.
```rust
// [2026-09-19 — S3 hizalaması] Ayrı `KamaNoiseWireSizer` trait'i KALDIRILDI; Transport↔Crypto seam'i yalnız
// PacketProtection + SessionControl'dür (KM-SEAM-TC-001). sealed_len PacketProtection'ın metodudur:
//   fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError>;
// NORMATİF: seal(ctx, p).len() == sealed_len(ctx, p.len())  — üst-sınır-DEĞİL, KESİN. Transport bunu pacer-admission
// için çağırır; kripto format matematiğini tekrar etmez (limits(ctx) ile aynı sahiplik).
```
Pacer-akışı: logical-packet → exact-sealed_len → pacer/CC-token-RESERVE → seal → reserve==len-DOĞRULA → send.
Seal-başarısız → reservation-serbest. (Aksi: gereksiz-crypto-CPU · queue-bütçe-aşımı · R_wire_max-tahmini-kalır ·
padding-varyasyonu-microburst/PMTU-ihlali.)

**P0-4 SEND-SIDE-WIRE-IDENTITY (ACK/BtlBw/ARQ-semantiği-bunsuz-İMKANSIZ):** receiver-tarafı-OpenedShard-tanımlıydı
ama sender-tarafı wire-PN→logical-eşleme-YOKTU. ACK-gelince-engine-bilmeli: wire_pn → {packet-class, block_id, ESI,
source/repair/control, retransmission-generation, sealed-bytes, send-timestamp, dh_epoch, deadline}.
```rust
pub struct SealedUnit { pub bytes: Bytes, pub meta: SealedPacketMeta }   // ad S3 ile aynı
pub struct SealedPacketMeta { pub full_wire_pn: u64, pub dh_epoch: u8, pub sealed_len: u32 }
// [2026-09-19 — S3 hizalaması] `logical_ref` (packet-class/block_id/ESI) bu struct'tan ÇIKARILDI: TRANSPORT'un bilgisidir,
// transport kendi gönderdiği birimi kendi tablosunda meta.full_wire_pn ile eşler (KM-SEAM-TC-001 K2/K4). seal() → SealedUnit{bytes, meta}.
// `packet_class` da transport bilgisi (kaldırıldı); `send_generation` §0.5 send-record'un iç alanı — meta'da değil, Crypto'nun kendi state'inde.
```
Engine/pacer send-record-oluşturur; ACK/replay/FEC-logical-state bu-kayıt-üzerinden. (Bunsuz: ACK-hangi-shard'ı-
teslim-sayar? repair-ACK-BtlBw'ye-kaç-byte? recovered-hangi-retx-iptal? R_sent↔R_acked-ayrımı? RTT-hangi-tx'ten?)

## 1. GÖNDERİM/ALIM-SIRASI (SoC-analiz-§2, NORMATİF)
```
GÖNDERİM: App → Kama-Transport/FEC → UnsealedTransportShard → KamaNoise-Seal → UDP
ALIM:     UDP → KamaNoise-Open/Authenticate → OpenedTransportShard → Kama-Transport/FEC → App
```
FEC Noise'un-varlığını-BİLMEZ; engine/orchestrator iki-modülü-normatif-sırayla-birleştirir.

## 2. CRYPTEX-METADATA-YERLEŞİMİ (SoC-analiz-§7 — H1-hatanın-DOĞRU-yeri) [NORMATİF]
Cryptex = FEC-algoritması-DEĞİL, **metadata-gizleme-ilkesi**: FEC/transport-semantiğinin-hangi-kısmı-outer-
cleartext'te / hangi-kısmı-Noise-ciphertext'inde. **AAD-DÜZELTMESİ (H1 — AAD-authenticated-ama-ENCRYPTED-DEĞİL):**
```
OUTER-HEADER (minimum-cleartext, AAD-authenticated — ŞİFRELİ-DEĞİL):
  routing/session-handle · truncated-wire-PN · dh_epoch-selection (wire_flags.bit0) · generic-encrypted-discriminator
AEAD-CIPHERTEXT (ŞİFRELİ — FEC-semantiği BURADA):
  fec_version · code_suite_id · profile_id · block_id · ESI/symbol_id · source/repair-semantiği ·
  logical-message/chunk · flags · symbol-bytes
AEAD-TAG
```
**CRYPTEX-KORUR (jammer-okuyamaz):** hangi-paket-source/repair · block_id · ESI · profil · K/R-geometrisi · hangi-
shard-düşürülürse-block-çöker. **CRYPTEX-KORUMAZ (beyanlı):** timing · toplam-trafik · paket-boyutu/size-class ·
FEC-cadence-değişimi · repair-wire-artışı. Normatif: "Cryptex FEC-semantiği+kesin-symbol-konumu-gizler; timing/
hacim/boyut-GARANTİ-ETMEZ." Tam-gizlilik cover-traffic+padding (kapsam-DIŞI). Hafifletme: outer-source/repair-AYNI-
type/size-class. **INNER-KÜÇÜLTME (sub-kbps):** session/key_epoch outer'dan · K/R/E profile'dan · source/repair ESI-
aralığından · version-negotiated → hedef-tek/düşük-çift-haneli-envelope (her-shard-bağımsız-parse-korunur).

## 2.5 PN-NAMESPACE + WIDTH-DEMUX [NORMATİF — KARAR VERİLDİ]
**TEK-direction-local-full-PN-space** TÜM-sealed-packet-class'lar-için (SESSION_DATA + FEC-source/repair + secure-
ACK/SACK/recovered/completion/control): ccr-§35.8-replay-identity'de-packet-class-YOK → tek-monoton-full-counter-
per-(session, direction, dh_epoch). Gerekçe: nonce-uniqueness-yapısal-ispat · ACK/send-record-ortak · cross-class-
collision-imkansız · Seçenek-B'nin-domain-separated-nonce-türetimi-gereksiz-karmaşıklık. **Encoding:** mevcut-
SESSION_DATA seq16-BYTE-FOR-BYTE-korunur (P0-9); YENİ-class'lar (FEC/control) **32-bit-truncated-PN** (Alpha-MVP —
beşinci-geçiş+analiz-6.2-önerisi; 16-bit-yalnız-ölçüm-kanıtıyla-sonra). packet_class-discriminator PN-space'i-
BÖLMEZ — inner-type+SealedPacketMeta'da. Restart: PN-persistence-§4 (nonce-reuse-yasak).
**PN-WIDTH-DEMUX [NORMATİF — KARAR-VERİLDİ, PATCH-9]: YOL-1-SEÇİLDİ; YOL-2-REDDEDİLDİ** (mühürlü-SESSION_DATA-
envelope-DEĞİŞMEZ). Parse-döngüsü (packet_class-inner'da → decrypt-öncesi-PN-genişliği-bilinemez) HANDLE-UZAYIYLA-
çözülür:
```
h_legacy : mevcut-SESSION_DATA-parser'ını-seçer  → [h_legacy:4][seq16:2][ciphertext:N][tag:16]  (byte-for-byte-DEĞİŞMEZ)
h_ext    : yeni-FEC+secure-control-parser'ını-seçer:
           [h_ext:4][wire_format:1][wire_flags:1][pn32:4][ciphertext:N][tag:16]
wire_format = EXTENDED_V1 · wire_flags.bit0 = dh_epoch · diğer-bitler=0, non-zero-reserved → REJECT
```
· Hub h_ext'i HANDLE_ASSIGN-ile-dağıtır (§2.55 — hub-authenticated, E2E-FS-response'tan-AYRI); **handle-parser-layout'unu-decrypt-ÖNCESİ-seçer.**
· wire_format + wire_flags + pn32 **AAD'ye-DAHİL.** FEC-source/repair/control-semantiği-inner-ciphertext'te;
  outer-SIZDIRMAZ (tüm-extended-tek-görünüm — Cryptex-korunur).
· legacy-seq16 ve-extended-pn32 AYNI-direction-local-full-PN-namespace'in-düşük-bitleri; packet-class-nonce-
  namespace'ini-BÖLMEZ.

**[GERİ ALINDI 2026-09-18 — söküm yanlış öncüle dayanıyordu]** §2.5'in orijinal hükmü GEÇERLİDİR: session verisi
(SESSION_DATA) + o session'ın FEC source/repair shard'ları + E2E-seviyesi kontrol (inner_kind 0x10–0x1A) **tek
direction-local full-PN uzayı, tek E2E cipher state** (`send_cipher[dh_epoch & 1]`, protector.rs:245-247).
Kanonik mimaride (App → Transport → Crypto tel kenarında → UDP) bu doğrudur: shard'lar session verisidir, E2E
anahtarıyla mühürlenir, Cryptex metadata (block_id/esi/k) ciphertext İÇİNDE kalır, INV-FEC-1 (open → reconstruct)
E2E anahtarıyla PoC dahil tutar. **Ayrı anahtar bağlamı yalnız hub-yönelik trafik içindir:** HANDLE_ASSIGN (§2.55,
hub-channel), engine ACK_FRAME (hop), Plane 1 RPC — bunlar `ProtectionContext::HubChannel` ile hop anahtarı alır ve
ayrı PN uzayındadır. Transport bu bağlamı içeriği okumadan verir (birimi hangi kapıdan aldığını bilir); bugün kodda
ctx alanı YOK, tek seal yolu (runtime.rs:1089-1137; Plane 1 için ayrı yol yok, hub-server stub) → **S3 K1 ile KAPANDI**: `ProtectionContext{Session(h)|HubChannel}` çağıranın verdiği tek bağlamdır, transport v1.x'te `SealArgs`'a eklenir (KM-SEAM-TC-001 §1.1).
Ürün ACK yolu bugün tek: engine ACK_V2/ACK (ack.rs:26-30) aynı seam'den; noise-wire Control::Ack/Sack (0x10/0x11)
kodlanmış ama tüketicisi yok (audit tema E aynen açık).

## 2.55 H_EXT TAHSİS + BİLDİRİM [NORMATİF — DÜZELTİLDİ: hub-authenticated HANDLE_ASSIGN] (extended-parser-bunsuz-çalışmaz)
**KRİTİK-AYRIM (önceki-hata-düzeltmesi):** h_ext **Bob'ın-E2E-FS-response'una-BAĞLANMAZ** — SESSION_INIT_RESPONSE
Bob'ın-ephemeral'ını-taşıyan-E2E-authenticated-FS-upgrade-mesajıdır; **hub Bob-adına-E2E-authenticated-payload-
ÜRETEMEZ.** h_ext hub-routing-verisidir (L2), E2E-içerik-DEĞİL → ayrı-hub-authenticated-kanaldan-dağıtılır.
```
HANDLE_ASSIGN [WIRE-FROZEN — TEK-yol, exact-layout]:  hub→client hub-control-mesajı (hub↔client-Noise-transport-
kanalında-authenticated; E2E-endpoint-keyi-DEĞİL). HER-İKİ-tarafa-AYNI-format:
  version            u8      = 0x01
  session_attempt_id u64_be  # hub-assigned, hub-local-unique (attempt-bağı; collision-safe)
  init_counter       u64_be  # initiator-INIT'inden-echo (client-doğrulaması: hangi-INIT'e-cevap)
  h_legacy           u32_be
  h_ext              u32_be
TESLİM-SIRASI: INITIATOR'a hub-admission-sonrası-hemen (INIT'i-taşıyan-aynı-UDP-flow'da; Bob'dan-BAĞIMSIZ).
  RESPONDER'a forwarded-INIT-tesliminden-ÖNCE-veya-birlikte (hub-sıralama-garantisi: hub h_ext'li-extended-paketi
  responder'a HANDLE_ASSIGN-teslim-edilmeden-FORWARD-ETMEZ; responder-offline'sa mailbox'ta [HANDLE_ASSIGN, INIT]-
  sıralı-teslim).
REPLAY-IDENTITY: (hub_channel_session, session_attempt_id, init_counter) — aynı-attempt-tekrarı-AYNI-içerik
  (idempotent). RETRY+ACK: client HANDLE_ASSIGN_ACK-döner (hub-kanalında; session_attempt_id-echo, u64_be); hub
  ACK-gelene-veya-deadline'a-kadar-bounded-retry. ACK-yoksa-bile-client'ın-ilk-h_ext-kullanımı-implicit-teslim-kanıtı.
NAMESPACE: HANDLE_ASSIGN/HANDLE_ASSIGN_ACK hub↔client-KANAL-mesajlarıdır — E2E-inner_kind-registry'sine (§2.56,
  0x01-0x18) DAHİL-DEĞİL; ayrı-hub-control-namespace (hub-kanalının-kendi-mesaj-tipleri).
```
**Bu-değişiklik reverse-bağımsız-proactive-FEC-hedefini-kurtarır:** initiator h_ext'i **hub-admission-sonrası** öğrenir
(Bob'ın-FS-response'unu-BEKLEMEDEN) → **Bob-offline/reverse-çökük-olsa-da extended/FEC-GÖNDERİLEBİLİR** (epoch-0-usable
§0.7-A ile-tutarlı; ilk-PARAM_SET-hub-admission-sonrası, RESPONSE-beklemez).
```
LIFETIME: session-ömrü-sabit; session-kapanınca-serbest. REUSE: session-içinde-değişmez; farklı-session-hub-
          collision-kontrolü (ccr-§35.7-unknown-handle→cheap-drop).
RESTART:  process-restart=yeni-session (§15) → yeni-SESSION_INIT → yeni-HANDLE_ASSIGN → yeni-h_legacy+h_ext.
HUB-MIGRATION/ROUTE-CHANGE: yeni-hub yeni-HANDLE_ASSIGN-atar; eski-h_ext-eski-hub'a-özel; in-flight-eski-h_ext-
          yeni-hub'da-unknown-handle→drop (retransmit-yeni-h_ext'le).
AAD-BAĞI: h_ext AAD'ye-dahil (§2.8) → transit'te-DEĞİŞTİRİLEMEZ (malicious-hub-h_ext-değiştirse-AEAD-fail; hub-
          zaten-routing-handle'ı-görür — E2E-authentication-sınırı-değişmez, ccr-§35.7).
```

## 2.56 INNER_KIND-REGISTRY [NORMATİF] (decrypt-sonrası-tek-parser-seçimi; packet_type↔control_type-çakışması-YOK)
Decrypt-sonrası-ikinci-byte-AYRI-yorumlama (FEC-packet_type / control-control_type)-YASAK → **tek-birleşik-alan:**
```
UNIFIED-INNER-HEADER:  inner_version u8 (=INNER_V1=0x01) · inner_kind u8 · variant_body...
INNER_KIND-CODEPOINT'leri (çakışmasız):
  0x01 FEC_SOURCE            0x02 FEC_REPAIR
  0x10 ACK                   0x11 SACK                0x12 RECOVERED          0x13 BLOCK_COMPLETE
  0x14 PARAM_SET             0x15 PARAM_ACK           0x16 BLOCK_ABORT        0x17 SESSION_CLOSE
  0x18 SESSION_RENEW_REQUIRED
  0x19 CTRL_CS_PROBE [DR-CS-1-AMENDMAN]   0x1A CTRL_CS_ECHO [DR-CS-1-AMENDMAN]
  0x00 reserved(reject) · 0x03..0x0F reserved-FEC-range · 0x1B..0xFF reserved(reject)
EXTENDED_V1 wire_format=0x01. Bilinmeyen-inner_kind → FAIL-CLOSED-reject (frame-seviyesi; session-yaşar —
eski-peer'ın 0x19/0x1A-safe-ignore-tabanı; mixed-version-§DR-CS-1).
```
**DR-CS-1-AMENDMAN-NOTU [Abkarada-onaylı; dar-kapsam]:** 0x19/0x1A cold-start-minimal-bootstrap kontrol-
çifti için reserved-alandan K-WIRE-CTRL-1-prosedürüyle açıldı. Engine-spec'in eski `0x12=CTRL_PATH_PROBE`
kaydı CANCELLED/SUPERSEDED (0x12=RECOVERED-kilidi-korunur; runtime-assert). Bu-amendman mevcut-
codepoint'lerin anlamını/layout'unu/parser-davranışını DEĞİŞTİRMEZ; başka-codepoint/subtype/alan-ekleme
= DUR-B.
FEC-source/repair variant_body = §2.6-FEC-inner (profile_id..symbol_bytes). Control variant_body = §2.7-control-body.

## 2.57 CONTROL-BODY-LAYOUT'LARI [NORMATİF — MADDE-4/5, exact] (§2.56-inner_kind-sonrası-body)
Ortak: control-paketi = UNIFIED-INNER-HEADER(inner_kind∈0x10..0x18) · control_id-u32_be · body. **control_id:**
session+direction-scoped, retry'da-AYNI, yeni-logical-control'da-YENİ (idempotent-dedup); wrap-öncesi-session-renewal.
```
ACK   (0x10): acked_dh_epoch u8 · ack_range_count u16_be · [pn_start u64_be, pn_end u64_be]×count
              (full-wire-PN/range, epoch-scoped §4; ACK-eliciting-DEĞİL)
SACK  (0x11): acked_dh_epoch u8 · base_pn u64_be · gap_ack_block_count u16_be · [gap u16_be, ack_len u16_be]×count
              (epoch-scoped; ACK-eliciting-DEĞİL)
RECOVERED(0x12): block_id u32_be · esi_range_count u16_be · [esi_start u16_be, esi_end u16_be]×count
                 (logical-recovery-YALNIZ; ACK/BtlBw-credit-DEĞİL; retransmit-cancel+retention-release)
BLOCK_COMPLETE(0x13): block_id u32_be · final_source_count u16_be
PARAM_SET (0x14): param_hash[32] (§8-canonical-BLAKE2s-256) · profile_id u8 · code_suite_id u8   (ACK-eliciting)
PARAM_ACK (0x15): param_hash[32]  (aynı-hash-echo; ACK-eliciting-DEĞİL)
BLOCK_ABORT(0x16): block_id u32_be · reason_code u8
SESSION_CLOSE(0x17): reason_code u8
SESSION_RENEW_REQUIRED(0x18): reason_code u8 · renew_hint u8   (cap/wrap-yaklaşımı; yeni-handshake-tetiği §5)
CTRL_CS_PROBE(0x19) [DR-CS-1]: subtype u8 (=0x01 cold-start) · probe_seq u16_be · pad u8[9] (=0x00; amp≤1:
                 probe_wire ≥ echo_wire) — body=12B → EXACT-WIRE = 26+6+12 = **44B** (PROBE_WIRE_EXACT).
                 probe_seq: sender-session-monoton, yeniden-KULLANILMAZ (INV-CS-7); wrap→yeni-probe-YOK.
CTRL_CS_ECHO (0x1A) [DR-CS-1]: subtype u8 (=0x01) · probe_seq u16_be (yansıtılan) · seen_count u32_be ·
                 arrival_gap u32_be — body=11B → EXACT-WIRE = 26+6+11 = **43B** (ECHO_WIRE_MAX).
                 seen_count/arrival_gap YALNIZ-LEDGER (karar-girdisi-YAPILAMAZ — DR-CS-1-karar-§3).
                 Echo: alıcı-flow-state'inden idempotent-stateless; per-probe-uzun-ömürlü-allocation-YOK;
                 rate-limit T_ECHO_MIN/akış; hedefli-retransmit/yeni-ACK-semantiği-EKLENEMEZ.
```
Kurallar: her-count-profile'da-BOUNDED (max_ack_ranges/max_sack_blocks/max_recovered_ranges) · oversized/invalid-
range/unknown-reason → FAIL-CLOSED · reserved-alan-nonzero → reject · **cleartext-feedback-YOK** (hepsi-EXTENDED-
AEAD).
**IDEMPOTENCY + RETRY-STATE-MACHINE [DÜZELTİLDİ — her-mesaj-DOĞRU-ACK-türüyle]:**
```
PARAM_SET               → PARAM_ACK (hash-echo) gelene-kadar-retry (control_id-sabit); deadline→retry-durur,
                          emission-SÜRER (B2: ACK=confirmation, admission-kapısı-değil); kalıcı-hash-mismatch→SESSION_CLOSE
BLOCK_ABORT             ┐
BLOCK_COMPLETE          │ → KENDİ-wire-PN'leri network-ACK (0x10/0x11) tarafından-KAPSANANA veya-deadline-dolana
SESSION_CLOSE           │   kadar-retry. (PARAM_ACK-BUNLARI-onaylamaz — yalnız-PARAM_SET-hash'ini.)
SESSION_RENEW_REQUIRED  ┘
ACK / SACK / RECOVERED  → RETRY-EDİLMEZ (pure-feedback, en-güncel-durum-taşır)
CTRL_CS_PROBE / CTRL_CS_ECHO [DR-CS-1] → RETRY-EDİLMEZ (probe-retry = merdivenin-kendisi 3/9/21/45s;
                          echo = idempotent-stateless-yanıt). Replay: EXTENDED-outer-pn32-window +
                          control_id-dedup OTOMATİK (yeni-zarf-YOK, yeni-plaintext-yol-YOK).
```
· **SESSION_CLOSE-alan-taraf** ACK-gönderebilmek-için kısa-**TOMBSTONE/GRACE-state** tutar (session-state-silinse-de
  gelen-CLOSE-retry'larına-ACK-üretebilsin; bounded-süre-sonra-tombstone-discard). · Receiver TÜM-tekrarları
  **control_id-ile-idempotent** işler (aynı-id-ikinci-kez-app/state-side-effect-YOK; yalnız-ACK-yeniden-üretilebilir).

**DR-CS-1b-A2-AMENDMAN A-2 [Abkarada-onaylı; dar-kapsam — davranış-açıklama-notu]:** CTRL_CS_PROBE/
CTRL_CS_ECHO (0x19/0x1A) **control_id-dedup-set'ine GİRMEZ** — alıcı bunları demux'ta dedup-ÖNCESİ
stateless-echo-yoluna ayırır (caller-dispatch; DI-10/g10c: dedup-girişi unbounded-büyüme-anti-pattern'i).
Yukarıdaki "control_id-dedup OTOMATİK" ifadesi probe/echo için YALNIZ outer-pn-window kripto-replay-katmanını
ifade eder; echo idempotent-stateless olduğundan logical-dedup'un önleyeceği side-effect yoktur. Bu not
wire-layout/codepoint-anlamı DEĞİŞTİRMEZ.

**DR-CS-1b-A2-AMENDMAN A-3 [Abkarada-onaylı; dar-kapsam — control_id-atama-kontratı]:**
```
control_id MUST be monotonically allocated per session and direction.
Retries MUST retain the original control_id.
A control_id MUST NOT be reused within the same session/direction.
```
Wrap-semantiği: **control_id wrap olmadan önce session-renewal ZORUNLU** (modular-wrap-karşılaştırması
TANIMLANMAZ; wrap'a-ulaşmadan-renewal). Receiver-dedup fixed-window/high-water modeliyle uygulanabilir;
window-altına-düşen-id → yanıt `DuplicateAckOnly` + **ayrık teşhis `ControlIdBelowDedupWindow`** (window-
sizing-varsayımı-bozulması sessizce-"duplicate"-sanılmaz). Bu kontrat dedup/replay-semantiğinin ve peer-
interoperability'nin parçasıdır; wire-layout DEĞİŞMEZ.

## 2.6 FINAL-BYTE-LAYOUT [NORMATİF — KİLİTLİ] (tek-otorite: bu-tablo + schema-crate)
```
EXTENDED-OUTER (fixed 26B + encrypted_inner):
  h_ext u32_be | wire_format u8=EXTENDED_V1(0x01) | wire_flags u8 (bit0=dh_epoch, bits1..7=0→reject)
  pn_trunc u32_be | ciphertext var | tag 16B

FEC-SHARD ENCRYPTED-INNER (UNIFIED-HEADER §2.56 + FEC-variant_body):
  inner_version u8=0x01 · inner_kind u8 (0x01 FEC_SOURCE | 0x02 FEC_REPAIR) ·
  flags u8 (reserved-nonzero→reject) · profile_id u8 · block_id u32_be · esi u16_be · k_actual u8 ·
  symbol_bytes[profile.E]                                    # symbol_len-alanı-YOK: her-shard-TAM-E-byte (MADDE-6)

CANONICAL-SOURCE-SYMBOL-PREFIX (RS-kodlanır — kayıpta-metadata-reconstruct; boyut = E):
  message_id u32_be · chunk_offset u32_be · actual_payload_len u16_be · source_flags u8 (final/end-of-message) ·
  application_payload[actual_payload_len] · canonical_zero_pad[ E − 11 − actual_payload_len ]
```
**SYMBOL_LEN-KİLİDİ (MADDE-6):** her-source-VE-repair-shard **symbol_bytes == profile.E** (sabit; RS-crate-sabit-
shard-size'la-uyumlu). `actual_payload_len` gerçek-app-verisini-gösterir; kalan = canonical_zero_pad. Kısa-source-
wire-encoding-YOK (interoperability-belirsizliği-kapandı). RS-hem-prefix-hem-payload-hem-padding'i-kodlar.
**ID-SCOPE/WRAP (MADDE-14):** block_id = session+direction-scoped-monoton-u32, **NO-REUSE** · message_id =
session+direction-scoped-monoton-u32, **NO-REUSE** · ESI-u16 · K_actual-u8 · profile_id-u8 · source/repair-ayrımı-
YALNIZ-ciphertext'te · **cap/wrap-öncesi session-renewal-ZORUNLU** (SESSION_RENEW_REQUIRED §2.57 → yeni-handshake
§5) · ID-width'ler-iki-belge+schema-crate-TEK-otoriteden.

## 2.7 SECURE-CONTROL-FORMAT [NORMATİF — KİLİTLİ, MADDE-4] (codepoint'ler §2.56; body'ler §2.57)
Tüm-control'lar EXTENDED-outer + per-packet-AEAD + UNIFIED-INNER-HEADER (§2.56: inner_version+inner_kind).
control_id-u32_be + body §2.57'de-exact-tanımlı. Alpha-allowlist = §2.56-codepoint'leri (0x10..0x18);
**KEY_UPDATE-Alpha'da-YOK** (in-session-key-phase-yok §5; eski→Beta/SESSION_RENEW_REQUIRED). Semantik-özet:
ACK-full-wire-PN/range · RECOVERED-logical-(block_id,ESI)-ACK/credit-DEĞİL · control_id-idempotent-dedup ·
unknown-kind/oversized/invalid-range/reserved-nonzero → FAIL-CLOSED · count'lar-profile-bounded · cleartext-YOK.

## 2.8 CANONICAL-AAD [NORMATİF — KİLİTLİ, MADDE-13]
```
AAD = ASCII("KAMA-WIRE-EXT-V1") || h_ext_u32_be || wire_format_u8 || wire_flags_u8 || pn_trunc_u32_be
```
Alan-sırası-DEĞİŞMEZ · tüm-integer'lar-BIG-ENDIAN · native-struct-serialization/padding/serde-default/platform-
layout-KULLANILMAZ · ciphertext-length UDP-length-üzerinden-implicit (explicit-inner-lengths-authenticated-
ciphertext-içinde) · AEAD-tag-AAD'nin-parçası-DEĞİL · full-PN = truncated-reconstruction, seçili-Noise-cipher-
state-nonce-counter'ı · replay-window-YALNIZ-AEAD-success-sonrası-commit · **golden-vector-AAD-byte'larını-AYRICA-
assert-eder; iki-implementation-AYNI-AAD-dizisini-üretir.**

## 3. DÖRT-KİMLİK + KRİPTO-REPLAY↔LOGICAL-DEDUP (SoC-analiz-§10 + go/no-go-§3.2) [NORMATİF]
```
(1) full-wire-PN/nonce-counter  (2) block_id  (3) ESI/logical-symbol-id  (4) message/chunk-id
SECURE-WIRE-IDENTITY = (session_id, direction, dh_epoch, full_pn) → nonce-uniqueness + kripto-replay + wire-tx-id + network-ACK/RTT
FEC-LOGICAL-IDENTITY  = (session, block_id, ESI)    → logical-dedup + decoder-rank + ARQ-cancel + app-dup-prevent
```
**FEC-logical-identity ASLA-kripto-replay-key-DEĞİL.** RETRANSMIT: (block_id,source_esi)-AYNI · wire-PN+nonce-YENİ
(aynı-nonce/ciphertext-retransmit → replay-düşürür + nonce-reuse-bozulur). Geç-original: AEAD+outer-replay-geçer →
wire-muhasebe → logical-duplicate → rank-artmaz → app'e-2.teslim-YOK → RTT/BtlBw yalnız-gerçek-wire-ACK.

## 4. NONCE/PN/ANTI-REPLAY [NORMATİF — KİLİTLİ, MADDE-4] (ccr-§35.8-model + §2.5-YOL-1; N1=uyum-doğrulaması)
**seq-2B doğrudan-Noise-nonce-ise KABUL-EDİLMEZ.** Noise-explicit-nonce-taşımaya-izin-verir ama truncated-PN-
reconstruction/replay/dh_epoch-transition Noise'un-parçası-DEĞİL — Kama-Secure-Wire-Profile'ın-KENDİ-normatif-
datagram-sözleşmesi. **State PER-(session, direction, dh_epoch) — DÜZELTİLDİ (epoch-aware, epoch-0/1-aynı-PN-
değerlerini-kullanabilir → ayrı-state-ZORUNLU):**
```
expected_pn[dh_epoch]      # u64, en-yüksek-kabul+1, HER-dh_epoch-AYRI
replay_window[dh_epoch]    # bitmap, WINDOW=2^14=16384, HER-dh_epoch-AYRI
```
Legacy-seq16-16-bit-düşük, extended-pn32-32-bit-düşük; İKİSİ-de AYNI-direction-local-full-PN'in-düşük-bitleri (§2.5).
**[DR-3 — SABİT-KAPSAM-AYRIMI, KARAR-VERİLDİ]:** anti-replay-sabitleri PER-ENCODING-WIDTH: **legacy-seq16-yolu →
ccr-§35.8-sabitleri** (REPLAY_WINDOW=1024, ~~MAX_FUTURE_GAP=4096, MAX_REORDER=8192 — mühürlü; 16-bit-span-65536'ya-uygun-dar-pencere~~ — **[D-GAP-1 2026-09-23] bu iki
kapı ccr §35.8'de KALDIRILDI → legacy-seq16 yolunda far-future reddi YOKTUR**; kalan tek sınır yapısaldır (|cand−expected| < 2^15,
seq16 tek-anlamlılığı) ve GÖNDEREN tarafından korunur: transport-layer-tdd [AMENDMENT-9] / KS-PN-1. REPLAY_WINDOW=1024 aynen kalır); **extended-pn32-yolu → bu-bölümün-sabitleri** (WINDOW=2^14, FUTURE_GAP_MAX=2^13 — 32-bit-span-
geniş-pencere-taşır). İki-otorite-arasında-ÇELİŞKİ-DEĞİL-kapsam-ayrımı; iki-yol-ayrı-sabit-setini-uygular.
```
reconstruct(low_bits, low_width_bits, dh_epoch):
  span   = 1 << low_width_bits              # 16→65536 | 32→2^32
  center = expected_pn[dh_epoch]
  base   = center & ~(span-1)
  cand   = base | low_bits
  # en-yakın-aday, UNDERFLOW-SAFE (checked, u64-wrap-yok):
  if cand + span/2 < center:  cand = cand + span
  elif cand > center + span/2 and cand >= span:  cand = cand - span   # cand<span ise çıkarma-YOK (underflow-guard)
  return cand

accept(candidate, dh_epoch_wire):
  if dh_epoch_wire not in {current_dh_epoch, previous_dh_epoch_if_in_grace}: REJECT   # §5-grace
  ew = expected_pn[dh_epoch_wire]; win = replay_window[dh_epoch_wire]
  if candidate + WINDOW < ew:                    REJECT  # TOO-OLD (checked-add, underflow-safe)
  if candidate < ew and window_seen(win, candidate): REJECT  # DUPLICATE
  if candidate > ew + FUTURE_GAP_MAX:            REJECT  # FUTURE-GAP (FUTURE_GAP_MAX=2^13 < span/2 tek-anlamlı)
  #   [D-GAP-1 2026-09-23] BU SATIR YALNIZ extended-pn32 YOLU İÇİN GEÇERLİDİR. legacy-seq16 yolunda kapı
  #   KALDIRILDI (ccr §35.8) → o yolda bu kontrol UYGULANMAZ. pn32 kapısının D-GAP-1 kapsamına alınıp
  #   alınmayacağı AÇIK bir karardır (identity OQ-16): pn32'nin yapısal sınırı 2^15 değil span/2 = 2^31'dir.
  if AEAD_open(candidate, dh_epoch_wire) fails:  REJECT  # candidate=dh_epoch_wire-cipher-state-nonce; window-İLERLEMEZ
  # SUCCESS → COMMIT (yalnız-ilgili-epoch-state):
  window_mark_seen(win, candidate); if candidate >= ew: expected_pn[dh_epoch_wire] = candidate + 1
```
**UNDERFLOW-KURALI:** tüm-`−WINDOW`/`−span`/`−FUTURE_GAP` işlemleri checked/saturating (u64-wrap-YASAK); karşılaştırma-
formu `candidate + WINDOW < ew` (çıkarma-yerine-toplama) kullanılır. **EPOCH-PN-INIT:** her-dh_epoch'un-cipher-state'i-YENİ → expected_pn[1]-ve-send-counter-epoch-1'de **0'dan-başlar** (epoch-0-değerinden-devam-ETMEZ; ACK-epoch-scoped-olduğu-için-çakışma-yok). **ACK/SACK-EPOCH-BAĞI (§2.57):** ACK/SACK-body'si
`acked_dh_epoch u8` taşır → hangi-epoch'un-PN'ini-onayladığı-KESİN (epoch-0/1-aynı-PN-değeri-belirsizliği-çözüldü).
**WRAP:** full-PN-u64-tükenmez; cap/wrap-yaklaşımında SESSION_RENEW_REQUIRED → yeni-handshake (§5). **DH_EPOCH-0→1-GEÇİŞ [B3 — CONFIRMATION-SAFE-STATE-MACHINE; send/recv-epoch-DIRECTION-LOCAL]:**
```
1. Bob RESPONSE-gönderir AMA Alice→Bob-yönünde epoch-0-KABULÜNÜ-SÜRDÜRÜR (grace-BAŞLAMAZ — Alice'in-aldığı-belirsiz).
2. Alice RESPONSE'u-alınca kendi-OUTBOUND'unu-epoch-1'e-geçirir (send_epoch[A→B]=1) ve-HEMEN-epoch-1'de-en-az-
   bir-paket-gönderir (bekleyen-data-VEYA-ACK/keepalive — RESPONSE-ACK-eliciting-sayılır; idle-deadlock-önlenir:
   Alice-hiç-göndermezse-Bob-adım-3'e-geçemezdi).
3. Bob İLK-GEÇERLİ-epoch-1-paketini-alınca Alice'in-RESPONSE'u-aldığını-ÖĞRENİR (implicit-confirmation).
4. Bob ANCAK-bundan-sonra: epoch-0-recv-GRACE'ini-başlatır + kendi-OUTBOUND'unu-epoch-1'e-geçirir (send_epoch[B→A]=1).
5. GRACE sabit-2s-DEĞİL: ~~bounded = max(4×sRTT, GRACE_MIN_PACKETS=64-paket-eşdeğeri)~~ → KS-GRACE-1 (aşağıda; sRTT bağımlılığı KALDIRILDI)
   **[KARAR-SLOTU KS-GRACE-1 — 2026-09-07, HOS-3]** `sRTT` transport estimator'ıdır; kripto kabul
   penceresinin estimator kalitesine bağlanması istenmiyor (estimator bozulursa güvenlik penceresi
   oynar). Kripto-gözlemlenebilir forma çevrilecek:
     epoch-0 grace KAPANIR = (epoch-1'de AEAD-GEÇERLİ kabul edilen N_g paket) VEYA (T_g mutlak monotonic süre)
                             — HANGİSİ ÖNCE DOLARSA. Sayaç yalnız AEAD-geçerli epoch-1 paketleriyle artar (sahte
                             paket grace'i kapatamaz; implicit confirmation ile aynı ilke). T_g, implicit
                             confirmation ANINDAN başlar (RESPONSE gönderiminden değil). Kapanınca epoch-0 recv state
                             DISCARD, sonrası REJECT; yeniden AÇILMAZ.
     N_g = [KALİBRASYON]   T_g = [KALİBRASYON]   (semantik donmuştur, 2026-09-19)
   ccr §35.5/§35.12'nin "RESPONSE sonrası epoch-0 → REJECT" satırı bu kurala REFERANS verecek
   (iki belge tek hüküm). Hostile gerekçe: hemen-red seçilirse uçuştaki tüm epoch-0 paketleri
   çöp olur; 3 B/s rejiminde bu doğrudan teslim süresidir. Güvenlik maliyeti dar: o paketler
   zaten epoch-0 altında gönderilmişti; grace yalnız KABUL penceresini uzatır.; hostile-reorder/RTO-bütçesine-
   bağlı [CAL — D1-ölçümle-kilitlenir]. Grace-bitince epoch-0-recv-state-discard.
```
RESPONSE-kaybolursa: Alice-epoch-0'da-kalır, Bob-epoch-0-kabulü-SÜRER (adım-1) → "epoch-0-usable"-garantisi-
KORUNUR (çelişki-çözüldü). Bob-RESPONSE'u-bounded-retry-eder (§0.7-B-flight-machine-simetriği). **RESTART:** process-
restart=yeni-session (§15); full-PN-persist-EDİLMEZ, restart-her-zaman-yeni-handshake+session_id (nonce-reuse-
yapısal-imkansız). **ORDER:** (1)fixed-cost-outer-parse (2)handle→layout (§2.5) (3)low_bits+dh_epoch→candidate
(4)too-old/dup/future-precheck-window-İLERLETMEDEN (5)dh_epoch-cipher-state-AEAD-attempt (6)success→COMMIT (7)inner-
parse+bounds (8)OpenedPacket-handoff. **Unauthenticated-paket replay-window-İLERLETMEZ.** N1 = kod-bu-pseudocode'a-
uyuyor-mu (uymuyorsa-KOD-düzeltilir).

## 4.5 AEAD-NONCE96 [NORMATİF — KİLİTLİ, DR-4] (ChaCha20-Poly1305, per-direction per-dh_epoch CipherState)

> DR-4-CLOSED — KOŞUL-A (LE64-OTOMATİK). Envanter: `reports/decision-requests/DR-4-envanter.md` (vector=NO_VECTOR,
> compatibility-debt=NO → gerçek-borç-yok). D2'nin-BE'si farklı-nonce-modeli (HKDF-türevli) + Historical → LE'yi-engellemez.

```
fn nonce96(full_pn: u64) -> [u8; 12]:
    nonce = [0u8; 12]
    nonce[0..4]  = 0x00 0x00 0x00 0x00          # 32-bit zero prefix
    nonce[4..12] = full_pn.to_le_bytes()        # LE64 — Noise-Framework-ChaChaPoly-KANONİK-mapping
    return nonce

ÖRNEK-BYTE'LAR (elle-yazılmış — golden bunları-DOĞRUDAN-assert-eder):
    full_pn=0     → 00 00 00 00 · 00 00 00 00 00 00 00 00
    full_pn=1     → 00 00 00 00 · 01 00 00 00 00 00 00 00
    full_pn=2^32  → 00 00 00 00 · 00 00 00 00 01 00 00 00
    full_pn=2^63  → 00 00 00 00 · 00 00 00 00 00 00 00 80

NOT: wire'daki pn32/seq16 BIG-ENDIAN-taşınır (§2.6 — network-byte-order); nonce96 ise CIPHER-PRIMITIVE-girdisidir
ve Noise-ChaChaPoly-kanonik-LE64-kullanır. İKİ-FARKLI-SERIALIZATION-SÖZLEŞMESİ — karıştırılmaz.

GEREKÇELİ-AYRIMLAR (test-edilebilir-INVARIANT'lar — spec-cümlesi-değil-test):
  INV-NONCE-DIR-1  : direction nonce'a-KODLANMAZ çünkü Split() per-direction-AYRI-key-üretir.
                     BİRİNCİL-TEST: deterministic-KDF/Split-vector → k_A→B != k_B→A (key-derivation-seviyesi).
                     EK-REGRESSION: aynı-{nonce,AAD,plaintext}-iki-yön-farklı-ciphertext (ispat-değil-regression).
  INV-NONCE-EPOCH-1: dh_epoch nonce'a-KODLANMAZ çünkü her-epoch-AYRI-cipher-key/state.
                     BİRİNCİL-TEST: k_epoch0 != k_epoch1 (KDF-vector) + epoch1-counter=0-assert + epoch0/epoch1-
                     AYNI-key-NESNESİNİ-paylaşamaz (tip/lifecycle-assert).
  INV-NONCE-REUSE-1: aynı-key-altında-aynı-nonce96-İKİNCİ-KEZ-KULLANILAMAZ.
                     TEST: counter-monoton + no-reuse-guard + wrap/cap-öncesi-SESSION_RENEW_REQUIRED + restart=
                     yeni-session/yeni-key (full-PN-persist-YOK).

Seçili-mapping protocol-version-V1-için-IMMUTABLE.
GOLDEN-ZORUNLU-ASSERT-SETİ: reconstructed-full_pn · exact-nonce96-bytes · exact-AAD · exact-ciphertext · exact-tag ·
RustCrypto==libsodium-IETF-eşitliği (K-WIRE-NONCE96-1).
```

## 5. ALPHA DH-EPOCH + SESSION RENEWAL [NORMATİF — KİLİTLİ, MADDE-7] (in-session-key-phase-YOK)
**Alpha-normatifi (dört-kavram, çelişki-çözüldü):**
```
1. Split()                : handshake-sonu k_send/k_recv (bir-kez)                        → KORUNUR (mühürlü)
2. dh_epoch 0→1 upgrade   : SESSION_INIT_RESPONSE asenkron-full-FS (epoch-0-usable-önce)  → KORUNUR (mühürlü)
3. in-session-symmetric-key-phase-state-machine                                           → ALPHA'DA-YOK
4. cap→new-session        : packet/byte/time-cap-dolunca SESSION_RENEW_REQUIRED → tamamen-YENİ-handshake/session
```
**Terminoloji [KİLİT]:** yalnız **dh_epoch (0/1)**; `key_phase`/`rekey_epoch`/`KEY_UPDATE`-terimleri Alpha-
normatifinde **KULLANILMAZ.** Kimlik: **(session_id, direction, dh_epoch, full_pn).** Outer-state = wire_flags.bit0-
dh_epoch (§2.5). dh_epoch-grace §4'te. Noise-native-REKEY()-primitive-açıklaması → Beta-appendix (§Appendix-B,
non-normative). Cap/wrap-öncesi session-renewal-ZORUNLU (ID-scope §2.6).

## 5.5 [BETA-APPENDIX, NON-NORMATIVE] in-session-key-phase-research
Beta'da-değerlendirilecek in-session-symmetric-key-phase-machine'in-açık-soruları (Alpha'da-YOK): başlatıcı/eşik/
sinyal/bit-koruması/current-next-previous-key-sayısı/ikinci-update/ACK-sırası/self-sync/PN-scope/eski-key-ömrü/
bounded-trial/timing-side-channel. QUIC-key-phase-esin. **Alpha-implementasyonu-bu-bölümü-UYGULAMAZ.**
**N2 [UYUM-DOĞRULAMASI — §5-karar]:** mevcut-kodda in-session-rekey/key-phase-kodu-VAR-mı (varsa-Alpha-
yolundan-kaldırma-envanteri); cap-tetiği-SESSION_RENEW_REQUIRED→re-handshake-açıyor-mu? eski-dh_epoch-grace-
sonrası-discard-ediliyor-mu?

## 6. VERIFY-BEFORE-ADMISSION (SoC-analiz-§8 — SAHİPLİK-bu-katmanda) [NORMATİF]
"AEAD-doğrulanmamış-shard FEC-decoder'a-KABUL-EDİLMEZ" — **sahibi FEC-codec-DEĞİL, bu-profil.**
```
KamaNoise: AEAD-verify/open · kripto-replay-check · inner-envelope-decrypt · minimum-parse/bounds
Transport/FEC: opened-authenticated-shard-admission · logical-block/profile-validation · decoder-admission
```
FEC-API şu-detayı-BİLMEZ: hangi-AEAD/nonce/key-phase. FEC yalnız engine'in-verdiği-OpenedTransportShard'ı-işler.
"verify-before-FEC-admission FEC-codec'in-kripto-özelliği-DEĞİL, bu-profilin-zorunlu-admission-kuralı."
**TYPE-LEVEL-ENFORCEMENT (analiz-P0-5 — call-site-disiplini-YETMEZ) [NORMATİF]:** Opened/Authenticated-tip
**sealed:** alanlar-PRIVATE + `_proof: PrivateAdmissionProof` (public-constructor-YOK) + YALNIZ-
`KamaNoiseWireProtector::open()`-üretir + `#[cfg(test)]`-constructor-ayrı. **Schema-ownership-düzeltmesi (P0-8-
revize):** kama-transport-schema UNSEALED-DTO'ları-tanımlar (herkes-üretebilir); **OPENED/Authenticated-tip
noise-wire-crate'inde** (constructor-yetkisi-orada) — schema-yalnız-primitive/logical-tipler. "Şu-an-başka-crate-
üretebiliyor-mu"-cevabı: kod-henüz-yok; bu-kural-D1-tip-tasarımının-ZORUNLU-şartı (K-WIRE-SEALED-TYPE-1).

## 7. PER-SHARD-AEAD (SoC + go/no-go — block-tek-tag-YASAK) [NORMATİF]
source-shard-ayrı-AEAD · repair-shard-ayrı-AEAD · metadata AEAD-CIPHERTEXT'te (AAD-DEĞİL) · block-level-tag-VARSA-
yalnız-ek-defense-in-depth. (Block-tek-tag → forged-shard-decoder'a-girer + decode-DoS + block-sonuna-kadar-
unauthenticated-buffering.) FEC-before-per-shard-AEAD-pipeline: canonical-source-symbol → RS-repair-plaintext-source'
tan → her-source/repair-shard-ayrı-PN/nonce-AEAD → receiver-yalnız-auth/decrypt-shard-decoder'a → eksik-source-
plaintext-reconstruct. **Kaybolan-source'un-eski-outer-AEAD-tag'i-YENİDEN-ÜRETİLMEZ** (decoder-plaintext-symbol-
reconstruct-eder, tag-değil).

## 8. TRANSPORT-PARAMETER-BINDING (SoC-analiz-§12) [NORMATİF]
FEC/transport-parametreleri (fec_version/code_suite/profile/K-R-E/flags/partial-policy/schedule) Noise-transcript'e
FEC-modülü-tarafından-DOĞRUDAN-bağlanmaz. Akış: negotiated-params → canonical-serialization → canonical-hash →
engine → Kama-Noise-handshake/prologue/channel-binding. `kama_noise_session.bind_transport_parameters(hash)`.
**PARAMETER-BINDING [NORMATİF — KARAR-VERİLDİ, PATCH-10]: DURUM-B-SEÇİLDİ** (handshake-SONRASI, established-
Noise-channel-İÇİNDE-authenticated-binding). [DURUM-A (prologue-binding) = REDDEDİLEN-ALTERNATİF: profil-seti-
önceden-bilinmeli + negotiation-handshake-dışı-olamaz + version-mismatch-ayrı-discovery-isterdi.] Kurallar:
· protocol-version handshake-ÖNCESİ-SABİT · Alpha'da-TEK-code_suite_id · profile-seti = versioned-ALLOWLIST.
· Sender seçili-profile/parameter-setini encrypted **PARAM_SET**-control-frame'le-yollar; receiver canonical-
  hash'i-doğrular ve **PARAM_ACK**-ile-AYNI-hash'i-döndürür. **[B2-KARAR — reverse-bağımsızlık-KORUNUR]:**
  **PARAM_ACK = SENDER-CONFIRMATION'dır, data-admission-KAPISI-DEĞİLDİR.** Alpha protocol-version'a-bağlı
  **MANDATORY-BOOTSTRAP-PROFILE** tanımlar (tek-suite-zaten): sender PARAM_SET'i-gönderir-ve-HEMEN-FEC-data-
  emission-BAŞLATABİLİR (Bob-offline/reverse-çökük-olsa-da — proactive-FEC-hedefi). **Receiver-kuralı:** authenticated-
  PARAM_SET'i-alınca-sonraki-FEC-shard'larını-kabul-eder; PARAM_SET-henüz-gelmemişken-gelen AUTHENTICATED-FEC-shard
  (reorder/kayıp) → bounded-hold (max_preparam_shards, küçük; aşımda-en-eski-drop; unauthenticated-buffering-DEĞİL —
  shard-AEAD-geçmiş, yalnız-profile-bekliyor). PARAM_ACK-gelmezse: emission-DURMAZ; PARAM_SET-bounded-retry-sürer;
  sender-yalnız-"receiver-confirmed"-bilgisinden-mahrum-kalır.
· **PARAM_SET-CANONICAL-HASH (MADDE-8):** `param_hash = BLAKE2s-256( DS || fields )` · DS = ASCII("KAMA-PARAM-
  V1") · fields (canonical, big-endian, sıra-sabit): protocol_version u8 · fec_version u8 · code_suite_id u8 ·
  profile_id u8 · K_shape u16_be · R_code_max u16_be · E u16_be · partial_policy u8 · packet_scheduling_policy u8.
  Native-struct/serde/padding-KULLANILMAZ. PARAM_ACK AYNI-param_hash'i-echo-eder.
  **[B4-INVARIANT]:** `1 ≤ K_actual ≤ K_shape ≤ 255` (Alpha; K_shape-u16-format-korunur, üst-sınır-255 — shard-
  k_actual-u8-uyumu) · `E ≥ 11` (prefix-sığar) · ihlal → profile-reject-fail-closed.
  **STATE-MACHINE [B2-uyumlu — tek-tanım, §2.57-retry-tablosuyla-aynı]:** sender PARAM_SET → bounded-retry
  (control_id-sabit) until PARAM_ACK-veya-deadline; **deadline→retry-durur, emission-SÜRER** (ACK=confirmation,
  admission-kapısı-DEĞİL); yalnız-KALICI-hash-mismatch → SESSION_CLOSE.
· Unsupported/hash-mismatch → downgrade-olarak-FAIL-CLOSED-reddedilir. Generic-negotiation/provider-sistemi-DEĞİL. **DOWNGRADE-KORUMASI:** seçili-suite/profile Noise-transcript/channel-
binding'e-bağlı. Genel-provider-API-DEĞİL.

**DR-CS-1b-A2-AMENDMAN A-1 [Abkarada-onaylı; dar-kapsam — pre-param-hold aşım-politikası]:** Ürün-alıcı
yolunda pre-param bounded-hold'un aşım-davranışı **"aşımda-en-eski-drop" DEĞİL, rezervasyon-katmanı
TYPED-REJECT'tir** (INV-CS-RX-RESOURCE: "cap aşımı sessiz eviction değil typed diagnostic"). Admission
alt-kapısı: `RX_PREPARAM_SHARD_CAP_PER_SESSION = min(RX_COLD_TOTAL_SHARD_CAP_PER_SESSION,
MAX_PREPARAM_SHARDS)` — pre-PARAM admission preparam-cap VE total-cold-cap'i BİRDEN kontrol eder; hold'un
fiziksel kapasitesi aşılamadığından hold-içi en-eski-drop dalı ÖLÜ-KOD-PİNLİ kalır. `MAX_PREPARAM_SHARDS`
artırılacaksa engine-fiziksel-kapasitesi ve preparam-cap AYNI isimli-dilimde birlikte yükseltilir.
Yukarıdaki "(max_preparam_shards, küçük; aşımda-en-eski-drop)" ifadesi bu amendmanla ürün-yolunda
SUPERSEDE edilir (metin tarihsel-kayıt olarak korunur); wire-layout/PARAM-akışı DEĞİŞMEZ.

## 9. NOISE-GÜVENLİK-SINIRI (go/no-go-§3.6) [NORMATİF]
"Kama-wire kullanılan-N-türevi-handshake'in-EPOCH-BAZLI-özelliklerini-miras-alır: dh-epoch-0 = klasik-FS-DEĞİL (per-INIT-ephemeral-confidentiality; recipient-static-ele-geçerse-epoch-0-initial-çözülür), dh-epoch-1 = FULL-FS (SESSION_INIT_RESPONSE-sonrası). Profil-bunun-ÖTESİNDE-ek-FS/PCS-mekanizması-EKLEMEZ; REKEY-Alpha'da-cap→re-handshake (§5)." Modülerlik ≠ şifresiz-production-modu: "FEC-Noise'dan-bağımsız-olması production-Kama'nın-şifresiz-mod-sunması-
DEMEK-DEĞİL. Unit/POC: fec-core-tek-başına; sim/benchmark: güvenliksiz-harness; **PRODUCTION: Kama-Noise-ZORUNLU.**"

## 10. DoS (kripto-tarafı — fec-core-DoS'a-ek, SoC-analiz-§14)
KamaNoise forged-paketi AEAD'de-reddeder AMA auth'lu-peer-compromised-olabilir → fixed-cost-outer-parse (allocation-
öncesi) · replay-window-unauth-ilerletilmez · bounded-key-trial (trial-decrypt-DoS-sınırı). **AEAD-authenticity
resource-trust-DEĞİL** — fec-core-bounds (§13-fec-core-tdd) auth-sonrası-da-uygulanır.

## 11. KABUL-KRİTERLERİ (kripto-tarafı)
- **K-WIRE-NOISE-1:** production-build yalnız-normatif-Kama-Noise-profili; generic-provider/runtime-cipher-plugin-YOK.
- **K-WIRE-CRYPTEX-1:** FEC-semantic-metadata ciphertext-içinde; outer-cleartext yalnız-minimum-routing/PN/key-selection.
- **K-WIRE-ADMISSION-1:** AEAD-doğrulanmamış-datagramdan FEC-block-allocation/decoder-admission-YOK.
- **K-ID-SEPARATION-1:** logical-dedup-anahtarı ≠ kripto-replay-anahtarı.
- **K-RETX-1:** aynı-logical-source-retransmit yeni-full-PN+yeni-nonce.
- **K-REKEY-SOC-1:** FEC-codec/profile/golden-vector crypto-epoch'a-bağlı-DEĞİL.
- **K-NONCE-1:** PN-truncation-reconstruction + replay + wrap + dh-epoch-0→1-geçiş testleri. [current/prev-key-phase + rekey-reorder → BETA — Alpha-in-session-key-phase-YOK §5]
- **K-PRODUCTION-1:** FEC-tek-başına-test-edilebilirliği production-şifresiz-wire-mode-olarak-SUNULMAZ.
- **K-WIRE-DOS-1:** unauth-paket replay-window-ilerletmez; bounded-key-trial; fixed-cost-outer-parse.
- **K-WIRE-BIND-1:** transport-parameter-hash Noise-channel-binding'e-bağlı (zamanlama-A/B-seçili); downgrade-
  authenticated-reddedilir.
- **K-WIRE-CTRL-1:** TÜM-control-frame'ler §2.56-allowlist (ACK/SACK/RECOVERED/BLOCK_COMPLETE/PARAM_SET/PARAM_ACK/
  BLOCK_ABORT/SESSION_CLOSE/SESSION_RENEW_REQUIRED — KEY_UPDATE-YOK) secure-wire'dan-geçer;
  cleartext-feedback-YOK; sahte-recovered/ACK-forge-testleri.
- **K-WIRE-SIZE-1 (P0-3):** seal(p).len()==sealed_len(p) EXACT; pacer-reserve==gerçek-send-byte.
- **K-WIRE-SENDREC-1 (P0-4):** her-sealed-packet SealedPacketMeta-üretir; ACK→logical-eşleme-deterministik.
- **K-WIRE-PN-1:** 13-madde-PN-sözleşmesi-tanımlı; yeni-class-32-bit-KARAR (§2.5, tek-space); PN-width-DEMUX-
  mekanizması-mühürlü (P0-1-YOL-kararı); reconstruction-pseudocode + exhaustive-wrap/reorder + dh-epoch-0→1-test.
- **K-WIRE-KEYUPD-1:** Alpha-KARAR-VERİLDİ (§5): in-session-key-phase-YOK; cap (packet/byte/time) → tamamen-YENİ-
  handshake/session. Test: cap-tetiği-deterministik + eski-session-datagramı-yeni-session'a-admission-EDİLMEZ.
- **K-SCHEMA-1 (P0-8):** kama-transport-schema tek-sahip; iki-crate-kopya-struct-YOK; dependency-graph-döngüsüz.
- **K-WIRE-BOOTSTRAP-A:** forward-%90-loss-altında RECEIVER_ESTABLISHED bounded-retry-ile-oluşur (idempotent-
  duplicate + anti-amplification; hub-reflection-YOK; retry-amplification'a-dönüşmez).
- **K-WIRE-BOOTSTRAP-B:** reverse-%90-loss-altında SENDER_CONFIRMED eventual-olabilir (survivor-ACK; sender-
  bounded-retry-idempotent).
- **K-WIRE-BOOTSTRAP-C:** FS_UPGRADED gecikebilir-veya-gerçekleşmeyebilir; epoch-0-usable-KALIR ve-güvenlik-statüsü
  (klasik-FS-değil) açık-RAPORLANIR. Pattern-DEĞİŞMEZ (§0.7-A/§0.9). Test-matrisi: fwd/rev {0/90,90/0,90/90,burst}×{A,B,C}.
- **K-WIRE-NONCE96-1 [DR-4-kapanış-kanıtı]:** golden'lar nonce96-BYTE-DİZİSİNİ-doğrudan-assert-eder (full_pn ∈
  {0, 1, 2^32, 2^63}-örnekleri) + RFC-8439-resmi-vector'lar + libsodium-IETF-differential-cross-check (CI-oracle;
  runtime-backend-switching-YOK) + random-{key,nonce,AAD,pt}-corpus-eşitliği.
- **K-WIRE-AAD-1:** outer-alanlar-byte-level + AAD-canonical (golden-vector-AAD-kapsar); iki-impl-aynı-AAD.
- **K-WIRE-AEADFAIL-1:** AEAD-fail silent-drop+log, replay-window-ilerlemez, state-poisoning-yok, DoS-vektörü-değil.
- **K-WIRE-TRIAL-1:** key-trial-sayısı-sabit-küçük; trial+decode-timing-uniform (side-channel-sızdırmaz).
- **K-WIRE-RESTART-1:** process-restart/session-resume-sonrası PN-persistence-tanımlı; nonce-reuse-YOK.
- **K-WIRE-NONREG-1 (§0.9, Abkarada-kısıtı):** mevcut-N-türevi-SR_SM3-one-way-handshake (§0.7-A) + Split()+dh-epoch-upgrade + SESSION_DATA-zarfı +
  KEYSTONE-probe + PRE_CONVERGE DEĞİŞMEZ (noise-wire-OKUR+FEC-seal-EKLER). L4-wire-değişikliği AYRI-DECISION+onay+
  mühürlü-test-olmadan-YOK.

## 11.5 SHARED-SCHEMA-OWNERSHIP [NORMATİF — KARAR VERİLDİ, PATCH-7]
**Karar:** `kama-transport-schema` = primitive-tipler + logical-enum'lar + **Unsealed-DTO'lar** · `kama-noise-wire`
= sealed/private-**OpenedTransportPacket** · `transport-core` = recovery-orchestration · `fec-codec` = yalnız-FEC-
primitive/adaptörleri · `engine` = modülleri-birleştirir. (Reddedilen-yanlışlar: FEC-sahiplenirse noise→fec-
bağımlı; noise-sahiplenirse transport→noise; kopya=drift; engine'de=ters-bağımlılık.)
**`kama-transport-schema`-içeriği:** logical-packet-enums · canonical-serialization ·
block/ESI/profile-primitive-types · UNSEALED-packet-DTO'ları (Opened-DEĞİL — o-noise-wire-owned-sealed, §6) · wire-length-contracts · protocol-errors.
**İÇERMEZ:** Noise-state · AEAD · RS-codec · CC · socket · identity/trust. Bağımlılık: schema ← {transport-core,
noise-wire} ← engine; fec-codec yalnız-gerekli-primitive'leri-VEYA-adapter'la.

## 12. noise-wire-D0-A (KRİTİK-YOL — dördüncü-geçiş-§18-19, raporlar-genişletildi)
**GO (mevcut-Noise-kodunu-incele — RAPOR-üret, kod-YAZMA):**
- **N1 packet-identity/nonce:** seq-2B-ne? Noise-nonce-16-bit-seq'ten-mi? full-PN? truncation-reconstruction? anti-
  replay-full-PN? dh-epoch-penceresi (0/1)? unauth-replay-window-ilerletir-mi? retransmit-yeni-nonce/PN?
  **KIRMIZI: seq-2B-doğrudan-tek-başına-AEAD-nonce → production-DURDUR.**
- **N2 Alpha-lifecycle-uyum:** mevcut-kod Split()+dh_epoch-0→1-modeline-uyuyor-mu? in-session-key-phase/rekey-kodu-VARSA-Alpha-yolundan-kaldırılacak-envanteri.
- **N3 [UYUM-DOĞRULAMASI — karar-VERİLDİ]:** FEC-shard = AYRI-EXTENDED-class (§2.5, karar-KESİN — yeniden-
  seçilmez). Rapor: mevcut-fec.rs-FEC_SHARD-yolu-EXTENDED-layout'a (§2.6) uyuyor-mu? hub-h_ext-demux'u-opaque-mı?
  gerçek-outer-overhead == 26B-mi (§2.6-EXTENDED)?
- **N4 gerçek-wire-overhead:** outer_hdr+inner_hdr+AEAD_tag-byte (fec-core-PMTU-bütçesi-§11-ortak-için).
- **N5 admission:** verify-before-admission mevcut-Noise-open-yolunda-nerede? AEAD-fail-davranışı?
- **N6 [UYUM-DOĞRULAMASI — final-allowlist §2.56/§2.57]:** mevcut-kodda control-frame-VARSA §2.56-codepoint'lere
  (0x10..0x18) uyuyor-mu? **NACK/KEY_UPDATE-YOK** (NACK→SACK; KEY_UPDATE→Alpha-dışı). Eksik-control'lar-D1'de-
  §2.57-body'lerine-göre-eklenir.
- **N7 wire-sizing/PMTU-raporu (P0-3 — YENİ):** exact-outer-bytes · packet-class-başına-exact-inner · AEAD-tag ·
  padding · IPv4/IPv6/tunnel · sealed_len()-sözleşme-taslağı · profile-başına-max-E · sub-kbps-serialization-tablosu.
- **N8 send-record/ACK-accounting-raporu (P0-4 — YENİ):** PN→logical-map · send-timestamp · sealed-byte · retx-
  generation · repair/source/control-class · ACK/loss-terminal-states · recovered-etkileşimi · BtlBw-credit.
- **N9 Alpha-key-lifecycle-uyum-raporu (karar-VERİLDİ §5):** mevcut-kodda wire-visible-key-phase/rekey-state-
  machine-var-mı (varsa-Alpha-yolundan-kaldırılır)? cap-tetikleri-yeni-session/handshake-açıyor-mu? eski-session-
  packet'ı-yeni-session'a-kabul-EDİLMİYOR-mu?
- **N10 [UYUM-DOĞRULAMASI — flight-machine-KARAR §0.7-B]:** survival = bounded-idempotent-INIT-retransmission-
  flight-machine (KESİN — üç-seçenek-yeniden-sıralanmaz; bootstrap-FEC-Alpha'da-YOK). Rapor: mevcut-PRE_CONVERGE/
  KEYSTONE cached-byte-for-byte-INIT-retransmit-yapıyor-mu (MADDE-5)? retry-sabitleri (count/süre/backoff) ne?
  anti-amplification-var-mı? forward-%90→A, reverse-%90→B/C-davranışı-ölçülür.
**KİLİTLER (fec-core-D0-bunu-bekler):** symbol-boyu/PMTU-final · production-admission · sealed_len-sözleşmesi-
final · PACKET-enum-arayüzü (P0-1) · SealedPacketMeta (P0-4) · schema-crate-ownership (P0-8).
**D0-B-ORTAK-GATE:** normatif-liste = belge-sonundaki-KAPANIŞ-TABLOSU (16-gate, TÜMÜ-CLOSED — iki-belgede-byte-for-byte-aynı). **D1-implementation-GO-şartları: STATÜ-MERDİVENİ.**

---
**SONRAKI:** wire-tasarımı-KİLİTLİ (byte-layout §2.6/2.57, AAD §2.8) → D1-ENTRY-şartı = **N1-N10-UYUM-raporları**
(seçim-DEĞİL) + fec-core-POC ∥ ortak-entegrasyon-
gate → D1. **[GÜNCELLENDİ: wire-tasarımı-KİLİTLİ §2.5-2.8 — bu-eski-kısıt-kalktı.] seq-2B/nonce/rekey/AAD kod-görülmeden ONAYLANMAZ.**

## 13.5 KRİPTO-PROFİLE-FREEZE-CHECKLIST [SÜREÇ-NORMATİFİ — DR-4-dersinden] (her-crypto-profile-freeze'de-ZORUNLU)
Semantik↔serialization-boşluğu-sınıfını (DR-4: full_pn-tanımlı-ama-12B-nonce-mapping-tanımsızdı) yapısal-önler:
```
□ key-derivation  □ key-scope  □ direction-separation  □ epoch-separation  □ AEAD-algorithm+varyant
□ nonce-width  □ NONCE-EXACT-BYTE-MAPPING  □ counter-başlangıcı  □ counter-monotonicity/no-reuse
□ counter-wrap/cap  □ AAD-exact-bytes  □ plaintext-exact-bytes  □ tag-placement  □ decrypt-failure-state-etkisi
□ replay-window-commit-order  □ restart-semantics  □ INDEPENDENT-GOLDEN-ORACLE  □ RFC/standard-vector
□ cross-implementation-vector
```
Self-generated-golden TEK-BAŞINA-YETMEZ (yanlış-davranışı-da-sabitler): en-az {spec-elle-yazılmış-byte-örnekleri +
RustCrypto-çıktısı + bağımsız-libsodium-IETF-çıktısı} üçlüsü. Deprecate-edilen-her-appendix-için "hâlâ-normatif-
tek-satırı-var-mı"-taraması ZORUNLU (D2-tuzağı).

## D0-B ORTAK-GATE KAPANIŞ-TABLOSU [iki-belgede-BYTE-FOR-BYTE-AYNI — PATCH-G]
| Gate | Karar | Durum |
|---|---|---|
| Shared schema ownership | kama-transport-schema yalnız unsealed DTO/primitives | CLOSED |
| Opened ownership | Yalnız kama-noise-wire, private constructor | CLOSED |
| Packet enum | FEC source/repair + listed secure controls | CLOSED |
| Canonical source metadata | Source-specific metadata coded prefix'te | CLOSED |
| K_actual | Her authenticated FEC shard inner header'ında | CLOSED |
| sealed_len | Exact per-class formulas (EXT=26+inner, legacy=22+payload) | CLOSED |
| SealedPacketMeta | PN→logical deterministic mapping (PN Crypto'nun; K2) | CLOSED |
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
| dh_epoch transition | 5-adım confirmation-safe machine; grace = (N_g AEAD-geçerli epoch-1 paketi) VEYA (T_g monotonic süre), HANGİSİ ÖNCE — KS-GRACE-1, sRTT'siz; değerler KALİBRASYON | CLOSED (semantik) |
| K-width invariant | 1≤K_actual≤K_shape≤255; E≥11; fail-closed | CLOSED |
| AEAD nonce96 mapping | DR-4 KOŞUL-A LE64-lock (§4.5); envanter=NO_VECTOR/debt=NO | CLOSED |

> **[SUPERSEDED — 2026-09-19]** Aşağıdaki paragraf DR-4 KAPANMADAN ÖNCEKİ geçici durumu anlatıyordu; DR-4 CLOSED (KOŞUL-A, LE64, §4.5) ve kapanış tablosu "AEAD nonce96 mapping: CLOSED" olduğu için artık geçersizdir. Tarihsel kayıt olarak üstü çizili bırakıldı.
> ~~**Tüm-ÖNCEKİ-gate'ler-CLOSED; AEAD-nonce96-mapping-OPEN.** Envelope/wire-layout-freeze-KORUNUR;~~
> ~~cryptographic-profile-freeze DR-4-kapanana-kadar-ASKIDA. [DR-4-koşulu-uygulanıp-§4.5-işlendiğinde: OPEN→CLOSED →~~
> ~~FULL-DESIGN-GO-yeniden-aktive.] (Önceki-"11-gate"-sayımı-alt-kararların-birleştirilmesiydi; normatif-~~
> ~~liste-BU-tablodur.)~~

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
