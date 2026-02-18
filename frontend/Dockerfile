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

# Make nginx config directory writable by the nginx user so envsubst
# can write the final config before nginx starts.
RUN chown -R nginx:nginx /etc/nginx/conf.d /var/cache/nginx /var/log/nginx \
    && touch /var/run/nginx.pid && chown nginx:nginx /var/run/nginx.pid

# Substitute $API_URL, $PORT, and $NAMESERVER, then start nginx as non-root.
# NAMESERVER is extracted from the container's /etc/resolv.conf.
# IPv6 addresses are wrapped in brackets for nginx compatibility.
# Note: the CMD runs as root initially for envsubst, then exec's nginx
# which drops to the nginx user via the config. We switch USER after
# the dirs are prepared so the CMD itself runs as nginx.
USER nginx

CMD ["/bin/sh", "-c", "export NAMESERVER=$(awk '/^nameserver/{ip=$2; if(index(ip,\":\")){ip=\"[\"ip\"]\"}; printf \"%s \",ip}' /etc/resolv.conf) && envsubst '${API_URL} ${PORT} ${NAMESERVER}' < /etc/nginx/nginx.conf.template > /etc/nginx/conf.d/default.conf && nginx -g 'daemon off;'"]

EXPOSE 80
