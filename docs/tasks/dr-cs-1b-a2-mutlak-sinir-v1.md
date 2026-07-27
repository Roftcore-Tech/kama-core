# DR-CS-1b / A2 — MUTLAK-SINIR v1: İZİN VERİLEN DEĞİŞİKLİK YÜZEYİ [K-6 ADIM-7; ONAY BEKLİYOR]

**Statü:** Abkarada adım-6-mühür kararıyla yetkilendirildi ("Adım-7 ve adım-8 belgelerini hazırla;
production koduna henüz dokunma"). Bu belge, A2-icra-talimatı yürürlüğe girdiğinde üretim-kodunda
**değişmesine izin verilen TAM yüzeyi** tanımlar — listede olmayan her dosya/yol A1-dönemindeki gibi
**DOKUNULMAZ** kalır. Bu belge onaylanmadan (DUR-B-2) hiçbir üretim-dokunuşu başlamaz.

> A1'in "PRODUCTION CODE = SIFIR" mutlak-sınırı A2'de bu belgeyle DEĞİŞTİRİLİR; değişiklik yalnız
> aşağıdaki yüzeyde, `experimental-cold-start` flag-arkasında, faz-kapılarıyla yapılır.

---

## 1. GENEL KURALLAR (yüzeyden bağımsız; her commit'te geçerli)

```text
G-1  Tüm davranış-değiştiren üretim-dokunuşları cfg(feature="experimental-cold-start") arkasında;
     feature kama-engine + kama-noise-wire + kama-integration Cargo.toml'larına EKLENİR (A2-F0).
     Flag-OFF: byte-özdeş davranış — her crate için flag-OFF-özdeşlik pini İLK ilgili commit'te.
     SALT-ADDITIVE API'ler (yeni pub fn; flag-OFF'ta çağrılmaz) ayrık-sınıf olarak işaretlenir.
G-2  [CAL]-SEALED değerler (freeze-r2 #0..#9, #11-13) kodda SABİT olarak bu değerlerle girer;
     tuning YASAK. CONTROL_DEDUP_WINDOW yalnız CAL-10-mühründen sonra kodlanır (F-CID-0 kapısı).
G-3  A1-pinleri: dr_cs1_acceptance(22) + cold_start_liveness(4) DOKUNULMAZ; dr_cs1_wire(12):
     yalnız g5 DUR-B-4a-isimli-amendmanla, kalan 11 dokunulmaz.
G-4  Her faz-sonu: cargo fmt --check + clippy -D warnings (iki-config) + test --all-features yeşil.
G-5  Merge / default-flip / A1-yeniden-açma YASAK. PR #44-draft üzerinde çalışılır.
G-6  Bu yüzeyin DIŞINA çıkma ihtiyacı = çalışmayı DURDUR + DUR-B (yüzey-amendmanı istenir).
G-7  Zaman-ölçen her yeni mekanizma engine-spec §16.2 tablosuna saat-sınıfıyla kaydedilir (ZM-3);
     karşılaştırmaların iki yanı aynı saat (ZM-4); yeni wall-clock sabiti 4-decade şartlı (ZM-2).
```

## 2. İZİN VERİLEN YÜZEY (crate → dosya → izin-kapsamı)

### 2.1 `crates/kama-integration` — BİRİNCİL YENİ-KOD YÜZEYİ (D-1: composition root)

| Dosya | İzin | Not |
|---|---|---|
| `src/lib.rs` | ÜRETİMLEŞTİRME | "lib-kod içermez / üretim-grafiğine girmez" beyanı değişir; ReceiverHost modül-ağacı açılır |
| `src/receiver_host.rs` (YENİ) | YENİ-MODÜL | SessionManager/HandleRouter/open-demux/clock-sampling/ColdRxState-çağrıları/transport-Receiver kompozisyonu + lifecycle/cleanup (v2-§4 hibrit-sıra) + receiver-emisyon-bütçesi |
| `src/` altında ek yeni modüller | YENİ-MODÜL | ReceiverHost'un alt-parçaları (ör. tombstone_registry.rs, cold_ledger-köprüsü). **Şerh (verify):** tombstone-KAYIT-içeriği opak noise-wire-tipidir; ham anahtar/AEAD-primitifi integration'da TANIMLANAMAZ (tip session.rs-dönüşüm-kancasında doğar; registry yalnız cap=64 + grace-deadline + lookup) |
| `Cargo.toml` | FEATURE+DEP | experimental-cold-start; üretim-dep'ler (engine/noise-wire/schema/adaptive-transport) |
| `tests/` | SERBEST | mevcut testler korunur (12 test yeşil kalır) |

### 2.2 `crates/kama-engine` — ColdRxState (SAF state; lifecycle-sahibi DEĞİL)

| Dosya | İzin | Not |
|---|---|---|
| `src/cold_rx_state.rs` (YENİ) | YENİ-MODÜL | rezervasyon-defteri (OPEN_BLOCK/RETAINED_BYTE/TOTAL_SHARD eksenleri + SHARDS_PER_BLOCK ön-kapı + PREPARAM min()-alt-kapı) + saf admission/deadline state-machine (enjekte `now`; saat TUTMAZ) + notice-slot (1/1/1) |
| `src/error.rs` | GENİŞLETME | CapExceeded{scope,kind∈4-eksen} + C-8 5-diag + ParamNeverArrived + InvariantViolation sınıfı (ADDITIVE; mevcut varyant değişmez) |
| `src/preparam_hold.rs` | DAR | YALNIZ zeroize-on-drop/evict eklentisi (fec-core-§9 P0-9); MAX_PREPARAM_SHARDS ve en-eski-drop-dalı DEĞİŞMEZ (A-1: dal ölü-kod-pinli kalır) |
| `src/lib.rs` | MODÜL-KAYIT | yeni modül + re-export |
| `Cargo.toml` | FEATURE | experimental-cold-start (+ zeroize-dep gerekirse — registry-dışı kripto-primitifi DEĞİL) |
| `tests/` | SERBEST | |
| **DOKUNULMAZ** | `admission.rs` (TYPE-GATE/profil-gate/N8), `recovered_map.rs`, `param_state.rs`, `param_bootstrap.rs` | semantik-değişiklik = DUR-B |

### 2.3 `crates/kama-noise-wire` — DAR dilimler

| Dosya | İzin | Not |
|---|---|---|
| `src/demux.rs` | DAR | `HandleRouter::unregister(h_ext)` (ADDITIVE; flag-OFF'ta çağrılmaz) |
| `src/session.rs` | DAR | lifecycle=Closing durumu + tombstone-dönüşüm kancası (K-1 pinleri; cfg-gated). **Tombstone-TİPİ burada doğar** (kripto-materyal noise-wire-tipi içinde kalır) |
| `src/protector.rs` | **DAR-İSİMLİ — A-4-AMENDMAN-KAPILI (verify-BLOCKER-çözümü)** | YALNIZ additive cfg-gated **`drop_send_state()`**: send_cipher'lar=None + last_sealed_pn-zeroize; **recv-state'e (recv_cipher/recv_windows) SIFIR dokunuş**. Gerekçe: K-1-pinleri ("send-cipher grace'te YAŞAMAZ / recv-cipher yalnız-grace / recv-ReplayWindow korunur") mevcut-pub-API'yle UYGULANAMAZ (cipher-alanları modül-private; alternatifler pin/§0.9-ihlali). Bu satır **§0.9-L4 isimli-spec-amendmanı (A-4 ADAY)** gerektirir — DUR-B-2 paketinde Abkarada-onayına bağlı; onaysız protector.rs'ye dokunulMAZ |
| `src/retry.rs` | DAR + F-CID-0-KAPILI | admit()-İMZASI DEĞİŞMEZ; probe/echo caller-dispatch zaten dışarıda (A-2); fixed-window (ControlDedupWindow) YALNIZ CAL-10-mühründen sonra; `ControlIdBelowDedupWindow` diag |
| `src/restart.rs` | DAR + **REPLAY_PAIR_CAP-onay-kapılı** | PersistentReplayGuard: REPLAY_PAIR_CAP + typed-reject + "cert-verify-sonrası" çağrı-sözleşmesi (cfg-gated); **REPLAY_PAIR_CAP değeri Abkarada-onaysız kodlanamaz** (talimat-§9); onay-öncesi typed-stub |
| `src/error.rs` | GENİŞLETME | ADDITIVE typed-varyantlar |
| `src/lib.rs` | DAR | YALNIZ yeni-pub-tip re-export satırları (davranış-sıfır) |
| `Cargo.toml` | FEATURE | experimental-cold-start |
| `tests/` | SERBEST — **dr_cs1_wire.rs istisnası** | dr_cs1_wire.rs: YALNIZ-g5 (DUR-B-4a); diğer mevcut A1-test-dosyalarında davranış-değiştiren edit YASAK; yeni (cfg-gated) test-dosyası serbest |
| **DOKUNULMAZ** | `pn.rs`, `aad.rs`, `outer.rs`, `inner.rs`, `control.rs`, `opened.rs`, `epoch_machine.rs`, `init_flight.rs`, `legacy.rs`, `param_bind.rs`, `handle_assign.rs`, `hub_sim.rs` + protector.rs'nin A-4-dilimi-DIŞI tamamı | mühürlü-kripto (§0.9 L4-NOISE-SPLIT) + wire-layout (§2.6) + codepoint-registry (§2.56) + control-codec — **her dokunuş = DUR-B + isimli-spec-amendman** |

### 2.4 `crates/adaptive-transport` — İKİ dar istisna dışında DOKUNULMAZ

| Dosya | İzin | Not |
|---|---|---|
| `src/sender.rs` | **YALNIZ Tetik-2 dilimi; DUR-B-3-KAPILI** | RecoveredControl-işleyicisinde post-timeout-durum kontrolü + PostTimeoutNotice-defteri + `take_post_timeout_notice()` (K-2/D-6 sözleşmesi). ŞARTLAR: N8-bit-özdeşlik (fires-vs-noop BtlBw) + tek-terminal korunur (CsTerminal'e varyant EKLENMEZ) + flag-OFF-byte-özdeş. bw/loss/rate/pacer/fusion-yollarına SIFIR satır |
| `src/udp_runner.rs` + `examples/` + `tests/` | TEST-HARNESS | A1-rejimi aynen (test-only; W-3-65-rerun/plateau enstrümantasyonu) |
| **DOKUNULMAZ** | `receiver.rs` (transport-çekirdeği; patience HOST'ta yaşar), `bw.rs`, `loss.rs`, `fusion.rs`, `rate.rs`, `pacer.rs`, `arq.rs`, `ack.rs`, `budget*.rs`, `fec.rs`, `seq.rs`, diğerleri | INV-RATE-1 / N8 / INV-ENGINE-SOC bölgesi |

### 2.5 Tamamen DOKUNULMAZ crate'ler

```text
crypto · ccr-codec-alanı · kama-fec-codec · kama-transport-schema · hub-server · client-core ·
ra-ca · discovery · admin-server
(schema'ya alan/codepoint ekleme zaten §2.56-DUR-B; A2'nin hiçbir maddesi wire'a alan eklemez)
```

### 2.6 Doküman/script yüzeyi (serbest; kayıt-disiplinli)

`docs/**`, `reports/**`, `scripts/netem/**` — sessiz-silme yok; SUPERSEDED-üst-not deseni;
status-board yalnız DUR-SON hükümleriyle.

## 3. YÜZEY-DENETİMİ (her faz-sonu mekanik kontrol)

```text
git diff --name-only <faz-başı>..HEAD  →  her satır §2-tablolarındaki İZİNLİ yüzeyde mi?
Değilse: commit GERİ-ALINIR (cp-bak/stash disipliniyle; git-checkout-üzerine-yazma YASAK) + DUR-B.
```
