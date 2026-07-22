# AE — NİHAİ POLICY-SELECTION REVIEW [production-default-DEĞİŞMEDİ]

> Zincir: AA(f1683b6) AB(e0e3a0d) AC(0b4a54a) AD(önceki-commit). Deterministik-kanıt-TAM;
> netem-EXECUTE-EDİLEMEDİ (typed-blocker).

**Deterministik-nihai-tablo (cluster-inference-birimi=seed):**
F15-vs-Leftover: dl_op +0.100 [0.040,0.170]-CI-dışlamalı (rev99-30-seed) + decode/avoided-retx/worst-
decile-tümü-dışlamalı → OUTCOME-FAYDASI-KANITLI. F15-vs-P15: dl_op-EŞDEĞER; decode +15.5 [9.5,19] +
avoided-retx +9 + worst-decile −1.7s + **spend −48k (F15-daha-AZ-harcar)** — fairness≠fazla-spend-
KANITLI. Blackout-30s-harm: kök-neden-kapalı (dark-start-fazı; F10-6/10-kurtarır; recovery-normalized-
harm-0; bounded ~1-blok/seed) — kabul-edilebilir-veya-F10-ile-azaltılabilir-SINIFINDA ama nihai-kabul-
kullanıcı-objective-sıralamasına-ve-netem-yönüne-bağlı.

**§14-karar-kuralı-uygulaması:** F15-seçilebilirlik-şartlarından 4/6-karşılandı (cluster-korunumu ✓;
spend-açıklaması-elendi ✓; blackout-bounded+F10-azaltımı ✓; DATA/ARQ-tail-bound ✓; queue/cap/lifecycle ✓)
— **"netem'de-≥2-impairment-ailesi-aynı-yön"-şartı-KARŞILANAMADI (koşu-bloke)** → PASSED-hükmü-
KURULAMAZ. Inconclusive-tetikleyicisi: "Netem-çalıştırılamaz."

**HÜKÜM-ÖNERİSİ:** DR-1b FINAL POLICY SELECTION **INCONCLUSIVE — NETEM EXECUTION BLOCKED**
(exact: root/CAP_NET_ADMIN-yok, sudo-n-parola-ister; runner+harness-READY; deterministik-kanıt-
F15-yönlü-ve-cluster-doğrulamalı). Kalan-tek-adım: self-hosted-nightly'de scripts/netem/
run_policy_validation.sh → yön-tablosu → §14-yeniden-uygulama.
