# DR-1b FORMAL PROBLEM-STATEMENT (Commit-Q) — imkânsızlık-üçgeni + entitlement-sınıfları + DATA-first-ayrımı

> DR-1b-v2'nin-matematiksel-tabanı. Statü: docs-only; production-değişikliği-YOK.

## 1. İMKÂNSIZLIK-ÜÇGENİ (bağlayıcı-kayıt)

Aşağıdaki-ÜÇ-şart AYNI-ANDA-SAĞLANAMAZ:

1. **(T1)** DATA sürekli-bütün-mevcut-token'ları-kullanabilir (resource-exclusive-priority).
2. **(T2)** Parity-için token-REZERVASYONU-veya-BORÇLANMASI-yoktur.
3. **(T3)** Saturated-rejimde (DATA-talebi ≥ token-arzı, süreğen) parity-için-SÜRE-GARANTİSİ-vardır.

Kanıt-özeti: T1∧T2 ⇒ saturated-rejimde parity'ye-akan-token = leftover = 0 (süreğen) ⇒ parity-servis-
süresi-sınırsız ⇒ ¬T3. (MS1-ampirik-karşılığı: offered-c1 realization-0.01; d1000-tavanı.) ∎
**En-az-biri-değişmelidir.** DR-1b-v1'in "floor-garantisi + rezervasyon-yok" birleşimi bu-üçgeni-ihlal-
ediyordu (blocker-1) — v2 sınıf-seçimini-açık-yapar.

## 2. POLİTİKA-SINIFLARI

| sınıf | T1 | T2 | T3 | tanım |
|---|---|---|---|---|
| **L — Strict-leftover** (baseline) | ✓ | ✓ | ✗ | DATA-önce-tüm-token; parity-yalnız-gerçek-leftover; floor = garanti-DEĞİL, **best-effort** |
| **R — Protected-parity-entitlement** | ✗ | ✓(rezervasyon-VAR-borç-yok)* | koşullu-✓ | Toplam-token'ın-BOUNDED-payı parity-için-KORUNUR; DATA-korunan-payı-tüketemez; talep-yoksa-pay-DATA'ya-döner (work-conserving); wire-sıra-DEĞİŞMEZ. *Bu-bir-**REZERVASYON**-biçimidir-ve-açıkça-böyle-adlandırılır.* |
| **B — Bounded-borrowing/debt** | ✓(anlık) | ✗(borç-VAR) | koşullu-✓ | Floor-parity gelecekteki-parity-bütçesinden-KESİN-cap'li-borç-alır; sonraki-accrual-önce-borcu-kapatır; debt-şişmesi-cap'le-engelli |

(*R-sınıfında T2'nin "rezervasyon-yok" yarısı-değişir; "borç-yok" yarısı-korunur.)

**Varsayılan-inceleme-yönü: R (Protected-parity-entitlement)** — talimat-tercihiyle-uyumlu; NİHAİ-SEÇİM
bu-turun-kernel-deneyine-bağlı (DR-1b-v2 §karşılaştırma).

## 3. "DATA-FIRST" TANIM-AYRIMI (bağlayıcı)

- **3.1 Ordering-first:** aynı-scheduling-turunda wire-decision-sırası DATA → ARQ/retransmit → parity.
  **KORUNUR (invariant; tüm-sınıflarda).**
- **3.2 Resource-exclusive-priority:** DATA-bütün-token'ları-sınırsız-kullanır; parity-yalnız-leftover.
  **FloorCap/Protected-modda KORUNMAZ** (floor-matematiksel-olarak-bunu-gerektirir; §1).

**Bağlayıcı-cümle:** "DATA wire-ordering korunur; DATA'nın token-entitlement üzerindeki MUTLAK-önceliği
FloorCap-modunda korunmaz."

**SUPERSEDE-kaydı:** Eski-sınır "Reservation yok" YERİNİ-ŞUNA-BIRAKIR: **"Bounded-parity-entitlement
1b-kapsamındadır; queue-preemption ve wire-order-değişikliği kapsam-DIŞIDIR."** Bu-scope-değişikliği
KULLANICI-ONAYI-OLMADAN production'da-UYGULANMAZ (flag-arkası-tasarım + non-default; onay-kapısı
DR-1b-v2-§scope-changes).

## 4. Floor-terminolojisi (üç-durum-ayrımı; sözleşme-DR-1b-v2-§4'te)

`requested-parity` (latch-planned) ≠ `admitted-floor-ticket` (feasibility-koşullu-kabul) ≠
`serviced/emitted-floor-parity`. "Garanti" YALNIZ admitted-ticket-için-ve-KOŞULLUDUR
(scheduler-opportunity + token-feasibility); unconditional-wall-clock-SLA-YAZILMAZ.
