# Z5 — POLICY-SELECTION REVIEW [docs-only; production-default-DEĞİŞMEDİ]

> Zincir: Z1(44ba07f) → Z2(1f8576c) → Z3(d692a1a) → Z4(2929a60). PR#44-DRAFT.

**Karar-soruları (talimat-§1) — cevaplar:**
1. Gerçek-kurtarım? EVET — P15/F15 Leftover'a-karşı decode/dl_op/avoided-retx'te-belirgin (rev99:
   decode 8→127→225; dl_op 0.870→0.922→0.938).
2. F2-fairness-faydası? EVET — tüm-deficit-sınıflarında-F15-decode≥P15; paired only_F15-13-vs-only_P15-5.
3. ARQ-displacement-toplamı-bozuyor-mu? HAYIR-genel (rdisp-raporlu-bounded; completion-korunur) —
   İSTİSNA: blackout-30s-10-blok-harmed (şerhli; izleme-maddesi).
4. Fairness-mi-concentration-mı? FAIRNESS (H1) — H2-hiçbir-sınıfta-desteklenmedi.
5. Pareto? F15-cephede; P15-pratikte-domine; L-düşük-maliyet-köşesi; F10/F20-[CAL]-uçları.
6. Netem-yön-uyumu? BLOKE (root-yok) — hazırlık+iskelet-Z4; production-default-netem'siz-SEÇİLMEZ.

**Mekanizma≠sonuç-ayrımı-korundu:** seçim-kanıtı=outcome-metrikleri (dl_op/decode/avoided-retx/worst-
decile), mekanizma-metrikleri-değil. Controller-arm-tablolara-alınmadı (kalibrasyonsuz).

**ÖNERİ: DR-1b POLICY SELECTION READY — F15 RECOMMENDED** (deterministik-kanıt-S1-shortlist;
production-default-değişikliği AYRI-kullanıcı-onayı + netem-koşusu-ŞARTINA-bağlı; blackout-30s-harmed-
izleme-maddesi + [CAL]-share-kalibrasyonu-notuyla).
