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

# Define build arguments
ARG GIT_SHA
ARG RELEASE_TAG

# Set environment variables using the build arguments
ENV GIT_SHA=$GITHUB_SHA
ENV RELEASE_TAG=$RELEASE_TAG

RUN apk update
RUN apk add --no-cache make gcc g++ python3 libressl-dev dumb-init libreoffice-writer

ENV NODE_ENV production
ENV PORT 8080
ENV INTERNAL_PORT 8081

# Node user is 1000:1000
USER 1000
WORKDIR /usr/src/app

# Copy local code to the container image
COPY --from=build --chown=1000:1000 /usr/src/app /usr/src/app

# Expose a port
EXPOSE 8080
EXPOSE 8081

CMD ["dumb-init", "node", "--max-old-space-size=1024", "--enable-source-maps", "./server-dist/bundle.prod.js"]
