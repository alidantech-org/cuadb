# Docker, networking and deployment

## Run the published image

```bash
docker run --rm -p 127.0.0.1:4848:4848 -e CUADB_LOCAL_MODE=true -v cuadb-data:/data \
  ghcr.io/alidantech-org/cuadb:latest
```

Tags: `latest`, `X.Y.Z`, `X.Y`, `X`. Platforms: `linux/amd64`, `linux/arm64`. The image runs as an unprivileged user, keeps all state in `/data`, and has a health check.

`CUADB_LOCAL_MODE=true` is needed in a container because the process listens on `0.0.0.0` inside it, which would otherwise select permissions mode. Always publish the port on `127.0.0.1` when local mode is on, so only your own computer can reach it.

## Can the container reach my databases?

A container is a small private network of its own. Outbound traffic works like any program on your computer, but `localhost` inside the container means the container itself.

| Target | What to use |
|---|---|
| Database on a **remote host, your router/LAN, VPN or cloud** | Its normal address (`192.168.1.20`, `db.example.com`). Works from the default network. |
| Database on **your own computer** (Docker Desktop on macOS/Windows) | `host.docker.internal` as the host. |
| Database on **your own computer** (Linux) | Start with `--add-host=host.docker.internal:host-gateway` and use `host.docker.internal`. The database must listen on more than `127.0.0.1` (for PostgreSQL, `listen_addresses` and `pg_hba.conf`), and connections to any non-loopback host require a password. |
| Database in **another container** | Put both on one Docker network and use the container name. |
| **Everything the computer sees, with `localhost` meaning your computer** | Host networking. |

### Host networking

```bash
docker run --rm --network host -e CUADB_LOCAL_MODE=true -e CUADB_HOST=127.0.0.1 \
  -v cuadb-data:/data ghcr.io/alidantech-org/cuadb:latest
```

The container then shares your computer's network: `localhost:5432` really is your local PostgreSQL, LAN and router addresses work, and no port publishing is needed (open `http://localhost:4848`). `CUADB_HOST=127.0.0.1` keeps the UI itself private to your computer.

- On **Linux** this works out of the box.
- On **Docker Desktop for macOS/Windows** host networking is supported only in recent versions (enable it in *Settings → Resources → Network*), and it shares the Docker VM's network on some setups. If it does not behave, use `host.docker.internal` instead.
- Compose: `docker compose -f docker-compose.host.yml up`.

### Safety

Local mode gives full access to every database CuaDB can reach, with no sign-in. With host networking, do not bind to `0.0.0.0` in local mode, because the UI would then be reachable from your LAN. To expose CuaDB on a network use permissions mode (`CUADB_LOCAL_MODE=false`), create an administrator, and terminate HTTPS in a reverse proxy:

```bash
docker run -d -p 4848:4848 -e CUADB_LOCAL_MODE=false -v cuadb-data:/data ghcr.io/alidantech-org/cuadb:latest
docker exec -i <container> sh -c 'cuadb admin bootstrap --login admin --password-stdin' <<< 'a-long-password'
```

## SQLite files

Mount a folder and the files appear in the SQLite connection picker:

```bash
docker run ... -v "$PWD/databases:/data/sqlite" ghcr.io/alidantech-org/cuadb:latest
```

## Compose

[`docker-compose.yml`](../docker-compose.yml) (published port, loopback only) and [`docker-compose.host.yml`](../docker-compose.host.yml) (host networking).

## Build locally

```bash
docker build -t cuadb .
docker run --rm -p 127.0.0.1:4848:4848 -e CUADB_LOCAL_MODE=true cuadb
```
