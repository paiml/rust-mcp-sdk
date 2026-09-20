# API Coverage — pmcp.run attestation contract (contract-first, parked)

> Full coverage by default. Opt-outs are explicit, reasoned decisions.

Scope: the pmcp.run GraphQL surface this phase proposes and vendors at
`contracts/pmcp-run/attestation-v1.graphql`. The decision below is D-11 in
`122-CONTEXT.md`, taken during `/gsd-discuss-phase` on 2026-08-25.

| capability | decision | reason |
|---|---|---|
| verifyAttestation | INTEGRATE | contract-first: SDK-proposed SDL + offline blocking contract test; request builder and response decoder land as PRODUCTION code in `graphql_contract.rs` with their own offline unit tests; live leg parked behind `#[ignore]` + a triple env gate (D-11) |
| getAttestation | OPT-OUT | speculative until Phase 123 settles import semantics (D-11) |
| issueAttestation | OPT-OUT | entirely the platform's to design; issuance is the parked backend critical path (D-11, PKGX-F2) |

*Re-validated against the plans during reviews-mode replanning, 2026-08-25. The capability decisions are unchanged; only the INTEGRATE row's reason gained precision, because 122-04 now lands `verify_attestation_request_body`, `decode_verify_attestation_response` and `GRAPHQL_AUTH_HEADER` as production code rather than leaving the client half to unpark time.*

## Why the integrated capability is only half-integrated

The attestation *arrives inside the package* — that is what carriage means — so
the CLI never fetches one. The single remote need is asking the platform to
verify an attestation against its own identity, and that call is precisely the
parked leg: the SDK holds no keys and adds no crypto dependency (scoping
Decision 1), so signature verification is necessarily remote, and the backend
that would answer it is not scheduled.

Offline verification in this phase is the D-02/D-03 subject-digest comparison
and nothing more. That is stated plainly at every site where the phrase
"verification path" appears.

What "half-integrated" does NOT mean, after cross-AI review: it does not mean the
client half is unwritten. The request builder, the response decoder and the
`Authorization` header constant are shipped production code with offline unit
tests, and the parked test already calls all three over a real HTTP POST behind
its gates. The half that is missing is the BACKEND, not the client. Unparking is
deleting an `#[ignore]` and three early-return gate blocks; it is not writing a
request path.

## Why the two opt-outs are not gaps

Proposing the minimum for a contract we do not own means the platform ratifies
one operation, not three. `getAttestation` cannot be designed before Phase 123
settles what `import` means; `issueAttestation` is the platform's design, not
the SDK's, and is tracked as future requirement PKGX-F2.

*Phase: 122 — Attestation Carriage (contract-first, PARKED on the pmcp.run backend)*
*Requirement: PKGX-01*
