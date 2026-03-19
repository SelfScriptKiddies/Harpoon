FROM node:22-alpine AS build

WORKDIR /app

# Install deps
COPY crates/harpoon-web/package.json crates/harpoon-web/package-lock.json* ./
RUN npm install

# Copy source
COPY crates/harpoon-web/ ./

# Build — output to /app/dist
RUN npx vite build --outDir /app/dist
