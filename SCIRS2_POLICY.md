# SCIRS2 Policy Exception — kaccy Workspace

**Date:** 2026-06-23
**Decision:** Keep `rand`, `rand_distr`, and `statrs` in this workspace.
**Scope:** This repository only (`/notebooks/kaccy`).

---

## Background

The global COOLJAPAN workspace policy states:

> "You do not need to use rand (rand_distr) / ndarray -- instead, Please use SciRS2-Core."

This document records a deliberate, reviewed exception to that policy for the kaccy workspace.

---

## Rationale

### 1. Domain mismatch

kaccy is a Bitcoin protocol and web-application workspace, not a member of the SciRS2
numerical-ecosystem family (SciRS2, NumRS2, ToRSh, SkleaRS, etc.). The SciRS2 policy exists
to keep those crates internally consistent and to avoid pulling in `rand`/`ndarray` alongside
SciRS2-Core primitives. That concern does not apply here.

### 2. Hard cryptographic dependency

The core Bitcoin and cryptography crates that kaccy depends on require `rand` as a direct,
non-optional transitive dependency:

- `secp256k1` — key generation, nonce generation, signature randomization
- `bip39` — mnemonic entropy generation
- `aes-gcm`, `chacha20poly1305` — AEAD nonce generation
- `ml-dsa`, `ml-kem` — post-quantum key generation (ML-DSA-65, ML-KEM-768)

Replacing `rand` with SciRS2-Core RNG primitives in this context would require either patching
upstream cryptography crates (not feasible) or maintaining a compatibility shim that
re-implements the `rand::RngCore` trait on top of SciRS2-Core — a large, risky change with
no functional benefit and a meaningful risk of introducing subtle cryptographic weaknesses.

### 3. Risk vs. benefit

A `rand` -> SciRS2-Core migration in kaccy would:

- Require changes across every crate that performs key generation or AEAD nonce construction
- Break interoperability with all upstream crypto crates that accept `&mut impl RngCore`
- Provide zero user-visible benefit (the computation results are identical)
- Introduce audit surface in security-critical code paths

### 4. `statrs` usage

`statrs` is used in `kaccy-core` for statistical functions (normal CDF, inverse-normal) in
the risk and advanced-forecasting modules. SciRS2-Core provides equivalent functionality,
but the migration would again offer no functional benefit and would require retesting
security-adjacent numerical results. This is deferred to a future maintenance cycle.

---

## Decision

**rand, rand_distr, and statrs are retained as explicit, approved dependencies in the kaccy
workspace.** This is a deliberate documented exception, not an oversight.

Any future refactor proposal to migrate these must:

1. Confirm that all upstream crypto crates (`secp256k1`, `bip39`, `aes-gcm`, etc.) can
   accept SciRS2-Core RNG via a `rand::RngCore` compatibility adapter without changing
   their public API.
2. Pass a full cryptographic correctness review of the resulting nonce and key generation
   paths.
3. Be tracked as a separate PR with a dedicated security review.

Until those conditions are met, the exception documented here remains in force.

---

## Affected crates

| Crate | rand usage |
|---|---|
| kaccy-core | AEAD nonce generation, Monte Carlo sampling, GARCH noise |
| kaccy-bitcoin | secp256k1 key generation, BIP-39 entropy, SLIP-39 sharing |
| kaccy-api | CSRF token generation, PKCE verifier generation |
| kaccy-reputation | secp256k1 credential signing |
| kaccy-ai | (indirect, via upstream deps) |
| kaccy-db | (indirect, via upstream deps) |
