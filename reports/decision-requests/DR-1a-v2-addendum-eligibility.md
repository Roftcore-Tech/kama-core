# DR-1a-v2 ADDENDUM — DECISION-ELIGIBILITY/STALE SEMANTİĞİ (§7; Commit-E) [ONAY-BEKLİYOR; NO-IMPL]

> v2'deki-çelişki-giderimi: "stale→literal-0" ∧ "eşik-deferred" birlikte-uygulanamazdı — stale-sayılma-
> ölçütü-yoksa fallback-uygulanamaz. Bu-addendum ölçütü KOD-OLAY-AKIŞI-ENVANTERİNE-dayalı-yapısal-kurala bağlar.

## 1. Kod-envanteri (eligibility-tabanının-gerçeği)

- `fuzzy_evaluate` **HER `on_event`-işlemesinde koşulsuz-koşar** (sender.rs on_event: match-sonrası
  `fuzzy_evaluate`→`update_state`→rate→schedule zinciri) — **TEK-İSTİSNA:** `Event::RecoveredControl`
  SAF-LOGICAL-erken-dönüş (DR-S5-4; fuzzy-koşmaz, emit-yok, **blok-da-kapatamaz**).
- Commit-D: `fusion_event_index` her-fuzzy_evaluate'te ++; `last_fusion_snapshot` canonical.
- Blok-kapanışı/parity-enqueue (mevcut-mekanik + gelecek-1a-latch) **bir-on_event-işleme-BAĞLAMININ-
  İÇİNDE tetiklenir** (DATA-emisyonu-decision'ları o-event'te üretilir; enqueue aynı-bağlamda).
- Duvar-saati fuzzy-katmanında-yok (ADR-17; event-index-var).

## 2. Dört-seçenek-karşılaştırması (§7.1)

| Seçenek | Determinizm | Uzun-RTT | ACK-sessizliği | Uzun-blok-doluşu | Partial-timeout | Idle-geçiş | Test-edilebilirlik | Yeni-kalibrasyon-politikası? |
|---|---|---|---|---|---|---|---|---|
| Wall-clock-age | zayıf (saat-enjeksiyonu-gerek) | eşik-RTT'ye-göre-keyfî | eşik-keyfî | keyfî | keyfî | keyfî | orta | **EVET (eşik=[CAL])** — istenmiyor |
| Event-count-eşiği | tam | N-keyfî | N-keyfî | N-keyfî | N-keyfî | N-keyfî | iyi | **EVET (N=politika)** |
| **Current-EVENT-EPOCH [ÖNERİLEN]** | **tam** (index-eşitliği) | etkilenmez (event-bazlı) | etkilenmez | etkilenmez (latch-anı-event'inde-taze-decision-ZATEN-üretilmiş) | aynı-kural (flush=event-bağlamı) | temiz (dış-bağlam→0) | **çok-iyi** (index-karşılaştırma) | **HAYIR (yapısal; eşik-yok)** |
| Control-window-eligibility | orta (pencere-tanımı-ek-durum) | pencere-RTT-bağı | karmaşık | karmaşık | karmaşık | karmaşık | orta | kısmen (pencere=politika) |

## 3. ÖNERİLEN-YAPISAL-KURAL (en-dar + deterministik; kod-envanterinden)

**Eligibility-epoch = aynı-on_event-işleme-bağlamı.** Latch, YALNIZ latch'in-tetiklendiği-on_event-bağlamında-
üretilmiş-decision'ı kullanabilir:

```text
eligible(decision, latch) ⇔ decision.event_index == current_fusion_event_index
                             (⇔ decision_age_events == 0)
aksi-halde                 → ratio = LITERAL-0  (+ stale_zero_count-telemetri)
```

**Gerekçe:** fuzzy_evaluate her-on_event'te-koşulsuz-koştuğu VE blok-kapanışı-yalnız-on_event-bağlamında-
tetiklendiği-için, normal-akışta latch-anındaki-decision DAİMA-age-0'dır → kural pratikte-hiç-kısıtlamaz;
yalnız ANORMAL-yolları (dış-bağlam-latch-çağrısı, RecoveredControl-sonrası-varsayımsal-latch, gelecekte-
eklenebilecek-event-dışı-tetikleyiciler) güvenli-sıfıra-düşürür. **Keyfî-eşik-YOK; kalibrasyon-politikası-
GEREKTİRMEZ; test:** (i) normal-yol age==0→latch-decision (ii) yapay-dış-bağlam→0 (iii) RecoveredControl-
index-ilerletmez-kanıtı. Not: "son-eligible-network-observation" ve "control-window-reset" tanımları bu-
kuralın-İÇİNDE (her-on_event=yeni-gözlem+yeni-decision); block-open-sonrası-seçeneği REDDEDİLDİ (uzun-blokta-
bayat — §7.1-tablosu).

## 4. Revize `ControllerDecisionSnapshot` (§7.3)

```rust
struct ControllerDecisionSnapshot {
    ratio_permille: u32,
    event_index: u64,        // = fusion_event_index (Commit-D-sayacı; eligibility-testi bunun-üzerinden)
    no_rule: bool,
    emergency: bool,
}
// observation_epoch AYRI-ALANA-GEREK-YOK: event_index == observation-epoch (her-on_event=yeni-gözlem;
// kod-envanteri-§1). Alan-azaltımı-gerekçeli (§7.3 "gerçek-ihtiyaca-göre-azaltılabilir").
```

## 5. Sabit-kalanlar (§7.4-7.6 — v2'yle-uyum)
no_rule → **LITERAL-0** (config-bağımsız) · initial (hiç-eligible-decision-yok) → **LITERAL-0** ·
emergency → **"Temporary safety interlock pending 1b/1d arbitration"** (istenen-r-telemetride; emitted=0;
kalibrasyon-dışı-hücre).

## 6. Gate-2-durumu
Bu-addendum-ONAYLANIRSA DR-1a-gate-2 (eligibility-uygulanabilir+test-edilebilir) KAPANIR
(gate-1 variable-r: [preflight-raporu](../d1-exit/variable-r-preflight.md) PASS).
