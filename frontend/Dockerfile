# Multi-stage build for the RRE admin frontend (Next.js App Router).
#
# Build context is the frontend/ directory:
#   docker build -f frontend/Dockerfile -t rre-frontend frontend
#
# Base image pins the major version only (never :latest).
#
# NEXT_PUBLIC_API_BASE is inlined into the client bundle at build time, so it is
# a build ARG (not a runtime env). Override per environment via --build-arg.
FROM node:20-alpine AS deps
WORKDIR /app

# Install dependencies (lockfile-aware: `npm ci` when a lockfile exists for
# reproducible installs, otherwise fall back to `npm install`).
COPY package.json package-lock.json* ./
RUN if [ -f package-lock.json ]; then npm ci; else npm install; fi

# ----------------------------------------------------------------------------
FROM node:20-alpine AS builder
WORKDIR /app

ARG NEXT_PUBLIC_API_BASE=http://localhost:8000
ENV NEXT_PUBLIC_API_BASE=${NEXT_PUBLIC_API_BASE}
ENV NEXT_TELEMETRY_DISABLED=1

COPY --from=deps /app/node_modules ./node_modules
COPY . .
# Ensure a public/ dir exists even if the repo ships none (Next serves it).
RUN mkdir -p public && npm run build

# ----------------------------------------------------------------------------
FROM node:20-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

# wget (busybox) is used for the healthcheck; create a non-root user.
RUN addgroup -g 10003 -S rre && adduser -u 10003 -S rre -G rre

# Production runtime needs the build output + node_modules + manifest.
COPY --from=builder /app/package.json ./package.json
COPY --from=builder /app/next.config.mjs ./next.config.mjs
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/.next ./.next
# `public` is created in the builder stage (guaranteed present) below.
COPY --from=builder /app/public ./public

EXPOSE 3000
ENV PORT=3000
ENV HOSTNAME=0.0.0.0

USER rre

HEALTHCHECK --interval=10s --timeout=3s --start-period=20s --retries=5 \
    CMD wget -q -O /dev/null http://localhost:3000/ || exit 1

CMD ["npm", "run", "start"]
