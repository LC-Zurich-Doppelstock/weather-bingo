# Weather Bingo 🎿🌦️

Race-day weather forecasting for cross-country skiing — visualise conditions along the course before you start.

## What It Does

Weather Bingo helps skiers prepare for long-distance races by showing detailed weather forecasts along the race course. Select a race, set your target finish time, and see what conditions to expect at each checkpoint when *you* pass through.

- **Interactive map** with the race course and checkpoint markers
- **Checkpoint detail** — tap a checkpoint for temperature, wind, precipitation, humidity, and a mini-timeline showing conditions before/after your expected pass-through
- **Course overview** — compact graphs of weather along the entire course (temperature, wind, precipitation by km)
- **Forecast history** — see how predictions have evolved over time, crucial for reading snow conditions
- **Uncertainty ranges** — percentile bands so you know how confident the forecast is

## Architecture

```
Frontend (React/TS)  →  REST API (Rust/Axum)  →  PostgreSQL
                                ↕
                          yr.no (MET Norway)
```

| Component | Tech | Role |
|-----------|------|------|
| Frontend | React, TypeScript, Vite, Leaflet, Recharts, Tailwind | Map, charts, UI |
| API | Rust, Axum, sqlx | REST endpoints, forecast caching, yr.no integration |
| Database | PostgreSQL | Races, checkpoints, forecast history |
| Weather source | [yr.no Locationforecast 2.0](https://api.met.no/weatherapi/locationforecast/2.0/) | Forecast data |

## Key Concepts

**Cache-first forecasts** — the API serves from the database. If data is missing or stale (>1 min), it fetches fresh data from yr.no and stores it. Old forecasts are never overwritten — every fetch creates a new historical record.

**Pacing-aware** — forecasts are calculated for when *you* will be at each point, not just a fixed time. Set a target duration and the app computes your expected pass-through time at each checkpoint using even pacing.

**Race-agnostic** — the data model supports multiple races. Vasaloppet 2026 is the first, but adding more is just data.

## Races

Currently: **Vasaloppet 2026** (90 km, Berga/Sälen → Mora, March 1st 08:00 CET)

Race course and checkpoint data stored in `data/vasaloppet-2026.gpx` using a custom `wb:` XML namespace for race metadata. The GPX file is the single source of truth -- the API parses it at startup and upserts races + checkpoints into the database (idempotent).

## Getting Started

```bash
docker compose up
```

| Service | URL |
|---------|-----|
| Frontend | http://localhost:3000 |
| API | http://localhost:8080 |
| Swagger UI | http://localhost:8080/swagger-ui/ |
| PostgreSQL | localhost:5431 |

## Project Structure

```
weather-bingo/
├── api/              # Rust REST API
├── frontend/         # React + TypeScript
├── data/             # Race GPX files & seed data
├── docker-compose.yml
├── specs.md          # Full technical specification
└── README.md
```

## Cloud Deployment (Railway)

The app can be deployed to [Railway](https://railway.com/) as a PoC environment. See `specs.md` §8.3 for full details.

**Quick setup:**

1. Create a Railway project and connect the GitHub repo
2. Add a PostgreSQL plugin (managed, one-click)
3. Create an API service (root: `/`, Dockerfile: `api/Dockerfile`)
4. Create a frontend service (root: `/`, Dockerfile: `frontend/Dockerfile`)
5. Set environment variables (see `specs.md` §8.3)
6. Generate a public domain for the frontend service
7. Push to `main` — Railway auto-deploys

The frontend Dockerfile is multi-stage: `docker compose` targets the `dev` stage (Vite dev server), while Railway builds the full image ending at the `production` stage (nginx + static files + API reverse proxy).

## Documentation

See [`specs.md`](specs.md) for the full technical specification including data model, API contracts, UI wireframes, and colour system.

## License

TBD
