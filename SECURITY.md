# Security policy

## Reporting a vulnerability

Please report security problems privately through GitHub: **Security → Report a vulnerability** on the repository. Do not open a public issue. We aim to acknowledge reports within 3 working days and to ship a fix or mitigation for confirmed issues as quickly as is responsible.

## Supported versions

The latest released minor version receives fixes.

## Deployment guidance

- **Trusted local mode** gives full access without sign-in and is meant for your own computer. It is on by default only when CuaDB listens on a loopback address. Never expose it to a network. In Docker publish the port as `127.0.0.1:4848:4848`.
- For shared or internet-facing use run **permissions mode** (`CUADB_LOCAL_MODE=false`), create an administrator (`cuadb admin bootstrap`), put CuaDB behind HTTPS (reverse proxy) and use database roles with the least privilege needed.
- Saved connection passwords are encrypted with AES-256-GCM. Set `CUADB_SECRET_KEY` and back it up; keep the data directory private.
- AI features are optional. API keys stay in your browser; table and column names are sent to the provider you choose unless you turn that off; row data is sent only if you enable data-aware mode.

The design and residual risks are described in [docs/threat-model.md](docs/threat-model.md).
