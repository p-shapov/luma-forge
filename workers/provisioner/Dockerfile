FROM python:3.12-slim

ENV PYTHONUNBUFFERED=1

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY workers/provisioner/pyproject.toml ./
COPY workers/provisioner/src ./src

RUN python -m pip install --no-cache-dir .

CMD ["python", "-m", "app"]
