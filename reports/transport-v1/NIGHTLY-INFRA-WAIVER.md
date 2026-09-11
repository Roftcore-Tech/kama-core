# GECELİK CI — ALTYAPI FERAGATI (INFRA WAIVER)

Tarih: 2026-09-11 · Kapsam: Faz-4'te yeniden yazılan `.github/workflows/nightly.yml` hostile
shard'ları ve manifest kapısı (`hostile` job'u, satır 116–176).

## Neden hosted/self-hosted CI'da koşturulamadı

Kanıt (2026-09-11, `gh` ile doğrulandı):

| kalem | durum |
|---|---|
| self-hosted netem runner | `kama-netem-01` **online**, etiketler `self-hosted,Linux,X64,netem` |
| repo değişkeni `NETEM_RUNNER` | **TANIMSIZ** (`gh variable list` boş) → `if: vars.NETEM_RUNNER == 'true'` hiç sağlanmıyor, hostile shard'lar kuyruğa GİRMİYOR |
| `origin/main` | `75d8990a` — Faz-4 CI onarımından ÖNCEKİ ağaç; bu dal (`audit/transport-v1-closure`) 37 commit ileride, 0 geride, **PR açılmamış** |
| gecelik koşular (son 5, günlük) | hepsi `origin/main`'deki ESKİ iş akışında `failure`: kani / fuzz-deep ×6 / branch-coverage / netem-gate (install-action tool girdisi eksikliği — PR #48'in kök nedeni) — Faz-4 iş akışı hiç çalışmadı |

`NETEM_RUNNER` değişkenini tanımlamak ve dalı `main`'e merge etmek **sahip yetkisindeki**
işlemlerdir (repo politikası: merge açık onay ister; ayrıcalıklı işlemler sahibe aittir).
Bu yüzden yeniden yazılmış gecelik adım, doğrulama kampanyasını etkilemeden bu görevde
tetiklenemedi.

## Hosted CI DIŞINDA ne doğrulandı

Gecelik adımın çalıştırdığı **aynı betik ve aynı kapı mantığı** yerel doğrulanmış netem
harness'ında (üç netns, `unshare -rnm`, rootless) koştu:

| ne | komut | sonuç |
|---|---|---|
| aktivasyon kanıtı (gecelik adım 1) | `bash scripts/netem/v1_activation_proof.sh <out>` | 3/3 hücre `profile=kama-v1-full`, manifest üretildi, fairness BLOCKING beklendiği gibi (Faz-4 M5) |
| manifest kapısı (gecelik adım 2 mantığı) | `arm == kama-v1-full ∧ profile_delta == []` her manifestte | Faz-5: **330/330** Kama manifesti kanonik, sapma 0 |
| hostile shard'ların asıl içeriği | `scripts/netem/phase5_campaign.sh` T0–T7 | 990/990 koşu, 11/11 shard DONE, 0 BARRIER-FAIL, 0 harness/infra arızası |
| runner-benzeri dayanıklılık | seri shard, timeout, DONE damgası, devam-edebilirlik | ERRATUM-1 sonrası sıfırdan yeniden koşum sorunsuz |

## Ne HÂLÂ denenmedi

* `nightly.yml`'in **YAML adımının kendisi** gerçek runner'da (`runs-on: [self-hosted, linux, netem]`)
  — `NETEM_RUNNER` gate'i ve `python3` ile manifest okuma satırları CI ortamında çalışmadı.
* `preflight` adımındaki `sudo modprobe sch_netem` yolu runner'da.
* Artifact upload (`actions/upload-artifact@v4`) hostile shard için.

## Gelecekte nasıl etkinleşir (sahip adımları)

1. Bu dalı PR ile `main`'e merge et (iş akışı `main`'den okunur).
2. Repo değişkeni: `gh variable set NETEM_RUNNER --body true`.
3. Bir kez elle tetikle: `gh workflow run nightly.yml -f run_hostile=true` ve
   `hostile-*` artefaktlarında `manifest.json` → `arm == kama-v1-full` doğrula.
4. Yeşil sonrası bu feragat "KAPANDI" damgasıyla güncellenir.

## Sınıflandırma

**INFRA_FAILURE / INFRA WAIVER** — transport arızası DEĞİLDİR. Transport v1 kanıtı gecelik
CI'dan değil, yerel doğrulanmış harness'tan gelir ve bu feragatla birlikte okunur.
