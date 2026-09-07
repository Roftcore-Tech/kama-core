# EVIDENCE-MASS — BEKLENTİ KAYDI (koşudan ÖNCE) + kanıt-kütlesi tanımı

Taban: `f5b73da8` (persistence kalibrasyonu). Soru (owner): "kanıt kütlesi küçükken risk tahmini ne kadar
FEC yetkisi taşımalı?" — risk BÜYÜKLÜĞÜ ile kanıt KÜTLESİ ayrı kavramlar; ilk risk sinyali GECİKTİRİLMEZ.

## 0. Kanıt-kütlesi tanımı — paydayla ilişkisi (koddan, değiştirmeden ÖNCE)

`fec_loss_risk_split` (loss.rs) paydası **`sent_window_units`**: rolling `W_loss` penceresinde (zaman
kesimi + `LEDGER_MAX`) GÖNDERİLMİŞ birimlerin toplamı; kanonik `loss_windowed_ppm` ile AYNI payda.
Instant risk = Σ(ağırlık × olay birimi) / sent_window_units.

**Ayrım (belgelenir, değiştirilmez):** `sent_window_units` fate'i HENÜZ BİLİNMEYEN (in-flight, < ~1 RTT
önce gönderilmiş) birimleri de içerir — yani "pencereye ne kadar bilgi girdi"nin üst sınırıdır, kendisi
değil. Olay üretmiş birimler `declared_units`'tir (RiskSplit'te zaten var: Suspected+Confirmed+Spurious
birimleri). ACK'lenmiş-birim sayısı pencerede tutulmuyor; icat edilmedi. **Bu turda kütle = ham
`sent_window_units`** (owner tercihi); `declared_units` ikinci görünüm olarak birlikte enstrümante edilir.

**Küçük-payda mekaniği (koddan):** `l2_min_persistent_ppm = 10 000` ⇒ `sent_window_units ≤ 100` iken
**TEK** yaşlanmış Suspected birim (≥ persist_min) persistent'ı ≥ 10k yapar → L2 UYGUN. 96-mesajlık
akışta pencere hiç 100'ü aşmaz → kısa akışta L2 "bir tek gap ≥ persist_min yaşadı" ile açılır.
Bu, "195k/290k tepe" meselesinden AYRI bir küçük-payda etkisidir ve L2'ye doğrudan bağlıdır.

## 1. Enstrümantasyon (politikadan ÖNCE)
`RiskSplit.sent_window_units` (ham) · `TsSample.{evidence_mass, declared_units}` (50 ms seri, t_ms = akış yaşı)
· `diag_spec_transitions()` = ilk L1 / ilk L2 anında `(em, instant, persistent)` · mevcut: risk_t
(t_susp, t_conf, t_risk, t_parity), spec (…, t_L1, t_L2), lvl, persist, tel/parity/retx, out.jsonl (2×RTT).

## 2. Faz-1 (M0, salt enstrüman; kollar off · A-M0 · B-M0; P2; 96 msg; n=5) — öngörüler
1. **İnvaryant:** her ON koşuda `t_first_risk == t_first_suspected` (0 ms fark) — kütle okunmuyor, değişemez.
2. **em@L1 (E1):** ilk kuşku ≈ RTT + K=3 paket ≈ 250–400 ms → o anda gönderilmiş ≈ 12–20 birim
   (20 ms/mesaj) → em@L1 ≈ 10–25. **em@L2 (E1, ≥%10):** + persist_min (P2 taban 0.5×srtt ≈ 100 ms) →
   em@L2 ≈ 15–35. Öngörü: em@L2 < 40 hemen her kısa akışta — "yeterli kanıt" tabanı 32 bile L2'yi
   ~100–300 ms geciktirir; 64 ise akışın yarısına kadar (≈1.3 s) L2'yi kapatır.
3. **Tepeler:** instant risk tepesi 100–300k ppm em 8–20'de (1–3 kuşku / ~10 birim), plato %20'de ≈ 140–200k.
   Tepe anında seviye L1 (persistent henüz 0) → tavan 250‰ (r2) — tepe L2 AÇMAZ; L2 yalnız yaşlanmayla.
   **Öngörü D:** tepe kendisi patolojik parity üretmez (L1 tavanı zaten 250‰ ile sınırlı).
4. **Küçük-payda L2 (yeni):** E1-L2/L5'te 1–5 gerçek kayıp → ilk yaşlanmış gap'te L2 açılır (§0 mekaniği)
   → B (r3) A'dan daha çok parity öder; d2 kazancı düşük kayıpta OFF'a yakın (rsweep: ≤%10'da L2 domine
   edilmişti). Kütle tabanı bunu ancak E_min > em@L2 (≈30+) ise keser.
5. **E2-C1b (reorder, P2):** yanlış L2 A ≈ 0, B ≈ 26 (persist n=3) — kalıntı B L2'lerinin em'i KÜÇÜK
   (halka dolmadan, ilk ~200 ms; em ≲ 15) → kütle tabanı ≥ 16 bunları keser (M1'in tek gerçek kazanç adayı).
6. **E2-C3p (P2):** kalıntı 10–15 yanlış L2 hem erken (em ≲ 8, RTT 45 → 50 birim/s) hem geç (Q95 kuyruğu)
   → taban yalnız erken kısmı keser (≤ %50).
7. **E3-C4:** gerçek kayıp + reorder; P2 ufku ~150 ms; em@L2 ≈ 20–40; d2 A ≈ 0.55–0.70, B +5…+10 pp.
8. **Stall:** kayıplı hücrelerde %13–16, politikadan bağımsız. **Temiz yol (E1-L0):** risk 0, parity 0, L1=L2=0.

## 3. Faz-2 eşik-seçim kuralı (ÖNCEDEN kayıt; faz-1 verisinden seçilir, sonuçtan DEĞİL)
E_min adayları faz-1'in em@L2 dağılımından: (a) `E_lo` = E2'deki yanlış-L2'lerin em-medyanı üstü (erken
yanlış-L2'lerin ≥ %50'sini keser), (b) `E_hi` = E1 (≥%15) ilk-L2 em'lerinin Q90'ı (gerçek L2'lerin ≥ %90'ını
koruyan en büyük değer). E_lo ≤ E_hi ise ikisi de koşulur (M1: floor; M2: full = 2×E_min); E_lo > E_hi ise
çatışma var → yalnız E_hi koşulur ve bu "kütle yanlış-L2'yi gerçek-L2'den ayırt etmiyor" bulgusudur
(Case B/C'ye gider). Kabul: E1 ≥ %15'te d2 (stall-dışı) ≥ M0 − 0.03; E2'de yanlış L2 ↓ ≥ %50 ya da parity ↓;
t_L1 ve t_risk her modda M0'a özdeş.

## 4. Sapma dekompozisyonu
(1) tutmazsa → kütle L0/L1 yoluna sızmış (bug). (2) em@L2 ≥ 50 ise → pencere tahminimden büyük ya da
mesaj≠birim (fragmenter) — enstrüman okuması düzeltilir, politika yorumu değil. (3) tepe anında L2 görülürse
→ persistent'ın yaş eşiği srtt=0 başlangıcında tetikleniyor (öngörü hatası + olası kusur). (5) tutmazsa →
kalıntı yanlış-L2'ler geç (Q95 kuyruğu) → kütle tabanı işe yaramaz (Case B).
