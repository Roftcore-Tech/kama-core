# FB — DR-1b FINAL FREEZE [production-default-LeftoverOnly; bağlayıcı-kayıt]

> Phase-A (FA-graceful-shutdown) offered≈C SIGTERM-race'i-kapattı: rx FINISH-marker'la-temiz-çıkar,
> done-marker+checksum-yazar (clean_shutdown=true). Önceki-eşitlik-hükmü (netem-AH: outcome-policy-
> bağımsız) FA-marker'lı-rerun'da-KORUNDU (offered_c dahil complete-ledger; çelişki-YOK).

## Bağlayıcı-kayıtlar (freeze)
1. **Production/default = `BudgetPolicy::LeftoverOnly`** (DEĞİŞMEDİ).
2. F15/P15 = yalnız `experimental-fec-budget`-feature-arkası; default-DEĞİL; product/admin-activation-YOK.
3. Deterministic-F15-advantage = **model-fidelity-diagnostic** (production-policy-selection-evidence-DEĞİL).
4. Real-UDP/netem-outcome = test-edilen-profillerde **policy-independent** (Leftover=P15=F15 dl_op-özdeş).
5. F15-mekanizma-SAFE-ama-outcome-benefit-UNPROVEN (regresyon-yok; harm-yok).

## REOPEN-condition (exact; hepsi-gerçek-stack'te-kanıtlanmalı)
```
requested_r > 0  ∧  enqueued_r > 0  ∧  emitted_r/requested_r-DÜŞÜK
∧ düşük-emission GERÇEK decode/deadline/retransmission-KAYBINA-yol-açıyor
∧ ProtectedShare/FloorCap aynı-trace/profil-ailesinde outcome'u-İYİLEŞTİRİYOR
```
Yalnız-düşük-realization / yalnız-token-denial / yalnız-deterministic-model YETERLİ-DEĞİL.
