# FS-IID-CELL-SEMANTICS-AUDIT (S3-A GATE)

> **VERDICT: DUR-B — IID-COVERAGE-AXIS AMBIGUITY. STOP.** `deficit ∈ {1,2,4}` altında hiçbir bağlayıcı-otorite
> IID20 için *deficit-anlamı* GROUND-EDEMEZ. Tek aday-anlam (R, `r=d` yoluyla) bağlayıcı v1.2 ile ÇELİŞİR,
> kodda ENFORCE-EDİLMEZ, 6-arm'ın YALNIZ 1'ini (A1-EARLY) etkiler ve benimsense bile FROZEN-540-matrisini
> yeniden-tanımlar → Abkarada-onayı + re-freeze şart. **Hiçbir IID/reverse/pacing implementasyonu bu kapı
> PASS-etmeden ilerlemez (§2). S3-B..G BAŞLATILMADI.**

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / e511ef4 (S2) · foundation d4c08c7 · S0 0ba3612 · S1 5d84f5a
FS-PREREG-v1.2 : da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034 (immutable; DEĞİŞMEDİ)
FS-1 result-bearing : 0/540 · bu audit DIAGNOSTIC (read-only; kod/spec DEĞİŞTİRİLMEDİ; budget-harcamaz)
method : 11-agent adversarial audit (7 authority-reader + 1 synthesis + 3 refuter); code ground-truth doğrudan-okundu.
```

## 1. Soru (S3-A)
Frozen FS-1 matrisi **RTT × deficit × impairment**, impairment ∈ {ExactDeficit, IID20 (NaturalIid @ sabit p=0.20)}.
**`deficit ∈ {1,2,4}` impairment=IID20 iken NE YAPAR / NE ANLAMA GELİR?** Directive adayları: A(redundancy target) ·
B(configured R) · C(realized-loss'tan bağımsız analitik stratum) · D(tarihsel label) · E(başka grounded param) ·
**DUR-B (executable-anlamı YOK)**. HARD-RULE: *yalnız d-label'ı farklı ama AYNI konfigürasyonu çalıştıran üç IID20
satırı = üç ayrı bilimsel hücre DEĞİL; 540 sayısını korumak için duplicate-hücre TUTMA.*

## 2. Matris (18 hücre → 540)
```text
RTT={50,250,1000}ms (3) × deficit={1,2,4} (3) × impairment={ExactDeficit,IID20} (2) = 18 base-cell
  (FS-COVERAGE-PLAN-v1.0 §1 L13-18; v1.1 L4-5 korur; FS-1-MANIFEST-INCONSISTENCY §1 L25-34)
540 = 18 × 6-arm{A0,A1-EARLY,F0,F1,F3,F4} × 5-seed  (v1.1 L12; v1.2 §D L137 "6 × 18-cell × 5")
IID20-dilimi = 3 RTT × 3 deficit × 1(IID) = 9 hücre × 6 × 5 = 270 run.
```

## 3. GEREKLİ S3-A TABLOSU
| impairment | deficit field | execution effect | arm effect | trace effect | R effect | expected unique cell identity |
|---|---|---|---|---|---|---|
| **ExactDeficit** d=1/2/4 | `TraceSpec::ExactDeficit.drop_initial`, \|D\|=d (fs_trace.rs:72-73) | d adet InitialSource-tx SİLİNİR (decide() :242) | tüm-arm d gerçek-kaybı kurtarır | fwd_schedule_id `drop_initial`-set'e commit (hash_forward_spec :117-125) → **d ile DEĞİŞİR** | coverage r=d ⇒ A1-EARLY r=d, **gerçek-kayba matched** | **DISTINCT** (gerçek-kayıp-büyüklüğü = d) |
| **IID20** d=1 | **YOK** — `NaturalIid{loss_permille,seed}` deficit-field-SİZ (fs_trace.rs:77) | **YOK** (channel) — decide() yalnız `keyed(id,seed)` (:253-259); keyed d-taşımaz (:190-198) | tüm-arm AYNI sabit ~%20 rastgele-kayıp; d hiçbir-şey-değiştirmez | fwd_schedule_id **YALNIZ (loss_permille=200, seed)**'e commit (:138-145) → d∈{1,2,4} arasında **BYTE-ÖZDEŞ** | tek-iz: coverage `r=d` ⇒ A1-EARLY r=1 — bu **R** (per-arm-knob), deficit-DEĞİL; A0/fixed-r-arm etkilenmez | **DISTINCT-DEĞİL** — konfigürasyon-özdeş |
| **IID20** d=2 | YOK (field-siz) | YOK (channel) | sabit %20 | schedule d=1,4'e BYTE-ÖZDEŞ | r=2 tek-varyasyon (arm-knob=R) | DISTINCT-DEĞİL — schedule ≡ d=1 |
| **IID20** d=4 | YOK (field-siz) | YOK (channel) | sabit %20 | schedule d=1,2'ye BYTE-ÖZDEŞ | r=4 tek-varyasyon (arm-knob=R) | DISTINCT-DEĞİL — schedule ≡ d=1,2 |

**Özet:** ExactDeficit'te d ve r=d GERÇEK-kayıpta ÇAKIŞIR → grounded distinct hücre. IID20'de hücreyi-TANIMLAYAN
eksen (impairment schedule) d'den bağımsız BYTE-ÖZDEŞ; d'nin tek olası izi R'ye yeniden-yorumlanır (aday-B) — ki
R zaten FS-3'te K×{0,1,2,4,8} olarak sistematik-taranır; deficit değil, impairment-hücre-özelliği hiç değil.

## 4. Kod ground-truth (doğrudan-okundu; 3 bağımsız-ajan doğruladı; ÇEKİŞMESİZ)
```text
fs_trace.rs:77      NaturalIid { loss_permille: u32, seed: u64 }        → deficit-field YOK
fs_trace.rs:138-145 hash_forward_spec(NaturalIid) → yalnız (tag2, loss_permille, seed)'e commit
fs_trace.rs:253-259 decide()(NaturalIid) = (fnv1a(keyed(id,seed))%1000) < loss_permille  → d GİRMEZ
fs_trace.rs:190-198 keyed(id,seed) = (role, source_id, attempt, seed)  → d GİRMEZ
fs_harness.rs:592-595  r = match arm { A1{r}=>r, A0=>0 }               → R arm'dan; deficit'ten DEĞİL
fs_harness.rs:602-606  run_arm forward-channel HARDCODE ExactDeficit    → NaturalIid RUN-yoluna PLUMB-YOK
fs_harness.rs:85-87    a1_duplicate_target_set girdileri = (block_id,dup_seed,configured_r,k) → D GİRMEZ (deficit-BAĞIMSIZ)
fs_harness.rs:914-920  fs_deadline_ms(source_bytes, effective_bps, rtt_ms)  → d GİRMEZ (arm-independent)
fs_harness.rs:963      f4_select_k(base_rtt)                             → d GİRMEZ (S2-pin: K deficit-bağımsız)
fs_spine.rs:310-317    S2-spine forward-channel HÂLÂ yalnız ExactDeficit+PassAll → sıfır-IID-plumbing
fs_spine_pins.rs / fs_s2_pins.rs: her pin'de r HARDCODE 2 (tüm-arm) → r=d kodda GERÇEKLEŞMEZ
```
**⇒ Channel/trace/seed/deadline/K katmanlarının HEPSİ IID20 altında d'yi YOK-SAYAR. Kod-kanıtı, çekişmesiz.**

## 5. Tek basınç-noktası: `A1-EARLY(r=d)` — ve neden deficit'i KURTARMAZ
```text
KAYNAK: FS-COVERAGE-PLAN-v1.0 §1 L19 "A1-EARLY(r=d)" (18-hücrenin hepsine, IID20 dahil, uygulanır iddiası);
        C-P4 (FS-PREREG.md L102-104) deficit×IID20'yi "matched configured-redundancy" sweep tasarlar (arms A1/F0/F1(r)).
NEDEN "deficit-anlamı"nı GROUND-ETMEZ:
  (i)  BAĞLAYICI v1.2 §A1.1 L18-19: A1-EARLY = configured_r, target "deficit-BAĞIMSIZ"; r=d ASLA denmez.
       Çelişki-sırası v1.2 > coverage-v1.0 → v1.2 KAZANIR. r=d, alt-sıralı/superseded belgede.
  (ii) Kod r=d ENFORCE-ETMEZ: fs_runner --r ve --deficit AYRI flag; pinlerde r HARDCODE 2. r=d belgesel/gerçekleşmemiş.
  (iii)Benimsense bile YALNIZ A1-EARLY d-ile-değişir (FS-1 screening satırında sadece o r=d-etiketli). A0(r=0) +
       fixed-r-arm'lar d∈{1,2,4} arasında BYTE-ÖZDEŞ → 6-arm'ın ≥5'i saf-duplicate. (270-run'ın ≥150'si duplicate.)
  (iv) r=d benimsenirse d ARTIK R'dir → alan yanlış-adlandırılmış (directive: "misleading field name kullanmayı sürdürme"),
       ve bu FS-3'ün ayrık K×R sweep'ini çoğaltır.
NOT: v1.1 §5 "IID-loss matched-deficit KALDIRILDI" — d'nin channel-loss-oranını sürdüğü TEK tarihsel-kuplaj (aday-E)
     açıkça öldürüldü. Directive de p=0.20'yi sabit (matched-DEĞİL) dondurur. ⇒ aday-E de desteksiz.
```

## 6. Adversarial-panel + adjudikasyon
```text
7-reader lean : coverage=B · prereg-v1.2(binding)=INSUFFICIENT(v1.2 impairment/deficit HİÇ-tanımlamaz; v5-§canonical'a erteler)
              · prereg-historical=B(caveat: A0-only IID sweep saf-duplicate) · manifest/code=DUR_B · fs_trace-code=B(caveat:
              r=d belgesel/enforce-yok) · harness-code=DUR_B · scripts=INSUFFICIENT(IID hiç-yok).
synthesis     : DUR_B (confidence=HIGH; iid_cells_identical=true). "B-reader'lar 'bir arm'ın r'si farklı'da durup HARD-RULE'ü
                kod-kanıtlı schedule-özdeşliğine uygulamadı."
refuter-1(code): DUR_B UPHELD — grounding-YOK. Ek-kanıt: pinlerde A1-EARLY dahil her-arm r:2 hardcode → A1-EARLY bile
                d-arasında byte-özdeş; r=d "purely documentary/unrealized". Deadline/K/seed hepsi d-invariant.
refuter-2(seed/deadline): DUR_B UPHELD — per-deficit-seed YOK, per-deficit-deadline YOK. r=d = aday-B (tek-arm-R), deficit-DEĞİL.
                EYLEM-CAVEAT: orchestrator r=d onurlandırırsa doğru-dedup Alt-2 (A1-EARLY r=1/2/4 KORU, yalnız A0+fixed-r çök).
refuter-3(authority): PROCEED (aday-B) DİSSENT — C-P4 deficit×IID20'yi matched-r tasarlar; r=d C-P1'de değil genel-kalır.
                ANCAK KENDİ KABULÜ: soyut-sınırda (özdeş-execution+analitik-label) DUR_B doğru; d-as-CHANNEL inert; d "deficit
                DEĞİL, relabeled-R"; ve PROCEED'i bile AMEND/Seçenek-3 (eksen-relabel + count-yeniden-türet) üzerinden yönlendirir.

ADJUDIKASYON (sahiplenilen-karar): DUR-B. Gerekçe:
  • d'nin deficit-anlamı bağlayıcı-otorite+kodda GROUND-EDİLEMEZ (channel/trace/seed/deadline/K çekişmesiz d-inert).
  • Tek-aday (R via r=d): v1.2 ile çelişir + kodda enforce-yok + ≤1/6-arm + benimsense alan-relabel gerekir.
  • Directive'in İKİ alt-durumu da STOP-ve-sor'a yakınsar: "d-anlamsız → DUR-B"; VEYA "d→R → alanı-relabel-et" — ki
    IMMUTABLE-540-matrisin eksenini relabel + ≥5/6-arm-çökme + count-değişimi = FROZEN-prereg-amendment → Abkarada+re-freeze.
    Hiçbir yol 540'ı tek-taraflı-değiştirip IID-plumb-etmeye izin VERMEZ. Dissenting-PROCEED bile AMEND'e yönlendirir.
  • ⇒ Kapı PASS-ETMEDİ. HARD-RULE gereği 540, üç-ayrı-DEFICIT-hücre olarak gerekçelendirilemez.
```

## 7. Benzersiz-matris alternatifleri (Abkarada-kararı için)
```text
BASELINE (yazıldığı-gibi): 18 × 6 × 5 = 540; IID20-dilimi 9-hücre(270-run), 6-hücre(180-run) HARD-RULE-duplicate.

ALT-1 — IID20 deficit-eksenini ÇÖK (strict HARD-RULE dedup; SYNTHESIS-ÖNERİSİ):
  ExactDeficit-dilimi DEĞİŞMEZ: 3 RTT × 3 deficit = 9 grounded-distinct hücre.
  IID20-dilimi: deficit-ekseni RTT-başına 3→1 çöker ⇒ 3 RTT × 1 = 3 hücre.
  Benzersiz-hücre = 9 + 3 = 12 · Run = 12 × 6 × 5 = 360 · Kaldırılan = 540−360 = 180 duplicate-run.

ALT-2 — IID20 3.-eksenini AÇIKÇA R-ekseni (r∈{1,2,4}) olarak RELABEL (yalnız p=0.20-sabitte redundancy-sweep isteniyorsa):
  Eksen adı "deficit"→"R" olmalı + her-arm'ın r=d'si otoritede teyit (şu-an yalnız A1-EARLY etiketli).
  A0(r=0) IID20'de yine 3→1 çöker; r-taşıyan-arm'lar (A1-EARLY, ve F0/F1/F3/F4 ancak amend'lenirse) 3 r-seviyesi tutar.
  FS-3'ün K×R'siyle örtüşür → FS-1-screening için ÖNERİLMEZ. (A0-only-çöküşle net ≈ 510-run.)

ALT-3 — coverage'ı AÇIKÇA amend-et: r=d'yi IID-screening için tüm-arm'lara bind + count-yeniden-türet + FS-PREREG-v1.3-re-freeze.
  Frozen-540'ı değiştirir → Abkarada-onayı ŞART (FS-1-MANIFEST-INCONSISTENCY §7 "Abkarada-kararı" ile tutarlı).

ÖNERİ: ALT-1 (12-hücre/360-run) — 180 kanıtlı-duplicate-run'ı kaldırır, 9 grounded-ExactDeficit-hücreyi korur,
540'ı yapay-şişirmez. (Bilimsel-distinctness indirimi; §8 executability-blocker'ından AYRI/ortogonal.)
```

## 8. Ortogonal-blocker (bilgi-için; bu-audit'in konusu-değil ama birlikte-çözülmeli)
```text
FS-1-MANIFEST-INCONSISTENCY.md ZATEN kaydetti: NaturalIid RUN-yoluna plumb-edilmedi → 9/18 IID-hücre HİÇBİR-arm için
execute-edilemez (270/540 run bugün executable-DEĞİL). S3-A bunu FARKLI-yoldan doğrular+keskinleştirir: IID20-deficit
alt-gridi, executability bir yana, BİLİMSEL olarak da dejenere (duplicate-hücre). İki blocker ORTOGONAL; ikisi de
Abkarada-kararı gerektirir. (S3 kalanı — reverse-pairing + 0.80C-pacing — IID-diliminden BAĞIMSIZ ve bu kapıdan
etkilenmez; ExactDeficit-dilimi + reverse + pacing S3-B..G'de ilerletilebilir — AMA directive "kapı PASS-etmeden
IID-implementasyon ilerlemez" der; reverse/pacing IID-olmayan-parçalar, Abkarada onayı ile ayrı-ele-alınabilir.)
```

## 9. Abkarada'dan istenen karar
```text
1. IID20 altında deficit-ekseni: ALT-1 (çök, 360-run) · ALT-2 (R-relabel) · ALT-3 (amend+bind r=d, re-freeze) — hangisi?
2. Seçilen yol frozen-540'ı ve immutable FS-PREREG-v1.2'yi değiştirir → v1.3-amend + re-freeze onayı.
3. Executability-blocker (IID plumb-yok) ile birlikte mi çözülsün?
4. Bu kapı çözülene kadar S3'ün IID-olmayan parçaları (reverse-pairing, 0.80C-pacing, ExactDeficit-dilimi) ilerlesin mi,
   yoksa tüm-S3 mi beklesin?
```

## 10. STOP
```text
DUR-B (IID-COVERAGE-AXIS AMBIGUITY). Directive §2/§28 gereği: hiçbir IID implementasyonu ilerlemedi; S3-B..G BAŞLATILMADI;
NaturalIid-plumbing/reverse/0.80C-pacing/schema/fixtures/pin YAZILMADI; frozen-matris/spec/kod DEĞİŞTİRİLMEDİ;
FS-1 result-bearing 0/540; v1.2 immutable. Human-review (Abkarada) bekleniyor.
```
