# Docker Deployment

This project can run as a Docker container while using an already prepared
PostgreSQL database. The Docker setup does not create, migrate, or modify the
database schema automatically.

## Files

- `Dockerfile`: small Ubuntu 24.04 runtime image. It expects
  `target/release/dogn3` to be built on the host before `docker build`.
- `docker-compose.yml`: local deployment with the application and Redis.
- `.env.docker.example`: commented Docker environment template.

## Prerequisites

- Docker Engine with the Compose plugin.
- A PostgreSQL database that already has the current `dogn3` schema and data.
- Network access from the application container to PostgreSQL.
- A host directory containing post images, if local images should be served.

## Configuration

Create the Docker environment file:

```bash
cp .env.docker.example .env.docker
```

Edit `.env.docker` before starting the stack:

- Set `DATABASE_URL` to the real PostgreSQL connection string.
- Keep `BIND_ADDR=0.0.0.0:3000` inside Docker.
- Keep `REDIS_URL=redis://redis:6379` when using the Redis service from
  `docker-compose.yml`.
- Set `SITE_NAME` to the public site name.
- Set `SESSION_COOKIE_SECURE=true` when the site is served through HTTPS.
- Keep `PASSWORD_RESET_ENABLED=false` unless a sendmail-compatible command is
  available inside the container.

The Compose file maps `./data/images` on the host to `/app/images` in the
container by default. To use an existing image directory, set `DOGN_IMAGE_DIR`
when running Compose:

```bash
DOGN_IMAGE_DIR=/home/wy/pic/dogn_pic/pic docker compose up -d --no-build
```

Keep `IMAGE_DIRECTORY=/app/images` in `.env.docker`.

## PostgreSQL On The Docker Host

The example `DATABASE_URL` uses `host.docker.internal`. On Linux, the Compose
file maps that name to the Docker host through:

```yaml
extra_hosts:
  - "host.docker.internal:host-gateway"
```

PostgreSQL must listen on an address reachable from Docker, and its
authentication rules must allow the Docker bridge network. A local Unix-socket
connection such as `postgres:///dogn` will not work from inside the container.

## Manual Image Workflow

Use this workflow when the deployment host should not build the image or pull
the application image from a remote registry. The image is built on another
machine, exported as a tar archive, copied to the deployment host, imported,
and then started by Compose.

This avoids network access for the application image on the deployment host.
The deployment host still needs a Redis image available locally if the Compose
Redis service is used. Either export/import `redis:7-alpine` too, use an
already installed external Redis service, or disable Redis-backed features for a
temporary development deployment.

### 1. Build On A Build Machine

Run this on a machine that has Rust installed. Cargo uses the host Cargo cache,
so repeated builds do not re-download crates inside Docker:

```bash
cargo build --release --bin dogn3
docker build -t dogn3:local .
```

The Dockerfile is runtime-only. It copies the already built binary from:

```text
target/release/dogn3
```

The runtime image must have a glibc version that is at least as new as the
build host used for `cargo build`. For Ubuntu 24.04 build hosts, the binary may
require `GLIBC_2.39`, so the runtime image uses `ubuntu:24.04`. If the runtime
image is older, such as Debian bookworm, startup can fail with:

```text
dogn3: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.39' not found
```

If a smaller or older runtime image is required, build the binary on an older
compatible Linux distribution, or use a fully static `musl` build after testing
TLS, image processing, and runtime behavior carefully.

Do not remove the Docker image before rebuilding unless disk cleanup is
required. `docker build -t dogn3:local .` updates the tag and can reuse cached
runtime layers.

Optionally add a versioned tag for traceability:

```bash
docker tag dogn3:local dogn3:2026-06-03
```

Inspect the image:

```bash
docker image ls dogn3
docker image inspect dogn3:local --format '{{.Id}} {{.Created}}'
```

### 2. Export The Application Image

Export the image to a tar archive:

```bash
docker save dogn3:local -o dogn3-image.tar
```

Compress it if the transfer path benefits from a smaller file:

```bash
gzip -9 dogn3-image.tar
```

If the deployment host will also run the Compose Redis service and cannot pull
from Docker Hub, export Redis too:

```bash
docker pull redis:7-alpine
docker save redis:7-alpine -o redis-7-alpine.tar
gzip -9 redis-7-alpine.tar
```

### 3. Transfer Archives And Deployment Files

Copy these files to the deployment host:

- `dogn3-image.tar` or `dogn3-image.tar.gz`.
- `redis-7-alpine.tar` or `redis-7-alpine.tar.gz`, if Redis is not already
  available locally.
- `docker-compose.yml`.
- `.env.docker`, created from `.env.docker.example` and edited for that host.
- The local image directory mounted to `/app/images`, if post images are used.

The deployment host does not need the source tree when the image has already
been built. It only needs the Compose file, environment file, image archives,
and mounted runtime data.

### 4. Import Images On The Deployment Host

Load the application image:

```bash
gunzip -c dogn3-image.tar.gz | docker load
```

If the archive is not compressed:

```bash
docker load -i dogn3-image.tar
```

The loaded image must have the same tag used by `docker-compose.yml`:

```yaml
image: dogn3:local
```

If the archive was loaded under a different tag, retag it:

```bash
docker tag dogn3:2026-06-03 dogn3:local
```

Load Redis too when needed:

```bash
gunzip -c redis-7-alpine.tar.gz | docker load
```

Confirm that all required images are local:

```bash
docker image ls dogn3
docker image ls redis
```

### 5. Prepare Runtime Configuration

Create `.env.docker` on the deployment host:

```bash
cp .env.docker.example .env.docker
```

Edit it for the deployment host:

- `DATABASE_URL` must point to the prepared PostgreSQL database.
- `BIND_ADDR` should remain `0.0.0.0:3000`.
- `IMAGE_DIRECTORY` should remain `/app/images` when using the Compose volume.
- `REDIS_URL` should be `redis://redis:6379` when using the Compose Redis
  service, or point to the external Redis host if Redis runs elsewhere.
- `CACHE_ENABLED=false` can be used only if cache is intentionally disabled.
- `RATE_LIMIT_BACKEND=memory` is acceptable only for development; production
  retry limits should use Redis.

Ensure the host image directory exists before starting. The default Compose
mapping uses `./data/images`:

```bash
mkdir -p data/images
```

To use a different host image directory, set `DOGN_IMAGE_DIR` when running
Compose instead of editing `docker-compose.yml`:

```bash
DOGN_IMAGE_DIR=/home/wy/pic/dogn_pic/pic docker compose up -d --no-build
```

The container path remains `/app/images`, so keep
`IMAGE_DIRECTORY=/app/images` in the app env file.

### 6. Start Without Building Or Pulling The App Image

Start the stack using the already imported image:

```bash
docker compose up -d --no-build
```

To choose the app env file, image directory, and published host port at
deployment time:

```bash
DOGN_ENV_FILE=/etc/dogn3/dogn3.env \
DOGN_IMAGE_DIR=/home/wy/pic/dogn_pic/pic \
DOGN_HTTP_PORT=3000 \
docker compose up -d --no-build
```

If the host has the newer Compose plugin, this can additionally prevent pulls:

```bash
docker compose up -d --no-build --pull never
```

With the legacy Compose binary:

```bash
docker-compose up -d --no-build
```

The same deployment variables work with legacy Compose:

```bash
DOGN_ENV_FILE=/etc/dogn3/dogn3.env \
DOGN_IMAGE_DIR=/home/wy/pic/dogn_pic/pic \
DOGN_HTTP_PORT=3000 \
docker-compose up -d --no-build
```

If Compose reports that `dogn3:local` is missing, the application image was not
loaded or was loaded under a different tag.

### Start With Docker Run

Compose is recommended because it also starts Redis, but an imported image can
be started directly:

```bash
docker run -d \
  --name dogn3 \
  --restart unless-stopped \
  --env-file /etc/dogn3/dogn3.env \
  -p 3000:3000 \
  -v /home/wy/pic/dogn_pic/pic:/app/images \
  --add-host host.docker.internal:host-gateway \
  dogn3:local
```

The env file used by `--env-file` must include `BIND_ADDR=0.0.0.0:3000` and
`IMAGE_DIRECTORY=/app/images`. The mounted host image directory must be
writable by the container user, currently uid/gid `999:999`; otherwise image
uploads fail with `image_storage_unavailable`. If Redis runs in another
container without Compose networking, set `REDIS_URL` to an address reachable
from this container or run with `--network` configured for that Redis
container.

### Reusing The Standalone `.env`

The standalone `.env` used by `./scripts/server.sh` can be reused, but be
careful with host-local values. A file that works for the host process often
contains values like:

```env
DATABASE_URL=postgres://USER:PASSWORD@localhost:5432/dogn
BIND_ADDR=127.0.0.1:3000
IMAGE_DIRECTORY=/home/wy/pic/dogn_pic/pic
REDIS_URL=redis://127.0.0.1:6379
```

Inside a normal Docker bridge-network container:

- `localhost` and `127.0.0.1` mean the container itself, not the Docker host.
- `BIND_ADDR=127.0.0.1:3000` binds only to the container loopback interface.
- Host paths such as `/home/wy/pic/dogn_pic/pic` do not exist unless mounted at
  the same path.

If the container stays in `health: starting` and logs show:

```text
Error: pool timed out while waiting for an open connection
```

the application is usually unable to connect to PostgreSQL. With bridge
networking, keep the standalone `.env` unchanged and override only Docker-only
values in the `docker run` command:

```bash
docker rm -f dogn3

docker run -d \
  --name dogn3 \
  --restart unless-stopped \
  --env-file /home/wy/dogn3/.env \
  -e BIND_ADDR=0.0.0.0:3000 \
  -e DATABASE_URL='postgres://USER:PASSWORD@host.docker.internal:5432/dogn' \
  -e REDIS_URL='redis://host.docker.internal:6379' \
  -e IMAGE_DIRECTORY=/app/images \
  -p 3000:3000 \
  -v /home/wy/pic/dogn_pic/pic:/app/images \
  --add-host host.docker.internal:host-gateway \
  dogn3:local
```

`--add-host host.docker.internal:host-gateway` makes
`host.docker.internal` resolve to the Docker host from inside the container.

An alternative is host networking, which is closer to the standalone process
and allows the existing `localhost` database and Redis URLs to keep working:

```bash
docker rm -f dogn3

docker run -d \
  --name dogn3 \
  --restart unless-stopped \
  --network host \
  --env-file /home/wy/dogn3/.env \
  -v /home/wy/pic/dogn_pic:/home/wy/pic/dogn_pic \
  dogn3:local
```

With host networking, Docker does not use `-p`; the application binds directly
to the host network according to `BIND_ADDR` in the env file.

For host networking with the standalone `.env`, `IMAGE_DIRECTORY` may remain a
host path such as `/home/wy/pic/dogn_pic/pic`, but the bind mount still has to
make that path writable for uid/gid `999:999`. One deployment option is:

```bash
sudo chown -R 999:999 /home/wy/pic/dogn_pic/pic
```

Recent application versions print more detailed startup diagnostics before
connecting to external services. The logs include redacted endpoints, for
example:

```text
loaded runtime configuration bind_addr=127.0.0.1:3000 image_directory=/home/wy/pic/dogn_pic/pic ...
connecting to PostgreSQL database_url=postgres://wy:***@localhost:5432/dogn
failed to connect to PostgreSQL at postgres://wy:***@localhost:5432/dogn; configured host is localhost/127.0.0.1 ...
```

If the URL contains `localhost` or `127.0.0.1` while the app runs in Docker
bridge networking, treat that as a deployment configuration problem rather
than a database schema problem.

### 7. Upgrade Manually

For each new release:

1. Build the release binary and tag the new image on the build machine.
2. Export and transfer the new archive.
3. Load it on the deployment host.
4. Restart the app container:

```bash
docker compose up -d --no-build app
```

or with legacy Compose:

```bash
docker-compose up -d --no-build app
```

If the tag remains `dogn3:local`, Compose will recreate the app container using
the newly loaded image when the image ID changes.

## Connected Build And Start

Build the release binary, then build and start the application:

```bash
cargo build --release --bin dogn3
docker compose up -d --build
```

If the host has the legacy Compose binary instead of the Docker Compose plugin,
use:

```bash
cargo build --release --bin dogn3
docker-compose up -d --build
```

Check status:

```bash
docker compose ps
docker compose logs -f app
```

With the legacy binary, use `docker-compose ps` and `docker-compose logs -f app`.

The default published URL is:

```text
http://127.0.0.1:3000
```

## Stop

Stop the stack:

```bash
docker compose down
```

With the legacy binary, use `docker-compose down`.

Redis data is stored in the named Docker volume `redis_data`. To remove that
volume too:

```bash
docker compose down -v
```

Do not use `-v` unless losing Redis cache and rate-limit state is acceptable.

## Health Check

The runtime image uses `/api/health` as its container health check. This checks:

- PostgreSQL connectivity.
- Redis connectivity when cache/rate limiting uses Redis.

If the container is unhealthy, inspect logs with:

```bash
docker compose logs app
```
