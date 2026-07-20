# PR#43-MERGE-CHECKPOINT — Sprint-5a + 5b [Abkarada onayı bekliyor]

## HOLD-şartlarının durumu (üçü de karşılandı)
| Şart | Kapatan | Durum |
|---|---|---|
| Sprint-4-session-testleri | Sprint-4 (34 test) | ✓ (önceki-checkpoint) |
| cross-crate-integration | Sprint-5a (9 test) | ✓ |
| N1-legacy-interop | Sprint-5b (10 test) | ✓ (codec+sabit+demux-seviyesi) |

## Sprint-5a (cross-crate; kama-integration crate; mock-key; paylaşılan-koda DOKUNMADAN)
- Veri-yolu: app→chunk→prefix→FEC-encode(Model-A)→seal(EXTENDED,LE64)→open→admission→FEC-decode(known-zero)→app-EŞİT
- Kayıp-matrisi: source-kayıp+repair · karışık · yetersiz→fail-closed
- reorder+dup+replay (accept-entegre) + dh_epoch-0→1 pipeline-canlıyken
- Control-yolu: HANDLE_ASSIGN(hub_sim)→PARAM_SET→FEC→ACK/SACK(epoch)→RECOVERED(credit-YOK-assert)→BLOCK_COMPLETE→RENEW
- seal==sealed_len + PMTU-admission her-pakette

## Sprint-5b (N1-legacy-interop; ccr §35.6/§35.9 AKTİF-format; mock-key)
- Legacy 22B codec [hub_handle 4][seq16 2][ct][tag 16]; AAD §35.9 28B; nonce=LE64(§4.5)
- DR-3 legacy-sabit-seti (1024/4096/8192) extended'ten AYRI — iki-yol-iki-sabit assert
- HandleRouter demux (decrypt-öncesi; cross-handle-karışma-YOK); cross-direction-replay reddi
- 4-oracle: AAD-golden(28B) + full-wire-golden(45B) + legacy-differential(RustCrypto==libsodium) + RFC/mapping-shared
- SINIR: gerçek-Split-key-bağlama handshake-fazı (Sprint-6+); interop codec+sabit+demux-seviyesinde KAPANDI

## Kapı durumu
- noise-wire 90 test (default) + differential(libsodium 3) · kama-integration 9 test · toplam yeni 5a+5b: 19 test
- fmt ✓ · clippy -D warnings ✓ (default + all-features) · workspace-build ✓
- NON-REGRESSION: desen1_sim + adaptive-transport pin-seti YEŞİL (paylaşılan-koda dokunulmadı)

## Merge-atomiklik
main = D0 + DR-4-spec (012c5f4, nonce-boşluğu-yok). PR#43 = D1 Sprint-1..5b (base=main, rebased, temiz-diff). Draft.

## KARAR-TALEBİ (Abkarada)
5a+5b yeşil → **PR#43 draft-kalksın + main'e-merge?** Merge-sonrası Sprint-5c (send-record + FEC-emisyon; CERRAHİ,
paylaşılan-koda mini-DR-prosedürüyle) main-üstünde başlar. Onaysız 5c'ye GEÇİLMEZ.
