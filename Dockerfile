# ======= BUILD IMAGE =======
FROM rust:1.80.0-slim-bookworm AS build
WORKDIR /usr/src/app

# Install packages for building native packages
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    libreoffice \
    libreoffice-dev \
    build-essential && \
    rm -rf /var/lib/apt/lists/*

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create library entry point for dependency caching
RUN mkdir -p src && echo "" > src/lib.rs

# Build dependencies only
RUN cargo build --release

# Copy source code
COPY src ./src
RUN touch src/lib.rs

# Build the application in release mode
RUN cargo build --release

# ===== PRODUCTION IMAGE =====
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y \
    libreoffice \
    fonts-dejavu-core \
    fonts-noto \
    dumb-init \
    wget \
    ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN groupadd -g 1000 appuser && \
    useradd -d /home/appuser -s /bin/bash -u 1000 -g appuser appuser

ENV PORT=1234

# Switch to non-root user
USER 1000
WORKDIR /usr/src/app

# Copy the built binary from the build stage
COPY --from=build --chown=1000:1000 /usr/src/app/target/release/libreoffice-rest ./libreoffice-rest

# Expose the port
EXPOSE 1234

# Run the Rust server
CMD ["dumb-init", "./libreoffice-rest"]