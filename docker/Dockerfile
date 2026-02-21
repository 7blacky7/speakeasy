# Speakeasy Server – Multi-Stage Docker Build
#
# Stage 1: Builder – kompiliert das Server-Binary mit Release-Optimierungen
# Stage 2: Runtime – minimales Debian-Image mit nur dem fertigen Binary

# =============================================================================
# Stage 1: Builder
# =============================================================================
FROM rust:1.88-bookworm AS builder

WORKDIR /build

# System-Abhängigkeiten für Audio (cpal/ALSA) und Protobuf (tonic-build)
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libasound2-dev \
        libopus-dev \
        pkg-config \
        protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Abhängigkeiten zuerst cachen (Layer-Caching optimieren)
# Dummy-Sources erstellen damit cargo fetch die Dependencies lädt
COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml server/
COPY crates/core/Cargo.toml crates/core/
COPY crates/protocol/Cargo.toml crates/protocol/
COPY crates/db/Cargo.toml crates/db/
COPY crates/auth/Cargo.toml crates/auth/
COPY crates/voice/Cargo.toml crates/voice/
COPY crates/signaling/Cargo.toml crates/signaling/
COPY crates/audio/Cargo.toml crates/audio/
COPY crates/commander/Cargo.toml crates/commander/
COPY crates/chat/Cargo.toml crates/chat/
COPY crates/plugin/Cargo.toml crates/plugin/
COPY crates/crypto/Cargo.toml crates/crypto/
COPY crates/observability/Cargo.toml crates/observability/

# Proto-Dateien kopieren (benoetigt fuer commander build.rs / tonic-build)
COPY proto/ proto/

# Dummy-Sourcen für alle Crates anlegen (für Dependency-Caching)
RUN mkdir -p server/src && echo 'fn main() {}' > server/src/main.rs && \
    for crate in core protocol db auth voice signaling audio commander chat plugin crypto observability; do \
      mkdir -p crates/$crate/src && echo '' > crates/$crate/src/lib.rs; \
    done

# build.rs für commander kopieren (tonic-build braucht es beim fetch/build)
COPY crates/commander/build.rs crates/commander/build.rs

RUN cargo fetch

# Eigentlicher Quellcode kopieren und kompilieren
COPY . .

RUN cargo build --release --bin speakeasy-server

# =============================================================================
# Stage 2: Runtime
# =============================================================================
FROM debian:bookworm-slim

# Notwendige Laufzeit-Bibliotheken installieren
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        curl \
    && rm -rf /var/lib/apt/lists/*

# Nicht-root Benutzer anlegen
RUN groupadd --gid 1000 speakeasy \
    && useradd --uid 1000 --gid speakeasy --shell /bin/sh --create-home speakeasy

# Binary und Standardkonfiguration kopieren
COPY --from=builder /build/target/release/speakeasy-server /usr/local/bin/speakeasy-server
COPY --from=builder /build/server/config.example.toml /etc/speakeasy/config.toml

# Dateirechte setzen
RUN chown -R speakeasy:speakeasy /etc/speakeasy

# Daten-Volume für persistente Speicherung
VOLUME ["/data"]
RUN chown speakeasy:speakeasy /data 2>/dev/null || true

# Ports freigeben:
# 9987/udp  – Voice-Daten (UDP)
# 30033/tcp – File Transfer
# 10011/tcp – ServerQuery TLS
# 8080/tcp  – REST/gRPC API
# 9300/tcp  – Observability: Metriken (Prometheus)
# 9301/tcp  – Observability: Health-Check
EXPOSE 9987/udp 30033/tcp 10011/tcp 8080/tcp 9300/tcp 9301/tcp

# Umgebungsvariablen
ENV SPEAKEASY_CONFIG=/etc/speakeasy/config.toml
ENV SPEAKEASY_DATA_DIR=/data
ENV SE_LOG_FORMAT=json
ENV SE_LOG_LEVEL=info

USER speakeasy

ENTRYPOINT ["speakeasy-server"]
