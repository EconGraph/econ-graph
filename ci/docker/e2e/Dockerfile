# E2E Test Container with Pre-installed Browsers
# This container has Playwright browsers pre-installed to speed up E2E test execution

FROM node:18-bullseye

# Install system dependencies for Playwright
RUN apt-get update && apt-get install -y \
    # Playwright system dependencies
    libnss3 \
    libnspr4 \
    libatk-bridge2.0-0 \
    libdrm2 \
    libxkbcommon0 \
    libgtk-3-0 \
    libgbm1 \
    libasound2 \
    # Additional dependencies for better browser support
    libxss1 \
    libgconf-2-4 \
    libxtst6 \
    libxrandr2 \
    libasound2 \
    libpangocairo-1.0-0 \
    libatk1.0-0 \
    libcairo-gobject2 \
    libgtk-3-0 \
    libgdk-pixbuf2.0-0 \
    # Clean up
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy package files
COPY frontend/package*.json ./

# Install npm dependencies
RUN npm ci

# Copy frontend source code
COPY frontend/ ./

# Build frontend (this is a significant setup step)
RUN npm run build

# Install dev-server dependencies (needed for some E2E tests)
WORKDIR /app/dev-server
COPY frontend/dev-server/package*.json ./
RUN npm ci
WORKDIR /app

# Create a non-root user for security
RUN groupadd -r playwright && useradd -r -g playwright -G audio,video playwright \
    && mkdir -p /home/playwright/Downloads \
    && chown -R playwright:playwright /home/playwright \
    && chown -R playwright:playwright /app

# Install Playwright system dependencies as root first
RUN npx playwright install-deps

# Switch to non-root user
USER playwright

# Set environment variables for Playwright
ENV PLAYWRIGHT_BROWSERS_PATH=/home/playwright/.cache/ms-playwright

# Install Playwright browsers as the playwright user (without --with-deps since deps are already installed)
RUN npx playwright install

# Install additional mobile browsers for mobile E2E tests
RUN npx playwright install chromium webkit

# Default command (can be overridden)
CMD ["npx", "playwright", "test", "--help"]
