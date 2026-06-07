# SupaZip Docker image

Minimal distroless image containing the `supazip` CLI.

## Build

```bash
docker build -f packaging/docker/Dockerfile -t supazip .
```

## Run

```bash
# List contents of an archive
docker run --rm -v /path/to/archives:/archives supazip list /archives/test.zip

# Extract to a volume
docker run --rm -v /path/to/archives:/archives -v /tmp/out:/out supazip extract /archives/test.zip -o /out
```

## Multi-arch

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -f packaging/docker/Dockerfile \
  -t supazip:latest \
  --push .
```
