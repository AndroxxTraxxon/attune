# LDAP Secrets Management

When LDAP authentication uses **search-and-bind** mode, Attune connects to the
LDAP server with a long-lived service-account credential
(`security.ldap.search_bind_dn` / `security.ldap.search_bind_password`) before
re-binding as the end user. That password is sensitive and **must never be
committed to YAML config files** that are checked into source control or copied
into container images.

This document explains how to inject the password safely on each common
deployment platform, why it matters, and how Attune warns operators when a
literal value is detected.

## Why this matters

`config.example.yaml`, `config.production.yaml`, and `config.docker.yaml` are
designed to live in version control. Anything written into them is implicitly
shared with everyone who can read the repository or pull the image. A leaked
read-only LDAP service account is enough for an attacker to:

- enumerate every user and group in your directory,
- gather attributes (mail, phone, manager, employeeId, …) for phishing,
- correlate user accounts across systems,
- and, depending on directory ACLs, bind as users in some configurations.

For these reasons Attune treats `search_bind_password` as a runtime secret and
expects it to be sourced from the environment.

## Recommended mechanism: `ATTUNE__` env-var override

The `config` crate Attune uses already supports environment-variable overrides
with the prefix `ATTUNE__` and `__` as the separator. To set the LDAP service
account password without touching any YAML file, export:

```bash
ATTUNE__SECURITY__LDAP__SEARCH_BIND_PASSWORD='your-real-password'
```

This value takes precedence over anything in `config.yaml` and
`config.{environment}.yaml`. Leave the YAML field empty (`""`) or omit it
entirely.

## Platform-specific examples

### Plain shell / systemd unit

```bash
# /etc/attune/api.env  (chmod 0600, owned by the attune user)
ATTUNE__SECURITY__LDAP__SEARCH_BIND_PASSWORD=your-real-password
```

```ini
# /etc/systemd/system/attune-api.service
[Service]
EnvironmentFile=/etc/attune/api.env
ExecStart=/usr/local/bin/attune-api
```

For systemd ≥ 250 you can use [credentials](https://systemd.io/CREDENTIALS/)
(`LoadCredential=ldap_pw:/run/secrets/ldap_pw`) and read the file path inside
the service if you prefer file-based secrets over env vars.

### Docker / Docker Compose

Use a local `.env` file (already supported by `docker-compose.yaml`) — see
`env.docker.example` for the canonical entry:

```bash
ATTUNE__SECURITY__LDAP__SEARCH_BIND_PASSWORD=your-real-password
```

Compose will pass that through to the `api` container automatically because
the service inherits the project-level environment. Do **not** check the
populated `.env` file into git.

For stronger isolation use Docker Swarm secrets and read them via an entrypoint
shim that exports the env var from `/run/secrets/<name>` before exec-ing the
API binary.

### Kubernetes

Create a `Secret` and project it as an env var on the API `Deployment`:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: attune-ldap
type: Opaque
stringData:
  search_bind_password: your-real-password
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: attune-api
spec:
  template:
    spec:
      containers:
      - name: api
        image: attune/api:latest
        env:
        - name: ATTUNE__SECURITY__LDAP__SEARCH_BIND_PASSWORD
          valueFrom:
            secretKeyRef:
              name: attune-ldap
              key: search_bind_password
```

External secret managers (HashiCorp Vault, AWS Secrets Manager, GCP Secret
Manager, Azure Key Vault) typically expose secrets as env vars or files via
sidecars/CSI drivers; in either case the goal is the same — populate
`ATTUNE__SECURITY__LDAP__SEARCH_BIND_PASSWORD` at process start.

## Startup heuristic warning

On startup the API logs a `WARN`-level message if
`security.ldap.search_bind_password` looks like a literal value committed to
config rather than an injected secret. The current heuristic flags values that:

- are an unresolved `${VAR}` placeholder,
- are shorter than 8 characters, or
- contain obvious placeholder substrings (`password`, `change-me`,
  `placeholder`, `example`, `secret`, `todo`, …).

The check is best-effort — high-entropy real passwords pass cleanly. The
warning never aborts startup; it is a nudge, not a gate. If you receive a
false positive, audit your secret source and (if it really is a strong,
operator-managed value) move it to an env var anyway so the warning goes
away and the value never lands in git.

## Future work

- **Key-table integration.** The Attune `key` table already stores
  encrypted values. A future enhancement may allow `search_bind_password`
  to be looked up from a key ref (e.g. `key:ldap_service_account`) so that
  rotation can happen via the API/UI without restarting the service.
- **Native `${VAR}` expansion in YAML.** The `config` crate does not
  currently expand `${VAR}` placeholders inside YAML strings. Until then,
  prefer the `ATTUNE__` env-var override path described above.
