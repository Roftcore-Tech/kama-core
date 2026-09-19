# Dal (branch) defteri — kama-core

Güncelleme: 2026-09-19. **Tek doğruluk kaynağı `main`'dir.** Trunk-based: kısa dal → PR → squash merge;
`main`'e doğrudan push yok. Squash kullanıldığı için birleşmiş bir dal git'te "merged" GÖRÜNMEZ —
bir dalın içeriğinin main'de olup olmadığı bu defterden ya da PR durumundan okunur.

## Transport v1 nerede?

`main` (`53e0c5c6`, PR #52): Kama Transport v1 **donmuş ve belgelenmiş** durumda.
Başlangıç noktası: [`docs/transport-v1/README.md`](transport-v1/README.md) · nihai hüküm
[`reports/transport-v1/TRANSPORT-V1-CLOSURE.md`](../reports/transport-v1/TRANSPORT-V1-CLOSURE.md) ·
üst katman sözleşmesi [`docs/transport-v1/TRANSPORT-CRYPTO-HANDOFF.md`](transport-v1/TRANSPORT-CRYPTO-HANDOFF.md) ·
kanonik katman grafiği `specs/transport-layer-tdd.md` §0 INV-NEIGHBOR-1
(Application → Transport → Crypto → UDP).

## Yaşayan dallar

| dal | PR | durum | not |
|---|---|---|---|
| `main` | — | **kaynak** | her zaman yeşil; hosted CI `core-ci` (PR) + `core-main` (push) |
| `ci/all-branches-push` | #53 açık | sahip kararı bekliyor | yalnız `.github/workflows/ci.yml`; #52 sonrası main'in ci.yml'i ile yeniden hizalanmalı |
| `campaign/burst-reproof-v1-closure` | #49 açık | kampanya arşivi | 400+ rapor/kanıt dosyası (burst-reproof-v1, hüküm STILL-WORSE ×3); kod değil |
| `d1-sprint5c` | #44 draft | tarihsel | 224 commit; send-record + FEC emisyon + kama-engine çalışmasının kaynağı, içeriği sonraki PR'larla main'e taşındı; draft olarak referans |
| `test` | yok | tarihsel | SR-STR-CORE-001 test/CI seti; #47 ile main'e uyarlanarak entegre edildi, dal orijinal hâli |

## Arşiv dalları — MERGE ETMEYİN

| dal | neden duruyor |
|---|---|
| `turd1-reconciliation` | **ASLA merge edilmez** (`DO-NOT-MERGE-turd1.md`). TUR-D1 shrink/aktif-demotion: ölçümle kapatılmış tasarım kolu; kod yalnız kanıt zinciri için saklanıyor |
| `tur-s-sensing` | tek dosya `docs/design/tur-s-sensing-v1.md` — kapatılmış sensing tasarım notu |
| `cift-kilit-not` | tek dosya `docs/design/cift-kilit-v1.md` — tasarım notunun erken sürümü |
| `fix-wip-sweep` | PR #32 kapalı; 4 doküman/spec WIP dosyası |
| `pr1-pacer-bandwidth-domain` | PR #22 kapalı; pacer bant-alanı denemesi, yerine TUR-4 log-domain çözümü main'de |

## 2026-09-19'da kaldırılanlar (içerik main'de; uç SHA'lar geri yükleme için)

| dal | uç SHA | gerekçe |
|---|---|---|
| `audit/transport-v1-closure` | `b012204c` | PR #52 squash-merged; main ağacı dal ucuyla bit-bit aynı (`da907681`) |
| `fix/fec-mixed-loss-reorder-final` | `15133be4` | PR #51 — 6 commit'in tamamı #52 içinde; superseded |
| `fix/ci-install-action-tool` | `19cb6bf4` | PR #48 — düzeltme Faz-4 CI yeniden yazımıyla main'de; superseded |
| `b1-kopma-korlugu` | `6615e0eb` | main'den 0 commit ileride, fark yok |
| `b2-rtt-hijyen` | `e1b8488f` | main'den 0 commit ileride, fark yok |
| `m45-kanal-durumu` | `0ca52033` | dokunduğu tek dosya main ile özdeş |

Geri yükleme: `git push origin <SHA>:refs/heads/<dal>` (PR dalları ayrıca GitHub "Restore branch" ile).

## Yeni dal kuralları

Ad: `<tür>/<kısa-konu>` (`feat/`, `fix/`, `docs/`, `ci/`, `spec/`, `campaign/`). Ömür ≤ 2 gün; merge sonrası dal
silinir (repo ayarı `delete_branch_on_merge = true`). Deney/kampanya çıktısı koda karışmaz: `reports/<kampanya>/`.
Transport davranışını değiştiren her PR `docs/transport-v1/EVOLUTION.md` §22'deki altı beyanı taşır.
