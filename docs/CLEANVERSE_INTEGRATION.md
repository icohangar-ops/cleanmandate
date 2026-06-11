# Cleanverse Integration

CleanMandate targets Cleanverse API v3 primitives for **Track 02: Trusted AI Agent Transactions**.

## Primitives

### A-Pass (Agent Passport)

Binds the **principal wallet** to a verified identity before any agent-initiated transfer.

```http
POST /apass/verify
{ "wallet": "0x..." }
```

Client: `cm-cleanverse::CleanverseClient::verify_apass`

### CCP (Compliance Control Plane)

Pre-transaction screening with **Travel Rule** payload (originator/beneficiary names, VASPs, purpose).

```http
POST /ccp/pre-check
{
  "mandate_id": "...",
  "from_wallet": "0x...",
  "to_wallet": "0x...",
  "amount": "250.00",
  "asset": "A-USDC",
  "chain": "monad-testnet",
  "travel_rule": { ... }
}
```

### A-Token

Compliant stablecoin transfer after CCP clearance and CHP lock.

```http
POST /atoken/transfer
{
  "mandate_id": "...",
  "ccp_reference": "ccp-...",
  ...
}
```

## Modes

| Mode | Env | Use |
|------|-----|-----|
| **Mock** | `CLEANVERSE_MODE=mock` (default) | Offline hackathon demo |
| **Sandbox** | `CLEANVERSE_MODE=sandbox` + `CLEANVERSE_API_KEY` | Integration testing |

API documentation requires a Cleanverse invitation code — contact support@cleanverse.com or use hackathon onboarding.

## Monad

Set `chain: monad-testnet` in mandates. A-Token settlements target Monad's EVM-compatible execution layer (10k TPS, sub-second finality per hackathon brief).

## Audit export

Every gate writes to `.cleanmandate/audit.jsonl` with optional HMAC signatures (`CLEANMANDATE_SIGNING_KEY`). Export bundles are suitable for Travel Rule record-keeping and agent accountability.
