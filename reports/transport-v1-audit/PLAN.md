# TRANSPORT v1 KAPANIŞ DENETİMİ — PLAN (2026-09-08)

Soru: tasarlanan transport mimarisi gerçekten uygulanmış, ürün yoluna kablolanmış, amaçlanan koşullarda açık,
birlikte çalışıyor ve benchmark'ta adil biçimde temsil ediliyor mu? Yeni mekanizma İCAT EDİLMEZ.
Ayrım: **belirtilmiş ≠ uygulanmış ≠ kablolu ≠ açık ≠ fiilen çalışmış ≠ benchmark'lanmış**.

## Fazlar ve çıktıları
| Faz | İş | Yöntem | Çıktı |
|---|---|---|---|
| 0 | Envanter (spec, crate, modül, bayrak, bench kolları) | inline keşif | bu plan |
| 1 | SPEC → CODE: 12 mekanizma kümesi × (bul → çürütmeye çalış) | workflow, ajan başına küme; adversaryal doğrulama | `phase1-raw/spec2code-*.json` |
| 2 | CODE → SPEC: modül süpürmesi (ölü/kablosuz/duplicate/feature-gated/bench-only) + default-OFF sicili | workflow, modül başına ajan | `phase1-raw/code2spec-*.json`, `flags.json` |
| 3 | Sentez: mimari kapsama matrisi, default-OFF sicili, kablosuz-kod sicili, WIRE blocker listesi, **ürün v1 profili** taslağı | ben (yargı) + sentez ajanı (taslak) | `COVERAGE-MATRIX.md`, `DEFAULT-OFF-REGISTER.md`, `UNWIRED-REGISTER.md`, `PRODUCT-PROFILE-v1.md` |
| 4 | Runtime aktivasyon kanıtı: SHIP mekanizmaları için hedefli smoke (sayaç/geçiş/parity/retx/tail-probe/cold-start) + **mimari-aktif manifest** (bench başlangıç/bitiş) | küçük netem koşuları (n=1–3), bench manifest kodu | `ACTIVATION-PROOFS.md`, bench `--manifest` |
| 5 | Benchmark adillik denetimi: kama-product vs quinn stream/datagram semantik eşleme (güvenilirlik, teslim tanımı, deadline, payload, kurulum, tel muhasebesi, reverse-path) | kod okuma + küçük eşleşme testleri | `FAIRNESS-AUDIT.md` |
| 6 | Düzeltmeler: yalnız WIRE / gerekçesiz default-OFF / uyumsuz-entegrasyon; en küçük değişiklik; kapı yeşil | dal `audit/transport-v1-closure` (main PR #51 sonrası) | commit'ler |
| 7 | Hedef-rejim matrisi T0–T7 (tam v1 profili, manifestli) vs Quinn stream/datagram (+TCP varsa); hücre başına hüküm (DOMINATES/WINS/INCONCLUSIVE/INVALID/BLOCKED-…) ; kayıpta 1–7 sınıf soruşturması | netem, önce deney bütçesi kartı (soru/beklenti/çürütme/bütçe/karar) | `TARGET-READINESS.md` |
| 8 | Karar raporu (13 bölüm) + tek satır: READY / BLOCKED BY | | `TRANSPORT-V1-AUDIT-REPORT.md` |

## Kurallar
- Benchmark durdurma kuralı: kapsama matrisinde açıklanmamış WIRE / gerekçesiz default-OFF kalırken T-matrisi KOŞULMAZ.
- Her mekanizma tek sınıf: SHIP / WIRE / KEEP-EXPERIMENTAL / DELETE. "belki sonra" yok.
- Kanıt = dosya:satır + çağrı zinciri; ad/yorumdan çıkarım yok. Ulaşılabilirlik grep'le kanıtlanır.
- Hostile rejimde Quinn/TCP üstünlüğü = önce 1–7 sınıf soruşturması; yalnız hepsi temizse "gerçek mimari başarısızlık" (BLOCKER).
- Cargo tek süreç (-j2); netem koşarken cargo yok; her deney önce beklenti kartı.
- FEC kararı (bounded_v1, Path A) otoritedir; FEC araştırması yeniden açılmaz; yalnız ürün-durumu denetlenir.

## Bilinen ön-bulgular (doğrulanacak, sonuç değil)
- `RuntimeConfig::default()`: `fec: None` (FEC'in kendisi ürün default'unda kapalı) — v1 profili FEC'i hangi profille açar? belirlenmeli.
- `bootstrap_rtt: false` (DR-CS-3 "KALİBRE EDİLMEMİŞ — default flip ölçüm ister") — cold-start mimari gereği mi? WIRE/default adayı.
- `experimental-cold-start` (DR-CS-1 F2A), `experimental-fec-budget` (budget_kernel) cargo feature'ları default OFF.
- `LossConfirmHorizon::CadenceLegacy` default (kanal-bağımsız 31 B/s sabiti; spec §16 kanal-türevi ister — çelişki kayıtlı).
- Önceki Quinn kıyasları FEC'siz "kama" stub koluyla koştu (bench-stub-vs-product-runtime) → geçersiz tam-mimari kıyası.
- Bench: `kama` (stub), `kama-product`, `quic` (stream), `qdgram` (datagram); TCP kolu var mı? denetlenecek.
