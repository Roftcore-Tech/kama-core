# DR-CS-3 — PANEL CONTRACT V2 (channel semantics + acceptance)

**Frozen before any V2 panel is run.** Hashed prior to the harness change that implements it.
Supersedes the V1 panel's channel model; the V1 result is preserved, not rewritten.

Base: `main` = `3e6dfa30`, branch `transport-baseline-closure` = `e39b587e` (closure gate 12/12).

---

## 1. Why V1 is not a valid default-promotion panel

V1 measured `2/3` locked recovery. Its channel model was not sound for that comparison:

* the **baseline's** ACK reverse path was **lossless**, while the **treatment's** Echo reverse path
  was **GE-lossy** — an impairment applied to the treatment only;
* Probe drew from its own stream (`DRCS3ProbeForward`) and Echo from another
  (`DRCS3EchoReverse`), so packet classes had independent channel state;
* each class advanced its GE chain from its **own** packet index, so four echoes spread over 45 s of
  virtual time consumed four *consecutive* draws — maximal burst correlation between events that are
  6 s, 12 s and 24 s apart.

`rep14` failed exactly there: 4/4 probes authenticated, 4/4 echoes generated at the 172 B budget,
0 echoes reaching the wire because that private chain read `[D, D, D, D]`.

V1 is retained as a **historical diagnostic**. It is not a confirmatory panel and cannot promote a
default.

## 2. Binding channel semantics (V2)

1. **One forward channel state.** DATA and Probe share a single forward channel object and advance
   it **chronologically**, in emission order. There is no per-class forward stream.
2. **One reverse channel state.** ACK and Echo share a single reverse channel object and advance it
   **chronologically**, in emission order. There is no per-class reverse stream.
3. **No treatment-only impairment.** Any impairment applied to Echo is applied to ACK, and any
   impairment applied to Probe is applied to DATA, by construction — because they are the same
   channel.
4. **GE stays packet-domain.** State advances once per packet that traverses the channel, in
   chronological order. It is **not** advanced from a per-class index, and it is **not** converted to
   a time-decay model — that conversion would be a post-hoc change made after seeing V1's outcome.
5. **Separable RNG, identical distribution.** A distinct RNG stream per direction is permitted so the
   treatment cannot shift the baseline's DATA draw sequence; the distribution and path semantics must
   be identical. Baseline and treatment must see the same channel realisation for the packets they
   share.

## 3. Two panels, reported separately

**Primary continuity panel.** The frozen DR-CS cell is forward-loss + reverse-lossless, so:
DATA and Probe take the forward impairment; ACK and Echo are both lossless. This preserves continuity
with the frozen baselines and is the panel that decides the default.

**Bidirectional robustness panel.** DATA and Probe take the forward impairment; ACK and Echo take the
same reverse impairment. Baseline and treatment both face the two-way channel. This is a new
diagnostic; it makes no claim to reproduce the frozen baselines and its results are never merged with
the primary panel's.

## 4. Outcome classes

* **`RTT-OPPORTUNITY`** — at least one Probe reached the receiver and at least one corresponding Echo
  reached the sender. Mechanism must then produce: matched local RTT · `srtt>0` · PreConverge exit.
  A run with an opportunity but no exit is a genuine mechanism failure.
* **`NETWORK-CENSORED`** — every Probe was forward-dropped, or every generated Echo was
  reverse-dropped. Not a mechanism defect and **not a PASS**. Reported as its own class and **kept in
  the denominator** of the frozen acceptance rate. It may not be excluded post hoc, and seeds may not
  be changed to avoid it.
* **`SAFETY-FAILURE`** — budget overrun · replay/mismatch RTT · duplicate RTT sample · authentication
  violation · DATA displacement · ARQ timer diff. Never counted as censored.

## 5. Acceptance gates for `BootstrapRttConfig::default() = true`

Frozen here, before any V2 result is seen:

| # | gate |
|---|---|
| 1 | locked primary panel: **3/3** of the known stalls recover |
| 2 | new stalls: **0** |
| 3 | fresh primary N=40: first valid ACK in at least **38/40** |
| 4 | zero-transmission stalls: **0** |
| 5 | safety / budget / invariant violations: **0** |
| 6 | no-loss RTT{50, 250, 1000}: completion regression **0** |
| 7 | if a valid ACK arrives before the first probe instant, bootstrap overhead is exactly **0** |
| 8 | otherwise overhead stays within the already-frozen protocol budget (sender ≤ 200 B, receiver ≤ 172 B, echo 43 B < probe 44 B) |
| 9 | `gate.sh` fully green |

`NETWORK-CENSORED` runs count against gates 1 and 3 (they are in the denominator).

## 6. If a gate fails

Bootstrap is **not** left OFF and skipped past. Per C1: verify the instrument through §4 (A0 state ·
A1 counter semantics · A2 observation coverage · A3 positive control), find the real product root
cause, apply the minimum bounded fix **on the shipping `Sender` + `ReceiverControlCore` path**, and
re-run this same panel. Harness echo, fake responders and adapters without a product counterpart stay
forbidden.

## 7. Harness scope

`DEFAULT-BUILD AUTHENTICATED TRANSPORT COMPONENT PANEL WITH TEST-INJECTED SESSION KEYS`.
Real product `Sender`, real default-build `ReceiverControlCore`, real schema codec, real AEAD.
It does **not** validate the Noise handshake, a product binary, or a deployment network path — those
are M4.
