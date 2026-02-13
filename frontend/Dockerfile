# ==============================================================
# Stage 1: Dev — used by docker-compose (target: dev)
# ==============================================================
FROM node:20-alpine AS dev

WORKDIR /app
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm install
COPY frontend/ .

EXPOSE 3000

CMD ["npm", "run", "dev"]

# ==============================================================
# Stage 2: Build — production asset compilation
# ==============================================================
FROM node:20-alpine AS build

WORKDIR /app
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm install
COPY frontend/ .
RUN npm run build

# ==============================================================
# Stage 3: Production — nginx serving static files + API proxy
# ==============================================================
FROM nginx:alpine AS production

# Defaults (override on Railway via env vars)
ENV API_URL=http://localhost:8080
ENV PORT=80

COPY --from=build /app/dist /usr/share/nginx/html
# Place template outside /etc/nginx/templates/ to prevent nginx's
# automatic envsubst (which replaces ALL $vars, clobbering $host etc.)
COPY frontend/nginx.conf.template /etc/nginx/nginx.conf.template

# Substitute only $API_URL and $PORT, then start nginx
CMD ["/bin/sh", "-c", "envsubst '${API_URL} ${PORT}' < /etc/nginx/nginx.conf.template > /etc/nginx/conf.d/default.conf && nginx -g 'daemon off;'"]

EXPOSE 80
