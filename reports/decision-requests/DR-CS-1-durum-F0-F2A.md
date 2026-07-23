# DR-CS-1 — DURUM RAPORU: F0+F1+F2A TAMAM · F2B = DUR-B (ONAY BEKLİYOR)

**Tarih:** 2026-07-24 · **Branch:** `d1-sprint5c` · **PR:** #44 (DRAFT)
**Commit'ler:** F0=`27d7165` · F1=`4eca1ec` · F2A=`c0662f3`

## 1. TAMAMLANAN FAZLAR

### F0 (doc+test-only) ✅
DR-dokümanı repo'da (`DR-CS-1-minimal-bootstrap.md`; v1→v7-arşiv-üst-notu) · eski-4-liveness-testi
flag-OFF-pinleri-olarak-korundu · kabul-iskeleti (25-ignored→şimdi-16) · 8-TESPİT (`DR-CS-1-F0-
findings.md`) · **DUR-A kararları verildi (rapor-et-ve-devam):** M6=Seçenek-2 (harness+spec-sözleşme;
hüküm-tavanı-SENDER-SIDE) · S-A-profil-türevi (1200B→105s; (viii)-tablosu: 75s HİÇBİR-profilde-1200B-
için-geçerli-değil) + S-B-ürün-floor-zorunlu-işaretli · yeniden-giriş=yalnız-session-kurulumu.

### F1 (M1 batch-API) ✅
`on_event_batch{now_ns, inbound[]}` dört-faz-atomik (EK5-E1) + `next_wakeup_ns()` + `Event::Tick`≡
boş-batch + C2-post-condition (dönüşte-due-kalmaz; debug_assert+test) + E2-exact-no-op (yalnız-Tick)
+ EK5-v: flag-ON'da tekil-on_event = batch-of-1-delegasyonu + `CsBatchAdapter` (udp_runner; iterasyon-
başına-tek-damga; 4-event-sitesi-bağlı) + TEST-ONLY exposed-timer-hook. **Batch-atomiklik-kanıtı
(F2B/F3-kapısı) VERİLDİ:** DI-12E/F/G-F1-sürümleri + batch-of-1-kimlik + Tick-no-op + C2 — 6/6.
Driver-envanteri: udp_runner=BAĞLI; `bench/kama.rs` feature-almıyor→legacy-yol (flag-OFF); ürün-
reactor henüz-yok (client-core-inşa-edilmemiş) → adapter-bağlanmamış-production-driver YOK.

### F2A (probe-emission-feasibility) ✅ — **DUR-B-TETİKLENMEDİ (starve-yok)**
Minimal-dilim: merdiven-arm (PreConverge+srtt-None+ilk-DATA-wire-emisyonu; epoch-başına-bir-kez) →
fire-3/9/21/45s → paced-emisyon (schedule-sonu-pass; DATA-önce-mühürlü-sıra-dokunulmadı; TokenBucket-
değişmedi) → ≤44B-frame (§11.1-layout harness-plaintext-temsili). Echo/RTT/state/PTO/timeout YOK
(F2B'ye). **DI-1B:** saturated-backlog'da ilk-probe ≤3s+250ms ✓ (STARVE-ETMEDİ). **DI-6B:** floor-
31B/s'de 4-fire-tam-zamanında (93B@3s≥44B token-matematiği-doğrulandı); bütçe=176B≤200B; 5.-yok ✓.
Erken-ACK-disarm → probe-0 ✓ (temiz-hızlı-link-maliyetsiz).

Kapılar: clippy-D/fmt temiz · flag-OFF-kimlik (lib-74 + liveness-4 + desen1-5-pin) YEŞİL ·
both-features smoke-5/5 (adapter+probe canlı-UDP-yolunda).

## 2. F2B = DUR-B — ONAY-GEREKTİREN-KARAR (Abkarada)

**Tetik (F2B giriş-kapısı-(i), talimat-literal):** "0x10-0x18 çelişkisi = DUR-B" — **çelişki VAR:**
engine-spec-draft `0x12 = CTRL_PATH_PROBE` ↔ kilitli-otoritatif noise-wire-§2.56 `0x12 = RECOVERED`
(runtime-assert `pipeline.rs:502-505`); 0x10-0x18 TAM-DOLU; K-WIRE-CTRL-1: "yeni-tip = spec-
değişikliği"; §5-yasak-listesi "spec-edit" içerdiğinden bu-turda-çözülemez.

**ONAY-SORUSU:** Probe/echo çifti için §2.56-allowlist-genişletmesi onaylanıyor mu?
- **Öneri:** `0x19 = CTRL_CS_PROBE` + `0x1A = CTRL_CS_ECHO` (reserved-reject-alanından; fail-closed-
  registry-genişletme K-WIRE-CTRL-1-prosedürüyle) VEYA tek-codepoint+subtype (0x19+subtype 0x01/0x02).
- **Beraberinde:** §11.1-byte-layout'un sealed-control-yoluna-uyarlanması — ECHO_WIRE_MAX=**43B**
  (26B-EXTENDED-outer + 6B-control-inner-prefix + 11B-body; F0-tespit-(i)-türetimi); engine-spec'in
  32B-aritmetiği BAYAT (11B-eksik) → düzeltme; probe-pad-hedefi ≥43B (PROBE_WIRE_MAX=44B-tutarlı);
  replay = sealed-yol-otomatik (pn32-window + control_id-dedup).
- **Alternatif (onaylanmazsa):** DR-CS-1 harness-sınırlı-kalır (üretim-wire'a-çıkmaz) → hüküm-tavanı
  düşer: "SENDER-SIDE MECHANISM (HARNESS-WIRE) — PRODUCTION WIRE CODEPOINT UNRESOLVED".

## 3. BLOKE-KALAN (onay-sonrası sıra)
F2B (echo + bootstrap-RTT-ingest + Startup-transition + (d)-PTO-binding; kapılar (ii)-(vi) hazır-
görünüyor: mixed-version-çift-sütun-plan + replay-sealed-yol + estimator-izolasyon-planı + echo-
tanımı-43B + INV-CS-7-planı; (vii)=F2A-YEŞİL ✓) → F3 (M5-typed-timeout + M6-patience S-A-105s) →
F4 (kabul-matrisi; faithful-acceptance-6/6-önkoşul; T_W1_BOUND-kalibrasyon-dondurma) → F5 (DUR-SON
28-kalem + hüküm).

## 4. DEĞİŞMEYENLER
Feature `experimental-cold-start` DEFAULT-OFF; flag-OFF byte-for-byte (kimlik-kanıtlı). Üretim
fuzzy-config · LeftoverOnly · TokenBucket · bw.rs · SACK-V2 · ARQ-kuralları · geometrik-kopya ·
wire-format DOKUNULMADI. PR-draft; merge-YOK.
