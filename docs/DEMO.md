# CleanMandate — Demo Day Script

**Track 02: Trusted AI Agent Transactions** · ~3 minutes

## Setup

```bash
cd cleanmandate
cargo build --release -p cm-cli
export PATH="$PWD/target/release:$PATH"
```

## Act 1 — The mandate (30s)

Open `examples/mandates/vendor-payment.json`:

- Procurement agent requests **$250 A-USDC** to an allowlisted vendor
- Full **Travel Rule** metadata (originator, beneficiary, purpose)
- Principal wallet bound via **A-Pass**

## Act 2 — Policy gate (30s)

```bash
cleanmandate validate --mandate examples/mandates/vendor-payment.json
```

Show: recipient allowlist ✓, under daily cap ✓, Travel Rule fields present ✓.

## Act 3 — Full pipeline dry-run (60s)

```bash
cleanmandate pay --mandate examples/mandates/vendor-payment.json --dry-run
```

Walk through JSON output:

1. **A-Pass** — principal verified
2. **CCP** — Travel Rule cleared, `ccp_reference` issued
3. **CHP** — auto-lock (amount under human threshold)
4. **Audit** — events in `.cleanmandate/audit.jsonl`

## Act 4 — Human-in-the-loop (30s)

Duplicate mandate, set `"amount": "6000.00"`, run `pay` again.

Show `status: chp_review` — agent cannot move funds without principal approval.

## Act 5 — Live sandbox (30s)

```bash
export CLEANVERSE_MODE=sandbox
export CLEANVERSE_API_KEY=<hackathon_key>
cleanmandate pay --mandate examples/mandates/vendor-payment.json
```

Show **A-Token** `tx_hash` on Monad testnet.

## Act 6 — Audit export (15s)

```bash
cleanmandate export --mandate-id 550e8400-e29b-41d4-a716-446655440000
```

Signed compliance bundle ready for regulators.

## Talking points

- **Problem:** Agents can pay; institutions can't trust them.
- **Cleanverse:** A-Pass + CCP + A-Token = verified identity, clean funds, compliant settlement.
- **Differentiator:** Composes with MCP (`agent-conductor`), policy-as-code, CHP locks — not a wallet, a **mandate layer**.
