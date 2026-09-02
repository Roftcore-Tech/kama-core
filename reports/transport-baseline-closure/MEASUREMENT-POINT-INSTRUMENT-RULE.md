# PERMANENT INSTRUMENT RULE — THREE MEASUREMENT POINTS

Scope: binding on the DR-CS-3 structural oracle, on the forthcoming channel event log
contract, and on every future paired-arm gate in this programme.

## The three points

    call_entry_state
      = state visible from OUTSIDE, before on_event is called.

    decision_entry_state
      = state INSIDE on_event, after the refill / cap update, immediately before the
        first scheduler/arbitration decision.

    post_turn_residue
      = state remaining after on_event returns.

## The rule

    Post-call diag state may prove terminal/residual facts.
    It may not be used as an arbitration-entry predicate.
    Decision predicates must come from the decision point, or from an exact,
    fully reconciled state-transition ledger.

For the Gate-5 grant predicate the authoritative point is `decision_entry_state`.

Reading `diag_pacer()` BEFORE the call is NOT sufficient on its own: the refill happens
inside the same call, so the pre-call value can be stale. Reading it AFTER the call yields
only `post_turn_residue`.

## Reconciliation (when the decision point is not directly observable)

    decision_entry_tokens = post_turn_tokens + paced_debit_bytes

Admissible ONLY after closing every token mutation by code lineage:

  * no second refill inside the target turn;
  * burst_cap does not change later in the turn;
  * the bootstrap Probe debits no tokens (additive / unpaced);
  * no overlooked recovery or parity debit;
  * every try_send success and its byte cost is recorded.

General event-free form, with its initial conditions stated explicitly:

    decision_entry_tokens(T)
      = min(burst_cap,
            call_entry_tokens + floor(rate * elapsed_since_last_refill / 1e9))

The compact form `min(101, floor(31*T))` is valid ONLY where the initial token value and
`last_refill` genuinely resolve to a zero reference — true for the base fixture (tokens=0,
t=0), not assumable elsewhere.

## Recorded as historical, invalid as predicate

    tokens = 31 / 89 / 71   ->  POST-TURN RESIDUE — INVALID FOR ENTRY PREDICATE

These were the readings that made three differently-named cells collapse to one observed
grant regime (the RULE-8 violation). They are preserved as results, not reused as state.

## burst_cap lineage is CELL-SPECIFIC

    window_bytes < front_size
      => burst_cap locks to front_size
      => the bucket carries approximately ONE front-packet grant

So MultipleDataGrants is impossible here because of the front_size-based cap lineage, not
because of the literal constant 101. If the rate or the front packet changes -- e.g. a
retransmit or a parity unit reaches the queue front in cells 5/6 -- constructibility must be
re-evaluated for that cell. The base value burst_cap=101 is closed ONLY for cells where a
101 B DATA unit is at the queue front at the moment set_burst_cap runs.

## Contention non-vacuity

    decision_entry_tokens < DATA cost
      => the control (OFF) arm can use no DATA grant
      => nothing exists for a Probe to displace
      => ACTIVATED-BUT-VACUOUS-FOR-DATA-DISPLACEMENT

Such a cell may still serve as a Probe activation / state-invariance control, but it does
NOT enter the denominator of the Gate-5 DATA-displacement claim.
