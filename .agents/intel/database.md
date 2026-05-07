# Database Design

## Read Policy

- Tier: `deep-context`
- Deep-read only. Open when the task requires database design, RLS, RPC, migration timeline, or relationship detail.

pg_query-backed ordered static database map for agents. This reduces migration files in path order for common schema changes without requiring a live database; procedural or dynamic SQL may still need source inspection.

## Summary

- SQL files: 0
- Migration files: 0
- Tables discovered: 0
- Functions/RPCs: 0
- Policies: 0
- Indexes: 0
- Triggers: 0
- Relationship edges: 0

## SQL Timeline


## Relationship Map

- No foreign-key style references detected.

## Tables

## Functions And RPCs


## Types And Views


## Drops And Replacements

