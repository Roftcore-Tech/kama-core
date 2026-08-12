# DR-CS-3 — GROUNDING CORRIGENDUM

The verdict recorded in `DR-CS-3-PRODUCT-PATH-GROUNDING.md`
(sha256 `dd6737987c43696982565bd714cceddb…`) is **renamed**:

```text
was:  PRODUCT-BOOTSTRAP-PROBE-PATH-GROUNDED
now:  BOOTSTRAP-PROBE-PRODUCTIZATION-MAPPING-GROUNDED
```

**Reason.** What was grounded is the *mapping*, not a working path: the wire format
(`Control::CsProbe`/`CsEcho`, `0x19`/`0x1A`, ungated in `kama-transport-schema`), the parser, the
authentication boundary (`receiver_host.rs:318` AEAD-open before `:331`) and the outbound response
pattern (`emit_recovered`) all exist — but at that moment **neither the sender emitter nor the receiver
responder was implemented**. The original wording could be read as "the path works", which it did not.

The original artifact is **preserved unchanged**; this corrigendum supersedes only its verdict label.
Every technical finding in it stands, including both validation-architecture limits.

**Status after this turn:** the mapping has been *implemented* — see
`DR-CS-3-IMPLEMENTATION-REPORT.md`.
