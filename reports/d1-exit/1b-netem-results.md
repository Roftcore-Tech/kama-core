# AG — GERÇEK-NETEM Sonuçları (ROOTLESS-USERNS; admin-GEREKMEZ) [production-default-DEĞİŞMEDİ]

> Kullanıcı-talimatı: "önceki-testlerin-admin'siz-container'ları". İki-admin'siz-yol-da-doğrulandı:
> (a) `unshare -r -n` user-namespace (KULLANILDI — daemon'suz; ROOTLESS-NETEM-OK-probu),
> (b) repo harness/docker-compose (cap_add=NET_ADMIN; script-AF-hazır). Koşu: rootless-netns,
> `scripts/netem/run_rootless_netns.sh`, GERÇEK-UdpSocket-over-veth + GERÇEK-tc-netem.
> **100-run / 0-failure** (5-profil × {L,P15,F15}(+F10-blackout) × 5/10-rep). ENV/kernel-kaydı +
> ham-özetler: [data/netem/](data/netem/). PRE-DECLARED-reps=5/p7=10 (wall-time; deterministik-katman-
> birincil-kanıt — netem=YÖN/ROBUSTLUK-doğrulaması, packet-birebir-değil).

## Ne-KANITLANDI (mekanizma-yönü; netem-altında)
F15-mekanizması GERÇEK-stack + GERÇEK-impairment (burst-loss/GE/blackout-10s/blackout-30s/underutil)
altında REGRESYONSUZ ve DOĞRU: her-profilde **ticket 12/12-on-time · miss=0 · late=0 · waste=0 ·
displacement bounded (0-1200B) · crash/hang-yok · cap/lifecycle-ihlali-yok**. Leftover/P15/F15 üçü-de
tam-emit (48/48) → **experimental-yol gerçek-stack'te Leftover'a-regresyon-getirmiyor** (feature-ON-
robustluk-kanıtı).

## Ne KANITLANMADI (dürüst-sınır — named-residual)
Outcome-SEPARASYONU (F15>P15 decode/completion) bu-netem-ölçeğinde REPRODUCE-OLMADI. İki-neden:
(R1) **UNCONTENDED:** runner-96-unit + cömert-horizon → tüm-arm'lar-tam-tamamlıyor (starvation-yok →
ne-fayda-ne-zarar-ayrışıyor). (R2) **RUNNER-RECEIVER-OUTCOME-BOŞLUĞU:** UDP-runner-özeti budget-
telemetrisi-taşır (emit/waste/displacement/ticket) ama receiver-decode/block-completion/deadline-
TAŞIMAZ (rx-ledger-yok). Sonuç: deterministik-katmanın-outcome-yönü (F15-recovery-üstünlüğü + blackout-
harm) netem-tarafından-BAĞIMSIZ-DOĞRULANMADI; netem yalnız-mekanizma-robustluğunu-doğruladı.

## Sim↔netem yön-tablosu (mekanizma-boyutları-dolu; outcome-boyutları-residual)
| boyut | deterministik | netem | uyum |
|---|---|---|---|
| F15-ticket-servisi | on-time-baskın | **12/12-on-time (5-profil)** | ✓ UYUMLU |
| reservation-waste | 0 | **0 (100-run)** | ✓ UYUMLU |
| displacement-bounded | ≤envelope | **0-1200B bounded** | ✓ UYUMLU |
| cap/lifecycle-ihlali | 0 | **0** | ✓ UYUMLU |
| Leftover-non-regression | özdeş | **tam-emit-regresyonsuz** | ✓ UYUMLU |
| F15>P15 decode | +15.5 [9.5,19] | ölçülmedi (R1+R2) | ○ RESIDUAL |
| operational-completion | rev99 +0.100 | ölçülmedi (R1+R2) | ○ RESIDUAL |
| blackout-30s harm | 10/seed (dark-start) | reproduce-olmadı (R1) | ○ RESIDUAL |
| tail-latency-yönü | bounded | uncontended-nötr | ○ RESIDUAL |

Netem-TERS-yön-BULUNMADI (mekanizma-boyutlarında; §14-DUR-tetiklenmedi). Outcome-boyutları-nötr-değil,
ÖLÇÜLMEDİ.

## Kalan-adımlar (residual-kapatma; ayrı-kapı)
(1) rx-tarafına block-completion/decode-ledger ekle (receiver-outcome). (2) Netem'i CONTENDED-ölçeğe-
al (büyük-w_units + dar-horizon + p5-offered≈C/p6-30kbps saturation) → deterministik-separasyonu-
reproduce-dene. İkisi-de deterministik-bulguyu-değiştirmez; netem-outcome-yönünü-bağımsız-teyit-için.
