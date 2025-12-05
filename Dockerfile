# ======= BUILD IMAGE =======
FROM rust:1.91.1-trixie AS build
WORKDIR /usr/src/app

# Install packages for building native packages
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
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
FROM debian:trixie-slim

# Define build arguments
ARG NOTO_COLOR_EMOJI_VERSION=v2.051

# Install base dependencies first (including curl)
RUN apt-get update -qq && \
    apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
    curl \
    ca-certificates \
    tini \
    && rm -rf /var/lib/apt/lists/*

# Install fonts
RUN curl -o ./ttf-mscorefonts-installer_3.8.1_all.deb http://httpredir.debian.org/debian/pool/contrib/m/msttcorefonts/ttf-mscorefonts-installer_3.8.1_all.deb && \
    apt-get update -qq && \
    apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
    ./ttf-mscorefonts-installer_3.8.1_all.deb \
    culmus \
    fonts-beng \
    fonts-hosny-amiri \
    fonts-lklug-sinhala \
    fonts-lohit-guru \
    fonts-lohit-knda \
    fonts-samyak-gujr \
    fonts-samyak-mlym \
    fonts-samyak-taml \
    fonts-sarai \
    fonts-sil-abyssinica \
    fonts-sil-padauk \
    fonts-telu \
    fonts-thai-tlwg \
    ttf-wqy-zenhei \
    fonts-arphic-ukai \
    fonts-arphic-uming \
    fonts-ipafont-mincho \
    fonts-ipafont-gothic \
    fonts-unfonts-core \
    fonts-crosextra-caladea \
    fonts-crosextra-carlito \
    fonts-dejavu \
    fonts-liberation \
    fonts-liberation2 \
    fonts-linuxlibertine \
    fonts-noto-cjk \
    fonts-noto-core \
    fonts-noto-mono \
    fonts-noto-ui-core \
    fonts-sil-gentium \
    fonts-sil-gentium-basic && \
    rm -f ./ttf-mscorefonts-installer_3.8.1_all.deb && \
    mkdir -p /usr/local/share/fonts && \
    curl -Ls "https://github.com/googlefonts/noto-emoji/raw/$NOTO_COLOR_EMOJI_VERSION/fonts/NotoColorEmoji.ttf" -o /usr/local/share/fonts/NotoColorEmoji.ttf && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Install hyphenation packages
RUN apt-get update -qq && \
    apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
    hyphen-af hyphen-as hyphen-be hyphen-bg hyphen-bn hyphen-ca hyphen-cs hyphen-da hyphen-de hyphen-el \
    hyphen-en-gb hyphen-en-us hyphen-eo hyphen-es hyphen-fr hyphen-gl hyphen-gu hyphen-hi hyphen-hr hyphen-hu \
    hyphen-id hyphen-is hyphen-it hyphen-kn hyphen-lt hyphen-lv hyphen-ml hyphen-mn hyphen-mr hyphen-nl \
    hyphen-no hyphen-or hyphen-pa hyphen-pl hyphen-pt-br hyphen-pt-pt hyphen-ro hyphen-ru hyphen-sk hyphen-sl \
    hyphen-sr hyphen-sv hyphen-ta hyphen-te hyphen-th hyphen-uk hyphen-zu && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Install LibreOffice from backports
RUN echo "deb http://deb.debian.org/debian trixie-backports main" >> /etc/apt/sources.list && \
    apt-get update -qq && \
    apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends -t trixie-backports libreoffice && \
    libreoffice --version && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Set UTF-8 encoding (important for document conversion)
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# Create a non-root user and setup directories
RUN groupadd -g 1000 appuser && \
    useradd -d /home/appuser -s /bin/bash -u 1000 -g appuser appuser && \
    mkdir -p /home/appuser/.cache && \
    mkdir -p /home/appuser/.config && \
    chown -R appuser:appuser /home/appuser

ENV PORT=1234

# Switch to non-root user
USER 1000
WORKDIR /usr/src/app

# Copy the built binary from the build stage
COPY --from=build --chown=1000:1000 /usr/src/app/target/release/libreoffice-rest ./libreoffice-rest

# Expose the port
EXPOSE 1234

# Run the Rust server (use tini instead of dumb-init since we installed tini)
CMD ["tini", "--", "./libreoffice-rest"]
