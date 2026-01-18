# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records for `kaccy-db`.

## What is an ADR?

An Architecture Decision Record (ADR) captures an important architectural decision made along with its context and consequences.

## Format

Each ADR follows this format:

```markdown
# [Number]. [Title]

**Status**: [Proposed | Accepted | Deprecated | Superseded]
**Date**: YYYY-MM-DD
**Deciders**: [List of people involved]

## Context

What is the issue we're facing and why does it need a decision?

## Decision

What decision did we make?

## Consequences

What are the positive and negative consequences of this decision?

## Alternatives Considered

What other options did we consider?
```

## Index

- [ADR-001](001-use-sqlx-for-database-access.md) - Use SQLx for Database Access
- [ADR-002](002-multi-tier-caching-strategy.md) - Multi-Tier Caching Strategy
- [ADR-003](003-replica-load-balancing.md) - Replica Load Balancing Strategy
- [ADR-004](004-sharding-strategy.md) - Database Sharding Strategy
- [ADR-005](005-observability-approach.md) - Observability and Metrics Approach
