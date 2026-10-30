# Multi-stage Dockerfile for EconGraph Frontend
FROM node:24-alpine AS builder

# Set working directory
WORKDIR /app

# Copy package files
COPY package*.json ./

# Install dependencies (including dev dependencies for build)
RUN npm ci

# Copy source code
COPY . .

# Set build-time environment variables for Vite
ARG VITE_API_URL
ARG VITE_GRAPHQL_URL
ARG VITE_WS_URL
# Keycloak issuer for sign-in. A path is resolved against the app's own origin at runtime
# (src/auth/oidcConfig.ts). An empty issuer hides sign-in, so the build fails on one unless
# VITE_OIDC_SIGN_IN_DISABLED=true says that is intended.
ARG VITE_OIDC_ISSUER=/idp/realms/econ-graph
ARG VITE_OIDC_SIGN_IN_DISABLED=false
ARG VITE_OIDC_CLIENT_ID
ARG NODE_ENV

# Build-time feature flags profile, `release` or `dev` (see docs/build-flags.md)
ARG FLAGS_PROFILE=release

# Set environment variables for build
ENV VITE_API_URL=$VITE_API_URL
ENV VITE_GRAPHQL_URL=$VITE_GRAPHQL_URL
ENV VITE_WS_URL=$VITE_WS_URL
ENV VITE_OIDC_ISSUER=$VITE_OIDC_ISSUER
ENV VITE_OIDC_CLIENT_ID=$VITE_OIDC_CLIENT_ID
ENV NODE_ENV=$NODE_ENV
ENV FLAGS_PROFILE=$FLAGS_PROFILE

# Build the application (Vite)
RUN if [ -z "$(echo "$VITE_OIDC_ISSUER" | tr -d '[:space:]')" ] && [ "$VITE_OIDC_SIGN_IN_DISABLED" != "true" ]; then \
      echo "VITE_OIDC_ISSUER is empty, which would hide sign-in; set it, or set VITE_OIDC_SIGN_IN_DISABLED=true to build without sign-in." >&2; \
      exit 1; \
    fi \
    && npm run build

# Production stage
FROM nginx:1.30-alpine

# Copy built application from builder stage (Vite outputs to dist)
COPY --from=builder /app/dist /usr/share/nginx/html

# Copy nginx configuration
COPY nginx.conf /etc/nginx/nginx.conf

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000 || exit 1

# Start nginx
CMD ["nginx", "-g", "daemon off;"]
