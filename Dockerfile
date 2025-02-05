# ======= BUILD IMAGE =======
FROM node:22.13.1-alpine AS build
WORKDIR /usr/src/app

# Install packages for building native packages
RUN apk update
RUN apk add --no-cache make gcc g++ python3 libressl-dev
RUN rm -rf /var/cache/apk/*

# pnpm install/build
RUN npm install -g pnpm

# Copy files...
COPY . .

# Cleanup in case of local run
RUN rm -rf node_modules dist server-dist .env

RUN pnpm install --frozen-lockfile
RUN pnpm run build:server

# ===== PRODUCTION IMAGE =====
FROM node:22.13.1-alpine

RUN apk update
RUN apk add --no-cache \
    make \
    gcc \
    g++ \
    libressl-dev \
    dumb-init \
    libreoffice \
    libreoffice-writer \
    python3 \
    python3-dev \
    musl-dev \
    py3-pip \
    py3-setuptools \
    py3-wheel

# Create and use a virtual environment for Python packages
RUN python3 -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Now install unoserver in the virtual environment
RUN pip3 install unoserver

ENV NODE_ENV production
ENV PORT 8080

# Node user is 1000:1000
USER 1000
WORKDIR /usr/src/app

# Copy local code to the container image
COPY --from=build --chown=1000:1000 /usr/src/app /usr/src/app

# Expose a port
EXPOSE 8080

CMD ["dumb-init", "node", "--max-old-space-size=1024", "--enable-source-maps", "./dist/bundle.prod.js"]