# BYTE-HARİTASI — transport + kripto katman-yığını (2026-08-28; koddan/spec'ten doğrulanmış)

Abkarada-sorusu: "header/body bölümleri ne, kaç byte; aynı bilgiyi transport'a VE kriptoya iki kez mi
koyuyoruz?" Cevap iki tabloda: BUGÜNKÜ-interim (wire-v0, S2'de swap'la ölecek) ve S2-HEDEF (spec'in
gerçek yığını). Sonda: nerede çift-taşıma var, hangisi gerekçeli, hangisi gerçek düzeltme-adayı.

## 1) BUGÜNKÜ interim ürün-wire'ı (client-core wire-v0 — BİLİNÇLİ-GEÇİCİ, S2 tek-nokta swap)

| çerçeve | başlık-düzeni | başlık | + gövde |
|---|---|---|---|
| DATA (0x00) | type1 + seq u16 + **unit_id u64** | **11 B** | payload |
| FEC (0x01) | type1 + kind1 + profile1 + block u32 + esi u16 + k1 + seq u16 | **12 B** | symbol (E; 0x03→64 B) |
| RECOVERED (0x02) | type1 + block u32 + count1 | 6 B | 2×n esi (≤16) |
| PARAM (0x03) / ACK (0x04) | sabit | 35 / 33 B | — |
| motor-ACK V2 (0x14) | type+epoch+largest+delay+block0+rc | **15 B** | 4×range (≤16) + seen4 + rate2 dahil |
| seam (IdentitySeam stub) | sahte-tag | **+16 B** | (S2'de gerçek AEAD-tag olur) |

**Interim'deki gerçek şişkinlikler (S2'de kendiliğinden ölür; şimdi optimize ETME):**
`unit_id u64` = 8 B (FEC-açıkken `block×K+esi`'den türetilebilir — dedup için taşınıyor) ·
FEC-çerçevesindeki `seq2` (S2'de outer-PN devralır) · stub-tag.

## 2) S2-HEDEF yığını (kama-noise-wire sealed + ccr §35 E2E) — spec'in gerçek tasarımı

**KRİTİK NORMATİF GERÇEK (transport-layer-tdd:23,629 + app-rpc AC-APP-2):** kurulu (established)
hot-path mesajı FEC/outer-AEAD zarfına **DEFAULT GİRMEZ**:

| yol | katman-yığını | toplam-ek |
|---|---|---|
| **Established hot-path (FEC-siz)** | `hub_handle4 + seq2 + ct + tag16` — outer-AEAD/FEC/prefix = **0 B** (tdd:629) | **22 B** — TEK katman, ÇİFT-TAŞIMA YOK |
| **FEC-korumalı yol (Pipeline C/D)** | E2E-mesaj (22 B ek dahil) → SourcePrefix **11 B** (msg_id4+chunk_off4+len2+flags1) → FEC-inner **11 B** (ver1+kind1+flags1+profile1+block4+esi2+k1) → outer **26 B** (h_ext4+ver1+flags1+pn_trunc4 **+tag16**) | 22 + **48 B/shard** (sealed-FEC = 26+11+E; E=64→101 B/shard) |
| Kontrol (sealed) | outer 26 + control-prefix 6 + gövde | 32 B + gövde |

## 3) "Aynı bilgi iki kez mi?" — dürüst döküm

| aday | hüküm |
|---|---|
| **ÇİFT-TAG (FEC-yolunda E2E-tag16 + outer-tag16 = 32 B)** | **GERÇEK ama GEREKÇELİ (INV-FEC-1):** alıcı RS-decode'a girmeden HER shard'ı doğrulamak ZORUNDA (authenticate-before-decode; forge-shard decode'u zehirler) → bunu ancak shard-seviyesi outer-tag yapabilir. İç-tag ise E2E: hub bile sahteleyemez. İkisi FARKLI tehdidi kapatır. Yalnız FEC-yolunda ödenir; established-yolda tek-tag (spec bunu zaten mühendisliklemiş, tdd:23). |
| Üç "sıra-numarası" (E2E seq2 · FEC block4+esi2 · outer PN4) | Roller ayrık (E2E-replay / blok-koordinat / hop-nonce) — birleştirme = katman-ihlali. ANCAK S2-planında motor-seq→wire_PN TEK sayaç (SealArgs.wire_pn = engine-seq; oturumda karar verildi) → hop'ta ekstra sayaç YOK. |
| **SourcePrefix şişkinliği (11 B/symbol)** | **GERÇEK OPTİMİZASYON-ADAYI:** tek-chunk birimlerde `chunk_offset4` HEP 0; `message_id4`, E2E-seq'le rol-çakışık. Kılavuza ek-öneri: "tek-chunk profilinde kısa-prefix (len2+flags1=3 B) varyantı" → shard-başına **−8 B** (E=64'te sembol-alanının %12.5'i geri kazanılır!). fec-core/ccr düzenlemesi gerektirir — SEN karar ver (kılavuz-A-ekine A11 diye eklenebilir). |
| ccr DEPRECATED-V2 per-message-HKDF zarfı | Zaten V3'le çözülmüş (o dedupu spec yapmış) — dokunma. |
| wire-v0 çiftleri (unit_id vs seq vs FEC-koordinat) | Interim-gerçeği; S2-swap'ta tek-forma iner (yukarıda). |

**Tek-cümle:** Spec'in HEDEF tasarımı senin korktuğun çift-taşımayı büyük ölçüde ZATEN engelliyor
(established=22 B tek-katman; "bağ byte'ta değil anahtarda" ilkesi) — gerçek çift yalnız FEC-yolundaki
iki-tag (tehdit-gerekçeli) ve SourcePrefix'in 8 B'lik şişkinliği (düzeltme-adayı, karar senin).
