# Docker Schema Alignment

## Summary

Fixed Docker schema configuration drift after moving the application schema from `public` to `attune`.

## Changes

- Added explicit `ATTUNE__DATABASE__SCHEMA=attune` overrides to root Docker Compose application services so runtime services use the same schema as migrations and bootstrap jobs.
- Updated the distributable Docker Compose file from `public` to `attune` for API, executor, workers, sensor, and notifier services.
- Updated `AGENTS.md` schema guidance to reflect the dedicated `attune` schema for Docker/development/production and isolated `test_*` schemas for tests.

## Root Cause

The migration runner and init jobs create and load objects into the `attune` schema, while some runtime service configurations still pointed at `public`. Services with `search_path=public` could not resolve tables or enum types such as `inquiry` and `inquiry_status_enum`, producing errors like `relation "inquiry" does not exist` and `type "inquiry_status_enum" does not exist`.

## Verification

- Confirmed no remaining YAML Compose/config schema overrides point to `public`.
- Validated Docker Compose config rendering for:
  - `docker-compose.yaml`
  - `docker/distributable/docker-compose.yaml`
  - `docker-compose.yaml` + `docker-compose.agent.yaml`
