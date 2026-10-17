# kama-mobile — Hasan için teslim notu (K4G-15, `poc-uniffi-v0`)

**Paket.** `jniLibs/arm64-v8a/libkama_mobile.so` · `kotlin/uniffi/kama_mobile/kama_mobile.kt` (UniFFI 0.32.2 üretimi, paket `uniffi.kama_mobile`) · `KM-SEAM-AK-001.md` (ui-api v0.3 ↔ Rust eşlemesi, satır satır) · `PROVISIONING-FORMAT-v0.md` · `device-alice.json` + `device-bob.json` (örnek; `kama-provision` ile paketleme anında üretildi — `root.json` pakette yoktur, D9).

**Ekleme.** `.so` → `src/main/jniLibs/arm64-v8a/`; `.kt` → kaynak ağacı (elle değiştirme; her sürümde yeniden üretilir). Bağımlılık yalnız JNA 5.x: `implementation("net.java.dev.jna:jna:<5.x>@aar")`; kütüphaneyi JNA yükler (`System.loadLibrary` gerekmez). R8 açıksa: `-keep class com.sun.jna.** { *; }` ve `-keep class uniffi.kama_mobile.** { *; }`. Yalnız arm64-v8a, minSdk 28, 16 KB sayfa hizalı.

**Başlatma.** Provisioning dosyasını `noBackupFilesDir`'e yaz; sonra ana iş parçacığı DIŞINDA bir JVM iş parçacığından (disk ve soket açar):

```kotlin
val node = KamaMobile.start(configPath, dataDir, listener) // dataDir: uygulamaya özel, yoksa oluşturulur
```

Kapanış: `node.shutdown()`, sonra `node.close()`. JVM iş parçacığından `shutdown()` tam duruşu bekler: döndüğünde depo kilidi, UDP soketi ve süreç-içi röle kaydı bırakılmıştır, aynı `dataDir` ile yeniden `start` edilebilir; eşzamanlı ikinci `shutdown()` da ilkinin bitişini bekler. `onEvent` içinden `shutdown()` asenkrondur ve kapanışı HER DURUMDA ister: beklemeden döner, düğüm dinleyici döndükten sonra durur; yuva o an başka bir çağrıda tutuluyorsa (ör. süren `sendText`) kapanış kısa ömürlü bir iş parçacığına devredilir. Tek hatası `NodeException.Spawn`'dır (iş parçacığı açılamadı, kapanış istenmedi) — bunu yutma. Başka düğümün `shutdown()`'ı dinleyiciden senkrondur (join). Kapanış başlayınca her çağrı `NodeException.Stopped` atar; `metrics()`, `sendFile` ve `onPlatformSignal` yuva kilidini ve düğüm turunu beklemez (`metrics` yalnız ölçü anlık görüntüsünün kısa kilidini alır).

**Olay akışı.** `EngineListener.onEvent(ev)` düğümün kendi iş parçacığında çağrılır: hızlı dön (olayı `Channel`/`StateFlow`'a koy). İçinden aynı düğümün yanıt bekleyen çağrısı `NodeException.ReentrantCall` alır. Koruma yalnız aynı düğüm için; dinleyiciden başka düğüme engelleyici çağrı yapma. `connect(peer)` → `ConnectProgress` → `Connected` · `sendText` → karşıda `TextReceived`, sende `Receipt(msgId)` (= teslim edildi) · `sendFile(peer, path)` (yerel dosya yolu, `content://` değil) → `FileProgress(SENT, DELIVERED)`, karşıda `FileReceived(path)`; oturumsuz `sendFile` hata atmaz, `FileFailed(Outgoing(h), NotQueued(NoSession))` olayı gelir · `pttOpen`/`pttPush`/`pttClose` → karşıda `PttOpened` → `PttBundle`… → `PttClosed` · `sendSignaling` → `SignalingReceived` · saniyede bir `Metrics`. Hatalar tiplidir: `NodeException` (`NodeError` aynası; ör. `Busy`, `TooLarge`, `Ptt(PttError.Transport(TransportError.BACKPRESSURE))` → bekle ve aynı girdiyi yeniden ver). Attığın `kotlin.Exception` yutulur, düğüm etkilenmez; `kotlin.Error`'u UniFFI yakalamaz (JNA mesajı ve yığınıyla loglar):

onEvent gövdesini `try { … } catch (t: Throwable) { }` ile sar

**Zorunlu notlar** (plan K4G-15 Ek (5), DUR-K4G-11b ve DUR-K4G-15-B):

1. Bu sürümde iki telefon arası oturum YOK — el sıkışma zarfı taşıyıcısı hub Plane 1 rölesidir (hub sprinti); bu `.so` API entegrasyonu ve süreç-içi loopback (iki `KamaNode` aynı süreçte) içindir
2. `start()` async/tokio bağlamından çağrılmaz; JVM iş parçacığından çağrılır
3. `metrics()`'te `rtt_ms`/`loss_pct`/`bitrate_kbps` bu sürümde `None`; overlay alanları hub sprintinde dolar
4. Sinyalleşme (`sendSignaling`) alındısız ve dedupsuz: içerik kendi `call_id`/sıra numarasını taşımalı; oturum değişiminde 60 s içindeki bloblar yeniden sunulabilir
5. `pttOpen(peer, codec, bundleMs)` — codec ve demet süresi çağıran verir
6. Arayüz "Bağlandı" olayını (`EngineEvent.Connected`) bekler; oturumsuz `sendText` → `NodeException.Messenger(MessengerError.NoSession)` davranışı kalır (DUR-K4G-11b)
7. İki cihaz dosyası aynı pakette yalnız süreç-içi loopback testi içindir; gerçek kurulumda her telefon yalnız kendi dosyasını alır (Sprint 2 provisioning v1 + Deploy TDD §8).

**Ayrıca.** Kotlin'de alan adları camelCase'dir (`rttMs`, `lossPct`, `bitrateKbps`); kayıtlar değiştirilemezdir (`val`; `StreamHandle`/`FileHandle` harita anahtarı olabilir). Örnek dosyalarda veri adresleri alice `127.0.0.1:47001`, bob `127.0.0.1:47002`: iki dosya aynı süreçte iki `KamaMobile` içindir. `EngineEvent`'in Kotlin `toString()`'i Rust `Debug`'ıdır: metin, ses yükü ve blob yerine uzunluklarını yazar; alanların kendisi (`ev.text`, `ev.payload`, `ev.bytes`) içeriktir — loglama.
