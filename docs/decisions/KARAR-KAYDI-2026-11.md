# KARAR-KAYDI 2026-11 — yetki, dal, kapı, merge ve ajan kuralları

**Karar sahibi:** Abkarada · **Kayıt tarihi:** 2026-11-09 · **Bu belge:** sahibin tarihli kararlarından yalnız depodaki kuralların dayandığı maddelerin metnini taşır. Böylece klonu okuyan her ajan, CLAUDE.md'nin atıf yaptığı istisnaları buradan okuyabilir.

**Kaynak ve sürüm.**
- EK-1 = YOL PLANI v2 (revizyon 2) ek karar sayfası. Taslak 2026-11-03, D-01…D-35.
- İmza 2026-11-04: "EK-1 K4G kümesi EVET · EK-1 S2-A kümesi EVET · kalan satırlar EVET" (değiştirilen satırlar aşağıda ayrıca yazılı).
- Değişiklik 2026-11-09: dizgiler, gösterim penceresi WIP kuralı, yerel kapı şartı, corrigendum onayı.
- Onay lafzidir. İmzasız satır yürürlüğe girmez; "itiraz gelmedi" onay sayılmaz.
- EK-1'in burada olmayan maddeleri depo dışındadır. Bir iş emri onlara dayanıyorsa ilgili metni kendi içinde alıntılar.
- Metinler EK-1'den birebirdir. Yalnız depo dışındaki çalışma dosyalarına ve plan bölümlerine verilen atıflar (parantez içi kaynak etiketleri, "yerine geçtiği" satırlarındaki depo dışı kalemler) çıkarıldı; kuralın kendisine dokunulmadı.

---

## §1 D-01 — Yetki sırası ve CLAUDE.md yaması

- **Soru.** EK-1, donmuş spec'ler ve CLAUDE.md çeliştiğinde hangisi kazanır?
- **Karar (A).** EK-1'in imzalı satırları, listelediği donmuş metinlerin üstünde; CLAUDE.md §3, §6, §7 sahipçe yamanır; §4 yalnız D-02 (b) seçilirse. Metinler:
  - §3: "Spec ↔ spec çelişkisinde ve donmuş metin değişikliğinde yetki yalnız sahibin tarihli kararıdır (YOL PLANI v2 EK-1, D-numaralı). Listelenen yamalar aynı PR'da üstü çizili + YAMA KAYDI ile. Listede olmayan donmuş metin değişmez."
  - §6: "EK-1 D-23'te listelenen kartlar için ≤ 7 iş günü istisnası (saatlik commit; main'in dala günlük merge'ü; rebase/force-push yok). EK-1'de listelenen imza ve bayt düzeni değişiklikleri talimat sayılır."
  - §7: "Hosted CI fatura ya da altyapı yüzünden başlamazsa EK-1 D-17'deki yerel kapı geçerlidir."
- **Yerine geçtiği.** `CLAUDE.md:3`, `:32`, `:78`, `:85`, `:88-94`.
- **Durum.** D-02 (b) seçilmedi; §4 değişmez.

## §2 D-17 — CI faturası, yerel tam kapı, RUSTSEC

- **Karar.**
  - Hosted CI fatura/altyapı yüzünden başlamazsa yerel kapı merge'e yeter: ci.yml'in PR adımlarını sırayla, tek süreçle koşan betik depoya `scripts/ci/local-gate.sh` olarak alınır; toolchain 1.96.0, `--locked`, bayraklar dahil; çıktı PR'da; "CI-KİLİT İSTİSNASI ile merge" satırı. `scripts/gate.sh` bu kapı değildir (çok yapılandırmalı taşıma kapanış kapısı).
  - Hosted'a kalanlar adıyla: miri, ASan, MSan, audit, aead-differential. Kilit kalkınca zorunlu; kırmızıda ACİL DUR + revert. Merge'den önce kilidin sürdüğü `gh pr checks` / `gh run list` ile doğrulanır.
  - Kilit sürerken günde ≤ 3 birleşme, kritik yol önce; birleşme başına ≈ 1 sa cargo.
  - Makine kuralına bu plan için istisna: `-j 2`, tek süreç, yalnız kilit sürerken.
  - RUSTSEC/deny: aynı gün ayrı `chore(deps)`; düzeltilmiş sürüm yoksa tarihli gerekçeli ignore + sahip satırı.
- **2026-11-09 şartı.** Yerel kapı istisnası yalnız `scripts/ci/local-gate.sh`'in düzeltilmiş hâli main'deyken kullanılır. Düzeltmenin içeriği: CI tanımı kaymışsa, çalışma ağacı kirliyse ya da kilit alınamazsa kapı başarısız olur. Koşu sonunda HEAD ve çalışma ağacı yeniden doğrulanır, değiştiyse sonuç GEÇERSİZ olur. Bu hata yolları betik testleriyle gösterilir. O hâl main'e girene dek merge'ler hosted CI'a bağlıdır.

## §3 D-23 — Dal süresi, bayrak politikası, `sender.rs` bölgeleri

- **Karar.**
  - ≤ 7 iş günü istisnası, iki günü aşan bütün kartlar için: **E2a-K, E2a, E1, E2b, E3-1, E3-2, T1-1, T1-2a, T1-2b, T1-4, T1-5, T1-0, Ö-c, Ö-d, SAAT-G/Ç, A3-1, A3-2, H-3…H-6, E0a, E0b.** Yedi günü aşan işler PR'lara bölünür (A3 → A3-1/A3-2; E3 → E3-1/E3-2; E2a → E2a-K/E2a). Saatlik commit; main her gün dala merge; rebase/force-push yok.
  - `sender.rs`'te en çok iki eşzamanlı bölgeli dal (istisnalar E0a/E0b sayaç satırları ve iki `Event` varyantı).
  - Her kural bayraklı; kapı geçince varsayılan açık; Ş-T1 kartları `class_lanes` kapalı bit-eşitliğiyle; T1-1 davranış-nötr kanıtla; bayrak DEMO+1'e kadar; EVOLUTION §22 beyanı; kimlik D-11.
- **Yerine geçtiği.** `CLAUDE.md:78`.
- **2026-11-09 tarihli istisna (gösterim penceresi, 9–17 Kasım).** Modül başına tek açık dal (WIP = 1) geçerlidir. Bu pencerede D-23'ün "en çok iki eşzamanlı bölgeli dal" kuralı WIP = 1'le birlikte okunur. Pencere kapanınca D-23, `sender.rs`'in modüllere mekanik ayrımıyla birlikte yeniden yazılır.

## §4 D-32 — Onay dizgileri ve merge yetkisi

- **Başlatma.** Sahip şerit dizgisini yazar; orkestratör iş emrini o dizgiyle başlatır. Dizgi motor koduna dokunma talimatıdır.
  - 2026-11-09'dan itibaren dizgiler: **"YOL3-HA BAŞLA", "YOL3-HB1 BAŞLA", "YOL3-HB2 BAŞLA", "YOL3-M1 BAŞLA", "YOL3-M2 BAŞLA", "YOL3-T BAŞLA", "YOL3-Ö BAŞLA", "YOL3-CC1 BAŞLA"**. (EK-1'deki "YOL2-<ŞERİT> BAŞLA" biçiminin yerine.)
  - Gösterim dondurması: **"GÖSTERİM MOTORU DONDU <SHA>"**. Yol seçimi: **"YOL: A"** / **"YOL: L"** / **"YOL: B+"**. Hepsi sahibin ayrı mesajıdır.
- **Merge.**
  - (i) Kapı geçti (commit'li `gate_eval.py` hükmü) + CI yeşil ya da D-17 + bayrak kuralı → orkestratör squash-merge eder, PR adı onaylanan adla.
  - (ii) Bit-eşitliği ya da (b) sınıfı nötrlük kanıtlı davranış-nötr PR → orkestratör.
  - (iii) Tel biçimi, ürün profili kimliği, donmuş spec metni (D-01 listesi) ya da CLAUDE.md değiştiren PR → sahip **"MERGE <PR#>"** yazar. 2026-11-09 eki: S1 `send()` imzası (D-09), H-0 belge aktarımı, corrigendum ve yerel kapı PR'ları da bu sınıftadır.
- **Kapı sözleri** yalnız sahibin ayrı mesajı: "G-S4 GEÇTİ", "INT-02 GEÇTİ", "INT-03 GEÇTİ", "DEMO-01 GEÇTİ", "K4G-05g ONAYLANDI"; belgeye gerçekleşmiş olarak yazılmaz.

## §5 D-34 — Ajan cargo kullanımı, ajan sayısı, kapasite

- **Karar.**
  - Bu plan için "ajanlar cargo koşturmaz" kuralı kalkar: ajanlar ortak flock'lu cargo sarmalayıcısıyla, `CARGO_BUILD_JOBS=2`, debug ve dar test seçimiyle derler; ajan başına günlük cargo bütçesi 45 dk; release ölçüm ikilileri yalnız orkestratörde; netem ve ölçüm koşturma yasağı aynen.
  - Ajanlar: K4G 5 cargo (ajan-Ö, -M1, -M2, -T1, -T2) + 1 yalnız Python (ajan-A); S2-A 2 (ajan-HA, -HB). Toplam 7 cargo + 1 Python.
  - Kapasite: hedef %80 doluluk; aşım "kart aşımı" satırıyla fiyatlı.
- **2026-11-04 / 2026-11-09 ekleri.** Bulut oturumları (kendi konteyneri, kendi klonu) D-34 dışıdır; yerel kadro D-34'e tabidir. Yerel sarmalayıcı `scripts/ci/cargo-lock.sh`'tir; §2'deki yerel kapıyla aynı kilidi kullanır.

## §6 Onaylanan corrigendum

- 2026-11-09: "Corrigendum (`reports/ktp2/f0/ACIL-INCELEME-2026-11-03.md:714-717`) EVET — ayrı PR, MERGE sahipten."

## §7 `docs/plans/SPRINT-2-BACKLOG.md` süreç satırları

Backlog H-0 ile bayt-özdeş aktarıldı. İçeriği (kartların ne'si, S4/TDD atıfları, sayılar) geçerli. Süreç satırlarının aşılmış hâli belgenin başındaki tarihli SUPERSEDED bandında yazılı. Bandın dayandığı kararlar:
- S2-A şeridi, "PLAN-SPRINT2 ONAYLANDI" mesajını beklemeden açıldı (sahip imzası 2026-11-04, EK-1 D-13).
- A3, "INT-02 GEÇTİ"yi beklemez; A3 dumanı gerçek hub ikilisiyle koşar (EK-1 D-14, 2026-11-04 ekiyle). "INT-02 GEÇTİ" gösterimin şartı değildir.
- Backlog'daki "Gün N" takvimi geçersizdir. Geçerli takvim iş emirlerindedir.

## §8 D-CC-1 — CC-PORT (2026-11-11, sahip, gece yetkisi)

- **Karar.** Taşıma motorunun hız/tıkanıklık denetimi quinn-proto 0.11.18 BBR + RFC 9002 mekanizmalarıyla değiştirildi
  (`crates/adaptive-transport/src/cc/`, Apache-2.0/MIT atıflı kod taşıma; ADR-06/ADR-14 "fork yok" korunur). PR #170
  (motor, çalışma zamanı seçimli), #175 (ürün runtime'ı, `RuntimeConfig::engine`), #176 (ürün varsayılanı
  port + ilk uçuş 4·MTU; geri alma `KAMA_TRANSPORT_ENGINE=legacy`). **CC-PORT VARSAYILAN 399b573f** — sahip, 11 Kas
  gece; Ö-CC CP-1 ve sim kapısı dayanak; geri alma anahtarı var. Spec yamaları: CLAUDE.md §4/§8, engine-spec §0.3/§5/§13,
  transport-layer-tdd §5.4 ve invaryant tablosu (eski metin üstü çizili, silinmedi).
- **Dayanak.** `docs/design/CC-PORT-ESLEME.md` (eşleme, muhasebe, uyarlamalar U1–U8); Ö-CC CP-1 netem (port 15,7 s /
  10-10, eski 23,3 s / 10-10, eşli medyan 0,67); sim kapısı `tests/cc_port_sim.rs`.
- **Deney (ürün kuralı değil).** Kayıp filtresi `cc-port-lf` (PR #177, taslak): yalnız r20/r30 hücresinde port K0'dan
  geride kalırsa koşulur. Düşük bant kipi (0,025 kbps): ayrı deney, Aralık.
- **Açık risk.** Startup taşması sığ kuyrukta (BBR v1 doğası; U9 paket koruması, PR #179, azaltır); alıcıda sırasız varışta paket başına ACK (dönüş yolu
  maliyeti; düşük bant kipinde sınırlanacak).
- **Gösterim ağacı** (`yol2/goster`) etkilenmez; gösterim motoru seçimi CP-6 sonrası sahip kararı.

## §9 D-MERGE-1 — Kontrollü birleştirme (2026-11-12, iş emri rev. 02:10)

- **Yetki.** Sahip, 2026-11-12 02:00: `MERGE 191 194 187 182 192 183 196 193 189`. Her PR iş emrinin §2 şartıyla ve güncel main üzerinde birleşti. #181/#186 birlikte yetkisi yok (§3 kanıtı önce). #190 için ayrı `MERGE 190` gerekir.
- **Yöntem.** Her aday için aynı adımlar:
  1. `update-branch` ile main dala sunucu tarafında merge edilir (rebase yok).
  2. PR kontrolleri ADLARIYLA doğrulanır (#182 sonrası 13 ad, `cc regression gate` dahil).
  3. `gh pr merge --squash --match-head-commit <SHA>` ile birleştirilir.
  4. core-main o SHA'da yeşil olur. core-main `cancel-in-progress` olduğu için kapı bitmeden sonraki merge yapılmaz.
  5. Etkin farkın patch-id'si özgün farkla karşılaştırılır.

| sıra | PR | squash | birleşen baş | kontroller | core-main | ek şart |
|---|---|---|---|---|---|---|
| 1 | #191 | cc7f912e | 3c587232 | 12/12 | success | — |
| 2 | #194 | 7a40b18a | 8b91ee7a | 12/12 | success | — |
| 3 | #187 | c9108b70 | 9320d2ab | 12/12 | success | — |
| 4 | #182 | 2d290af5 | e64cd34b | 13/13 | success | muafiyet satırı 0 |
| 5 | #192 | a4b4cb30 | d6b166ec | 13/13 | success | yerel `s4_hub_restart --ignored` 10/10, aynı ağaçtan hub-server |
| 6 | #183 | 9d57995e | 86da6f64 | 13/13 | success | yerel `ack_after_admit` 3/3 |
| 7 | #196 | d1c2d5b4 | 7f757caf | 13/13 | success | çakışma çözümü (aşağıda) |
| 8 | #189 | 59311899 | 3b38fe8b | 13/13 | success | çakışma çözümü; 8 sim hücresi `CC_GATE_OUT` bayt aynı |
| 9 | #193 | f1342814 | d83f6264 | 13/13 | success | önceki aday kırmızısı yerelde yeniden üretilemedi (aşağıda) |

- **Çakışma çözümleri.** İkisinde de metin birleşimi temizdi, sonuç bozuktu.
  - #196 ↔ #183: `fec_ingest`'te `recovered_esis.push` iki kez kaldı. Koşulsuz olanı F4'ü bozuyordu.
  - #189 ↔ #182: `cc_port_sim.rs`'te `HashMap` iki kez içe aktarılmıştı (E0252).
  - Her biri tek satır silindi; ayrıntı PR yorumlarında.
- **Bitiş kapıları.** `cargo test --workspace --all-features` her squash SHA'sında core-main `full workspace` işinde koştu. `cc_gate_compare.py --selftest` main iş akışında yok ve ci.yml'de `workflow_dispatch` yok; bu yüzden f1342814'te yerelde koşuldu ve yeşil çıktı.
- **Bekleyenler.**
  - #181, #186: §3 entegrasyon kanıtı #175'te; karar sahipte.
  - #195, #185: #181'i bekliyor.
  - #190: `MERGE 190`; uzun ailede B'nin parite yolu gösterilmedi.
  - #188: profil kararı bekliyor.
  - #177: taslak.
  - ~~#140: `KAPAT 140` bekliyor.~~ Merge edilmeden kapatıldı, 2026-11-13 (§10).
- **Açık.**
  - `cc regression gate` zorunlu kontrol yapılmalı (depo ayarı, sahip).
  - `ack_after_admit::legacy_stalled_consumer_loses_no_acked_unit` CI'da bir kez 60 s zaman aşımına uğradı. Yerelde main ağacında ve #193 adayında 10'ar koşunun hepsi geçti (AÇIK-3).

## §10 2026-11-13 — #140 kapanışı ve birleştirmeler

- **Yetki.**
  - Sahip, 2026-11-13: `KAPAT 140` ("merge etmeden kapat; `k4g/05b-retx-fix` dalı ve envanter kaydı korunsun"), `MERGE 200` ve `MERGE 202`. Sözler iş kuyruğu (#175) üzerinden geldi; yerel oturumda sahipten doğrudan teyit edildi.
  - #197'nin yetkisi sahibin 2026-11-12 tarihli `197` GO sözü. Şartı #200'ün önce girmesiydi.
- **#140.**
  - Merge edilmeden kapatıldı.
  - `k4g/05b-retx-fix` dalı (`49864510`) silinmedi, çünkü main'de 28 dosya bu başa atıf yapıyor.
  - Envanter (#140 yorum 6832010314): taşınan 11 kalem, dışarıda kalan 18 kalem.
- **Birleştirmeler.** §9'daki yöntemle yapıldı. Her birinde etkin farkın patch-id'si özgün farkla aynı, main ağacı da birleşen başın ağacıyla aynı.

| sıra | PR | squash | birleşen baş | kontroller | core-main |
|---|---|---|---|---|---|
| 1 | #200 | 5b487ec8 | 28f0cbb6 | 13/13 | success (43334844042) |
| 2 | #197 | c77bdcbe | c1ee1d69 | 13/13 | success (43339956173) |
| 3 | #202 | 238a1d28 | bcdf46c0 | 13/13 | success (43343543089) |

- **#202'nin yerel doğrulaması** (#202 yorumları):
  - İlk test başında (`4d1e6fb4`) taban 35 koşunun 4'ünde yeşil verdi; test tabanı belirlenimli ayırmıyordu.
  - Dalın yazarı testi değiştirdi (`e02ac36b`, 6 s karartma). Taban 10/10 kırmızı çıktı. Aday 20/20 yeşil; tanı kopyasında 20/20 koşu `param_sends` = 6.
- **Kapsam dışı.** §9 "Bekleyenler"deki #181, #186, #195, #185 ve #190 satırları 2026-11-12'den sonraki birleştirme ve ölçümlerle bayatladı. Bu bölüm onları güncellemez.
