FROM python:3.12-slim

ENV PYTHONUNBUFFERED=1 \
    LUMA_FORGE_PROVISIONER_HOST=0.0.0.0 \
    LUMA_FORGE_PROVISIONER_PORT=8000 \
    LUMA_FORGE_WORKSPACE_MOUNT_PATH=/workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY pyproject.toml ./
COPY src ./src

ENV PYTHONPATH=/app/src

EXPOSE 8000
CMD ["python", "-m", "provisioner_worker"]

