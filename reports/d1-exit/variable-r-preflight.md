# VARIABLE-R PREFLIGHT — kanıt-raporu (Commit-E; §6.5) — SONUÇ: SÖZLEŞME-KANITLANDI

> Testler: `kama-engine/tests/variable_r_preflight.rs` (5-test; production-actuation-kodu-YOK).
> Profil: 0x03 (K=8, E=64, Rmax=4). Kod-satır-kanıtları: codec.rs:84-92/210-227 (repair-esi
> `< r_code_max`-BOUND; exact-R-yok; sufficiency-decode) · receiver.rs:40 (r-alanı-kalkmış).

## Test-matrisi-sonucu

| Test | k_actual | requested-r | emitted | received-repair | decode | state-cleanup | wire/profil-değişimi |
|---|---|---|---|---|---|---|---|
| A: r=0 kayıpsız | 8 | 0 | 0 | 0 | ✅ source-only | yapısal (stateless-fn) | GEREKMEDİ |
| B: r=1, 1-kayıp | 8 | 1 | 1 | 1 | ✅ recovered | ✓ | GEREKMEDİ |
| C: r=Rmax=4, 2-kayıp | 8 | 4 | 4 | 4 | ✅ recovered | ✓ | GEREKMEDİ |
| D: r=2 kayıpsız | 8 | 2 | 2 | 2 | ✅ | ✓ | GEREKMEDİ |
| r=1, 2-kayıp | 8 | 1 | 1 | 1 | ✅ typed-FAIL (`FecDecode`; stuck-değil) | ✓ | — |
| r=4 emit, 2-ulaştı, 2-kayıp | 8 | 4 | 4 | **2** | ✅ recovered (**dinamik-kabul**) | ✓ | GEREKMEDİ |
| out-of-order (repair-önce) | 8 | 4 | 4 | 4 | ✅ | ✓ | GEREKMEDİ |
| duplicate-repair | 8 | 2 | 2 | +1-tekrar | wire-replay-RED (`PnReject`) + decode ✅ | ✓ | — |
| r=0 → sonraki-blok r=4 | 8 | 0→4 | 0→4 | — | ✅ her-iki (leak/etki-yok) | ✓ | GEREKMEDİ |
| partial k_actual=5 × r=2/r=0 | **5** | 2 / 0 | 2 / 0 | 2 / 0 | ✅ / ✅ | ✓ | GEREKMEDİ |
| interleaved blok-ID (r=2 & r=1) | 8+8 | 2,1 | 2,1 | 2,1 | ✅ bağımsız | ✓ | GEREKMEDİ |

## §6.6 failure-branch-taraması: HİÇBİRİ-OLUŞMADI
Receiver-exact-R-beklentisi YOK ✗ · profil-ID-emitted-R-bağı YOK ✗ (r_code_max=ÜST-SINIR) · r=0-leak/stuck
YOK ✗ (stateless `admit_and_decode`; hata=typed) · ardışık-farklı-r-decoder-bozulması YOK ✗ · mevcut-API-
yetersizliği YOK ✗ (`encode_seal_block_with_r`-test-helper-yeterli) · wire-değişikliği-GEREKMEDİ ✗.

**HÜKÜM: DR-1a-gate-1 (variable-r-contract) → PASS.** Not: bu-preflight TEST-harness-seal-yoluyla; DR-1a-impl
aynı-matrisi engine-emisyon-yolundan (enqueue×r_final) TEKRAR-koşacak (impl-commit-şartı; DR-1a-v2-§3-planı).
