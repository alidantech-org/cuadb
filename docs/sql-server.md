# SQL Server feasibility review (Group 13)

Status: **GO for the driver, NO-GO for any adapter claim until a disposable SQL Server validates it.** No SQL Server instance exists in this environment, so no catalog, paging or type behavior below has been run against a server.

## Driver evaluation (crates.io data fetched 2026-11-04)

| Crate | Licence | Latest / updated | Notes |
|---|---|---|---|
| **tiberius 0.13.0** | MIT OR Apache-2.0 | 2026-09-25 (prior release 2024-07), 5.8 M downloads, ~959 k recent | Pure-Rust TDS client by Prisma. Features: `rustls` / `native-tls` / `vendored-openssl`, `tds73`/`tds80`, `integrated-auth-gssapi`, `sspi-rs`, `bigdecimal`. Tokio via `tokio-util` compat. No built-in pool (`bb8-tiberius`/`deadpool-tiberius` exist). MSRV 1.88 |
| mssql-client 0.20.2 | MIT OR Apache-2.0 | created 2025-12, ~7 k downloads | Too new and little used for a security-sensitive adapter |
| odbc-api 29.3.0 | MIT | updated 2026-11-01, 1.8 M downloads | Mature, but needs the Microsoft ODBC driver in the runtime image (non-redistributable licence, system libraries) — conflicts with the single small container |
| SQLx | — | — | SQLx 0.8 dropped its MSSQL driver; not an option |

**Recommendation:** tiberius 0.13 with `default-features = false, features = ["rustls", "tds73"]`. A scratch crate (outside the repository) with exactly those features and `tokio`/`tokio-util` compiles cleanly in ~53 s (114 transitive packages) with the repository's Rust toolchain. Not yet checked: duplicate-crate/binary-size impact next to SQLx, `cargo deny`/audit results.

## TLS and authentication
- Encryption: `EncryptionLevel::Required` by default; trust anchors via `rustls` (webpki roots feature available). `TrustServerCertificate` must remain an explicit, documented opt-in for lab servers. Development containers use self-signed certificates, so tests need that opt-in.
- Supported: SQL authentication (login + password). Out of scope for v1: Windows integrated auth (needs GSSAPI/SSPI libraries in the image), Azure AD tokens. Unsupported modes must produce a clear configuration error, never a silent fallback.
- URL form proposed: `sqlserver://login:password@host:1433/database?encrypt=required|off&trust-server-certificate=false`, validated like the other engines (loopback may omit TLS; remote requires a password and encryption). Credentials stay server-side (profiles file).

## Capability differences to design for
- Catalog: `sys.schemas`, `sys.tables`, `sys.views`, `sys.columns`, `sys.indexes`/`sys.index_columns`, `sys.foreign_keys`; visibility via `HAS_PERMS_BY_NAME` / metadata visibility rules (objects the login cannot see do not appear).
- Identifiers are quoted with `[` `]` (`]` doubled) and bound through `sp_executesql` or parameters; never string-built from browser input.
- Paging: `ORDER BY ... OFFSET n ROWS FETCH NEXT m ROWS ONLY` requires an `ORDER BY`; use the primary key as a stable tie-break. Offset paging only at first (same 100×100 cap).
- Types needing explicit handling: `uniqueidentifier`, `datetimeoffset`, `money`, `decimal` (use `bigdecimal` or text), `varbinary(max)`, `xml`, `sql_variant`, `geography`/`geometry` (show as unsupported/WKT), `rowversion`.
- Collation: case sensitivity depends on the database collation, so name lookups must use the catalog's exact names.
- Read-only: use a SELECT-only login; `ApplicationIntent=ReadOnly` helps only with availability groups. A query timeout is client-side plus server `SET LOCK_TIMEOUT`; there is no `statement_timeout` equivalent.

## Packaging and runtime
- No extra system packages with `rustls`. Binary size and compile time increase (~50 s extra clean build).
- A second async TLS stack alongside SQLx's `tls-rustls-ring-webpki` is possible; verify with `cargo tree -d` when integrating.

## Plan and blockers
1. (Needs your OK) Pull `mcr.microsoft.com/mssql/server:2022-latest` (~1.6 GB) and run it on a loopback port with `ACCEPT_EULA=Y` — accepting Microsoft's EULA is your decision, not mine.
2. Add `src/db/sqlserver/` behind a Cargo feature, implementing `DatabaseAdapter` (catalog, offset paging, typed cells) with fixtures under `tests/fixtures/`.
3. Prove with `tests/adapter_contract.rs` (add a fifth subject) plus a restricted-login test, timeouts, and a Docker run.
4. Only then document SQL Server as supported; until then the README and release matrix list it as **not supported**.

Decision recorded: proceed with tiberius 0.13 only after step 1 is approved; do not ship a SQL Server claim on the strength of this review.
