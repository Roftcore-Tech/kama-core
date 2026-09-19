# Dal (branch) defteri — kama-core

Güncelleme: 2026-09-19. **Tek doğruluk kaynağı `main`'dir.** Trunk-based: kısa dal → PR → squash merge;
`main`'e doğrudan push yok. Squash kullanıldığı için birleşmiş bir dal git'te "merged" GÖRÜNMEZ —
bir dalın içeriğinin main'de olup olmadığı bu defterden ya da PR durumundan okunur.

## Transport v1 nerede?

`main` (PR #52, `53e0c5c6`): Kama Transport v1 **donmuş ve belgelenmiş** durumda.
Başlangıç noktası: [`docs/transport-v1/README.md`](transport-v1/README.md) · nihai hüküm
[`reports/transport-v1/TRANSPORT-V1-CLOSURE.md`](../reports/transport-v1/TRANSPORT-V1-CLOSURE.md) ·
üst katman sözleşmesi [`docs/transport-v1/TRANSPORT-CRYPTO-HANDOFF.md`](transport-v1/TRANSPORT-CRYPTO-HANDOFF.md) ·
kanonik katman grafiği `specs/transport-layer-tdd.md` §0 INV-NEIGHBOR-1
(Application → Transport → Crypto → UDP).

## Yaşayan dallar

| dal | PR | durum | not |
|---|---|---|---|
| `main` | — | **kaynak** | hosted CI `core-ci` (PR kapısı) + `core-main` (main push) + `core-nightly` |
| `test` | yok | tarihsel (sahibi: h4s4nk44n) | SR-STR-CORE-001 test/CI seti; #47 ile main'e uyarlanarak entegre edildi, dal orijinal hâli. Başka geliştiricinin dalı — sahibine sorulmadan silinmez |

Bunların dışında görülen her dal kısa ömürlü bir iş dalıdır (açık PR'ı vardır, merge sonrası silinir).

## Arşiv ETİKETLERİ — dal değil, MERGE ETMEYİN

2026-09-19'da arşiv dalları **etikete** çevrildi: içerik ve SHA korunur, dal listesi temiz kalır.
Görüntüleme: `git fetch --tags && git log archive/<ad>` · geri yükleme: `git push origin archive/<ad>^{}:refs/heads/<ad>`.

| etiket | uç SHA | ne |
|---|---|---|
| `archive/turd1-reconciliation` | `4b0dbc08` | **ASLA merge edilmez** (`DO-NOT-MERGE-turd1.md`). TUR-D1 shrink/aktif-demotion: ölçümle kapatılmış tasarım kolu; kod yalnız kanıt zinciri için |
| `archive/tur-s-sensing` | `867c3e4f` | tek dosya `docs/design/tur-s-sensing-v1.md` — kapatılmış sensing tasarım notu |
| `archive/cift-kilit-not` | `0fa4360a` | tek dosya `docs/design/cift-kilit-v1.md` — tasarım notunun erken sürümü |
| `archive/fix-wip-sweep` | `286fdd50` | PR #32 kapalı; 4 doküman/spec WIP dosyası |
| `archive/pr1-pacer-bandwidth-domain` | `cb25ff26` | PR #22 kapalı; pacer bant-alanı denemesi, yerine TUR-4 log-domain çözümü main'de |
| `archive/campaign-burst-reproof-v1-closure` | `a26a6dfb` | PR #49 kapalı; burst-reproof-v1 kampanya kanıtı (1813 rapor dosyası, hüküm STILL-WORSE ×3); kod değil |
| `archive/d1-sprint5c` | `4aa11a93` | PR #44 (draft) kapalı; 224 commit — send-record + FEC emisyon + kama-engine çalışmasının kaynağı, içeriği sonraki PR'larla main'e taşındı |
| `archive/ci-all-branches-push` | `bd2ea4a5` | PR #53 kapalı; "her dal her push CI" — #52'deki `ci.yml` yeniden yazımıyla superseded, gerekçe PR yorumunda |

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
