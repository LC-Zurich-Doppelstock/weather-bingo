# ==============================================================
# Stage 1: Dev — used by docker-compose (target: dev)
# ==============================================================
FROM node:20-alpine AS dev

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm install
COPY . .

EXPOSE 3000

CMD ["npm", "run", "dev"]

# ==============================================================
# Stage 2: Build — production asset compilation
# ==============================================================
FROM node:20-alpine AS build

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm install
COPY . .
RUN npm run build

# ==============================================================
# Stage 3: Production — nginx serving static files + API proxy
# ==============================================================
FROM nginx:alpine AS production

COPY --from=build /app/dist /usr/share/nginx/html
COPY nginx.conf.template /etc/nginx/templates/default.conf.template

EXPOSE 80
