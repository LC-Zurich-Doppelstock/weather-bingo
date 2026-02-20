# AGENTS.md

Instructions for AI coding agents working on this project.

## Project Overview

Weather Bingo is a race-day weather forecasting app for cross-country skiing. See `specs.md` for the full technical specification, `README.md` for the high-level overview, and `data/` for seed data.

## Tech Stack

- **Frontend:** React 18+, TypeScript, Vite, Tailwind CSS, Leaflet (map), Recharts (charts), Vitest + React Testing Library
- **API:** Rust, Axum, sqlx, PostgreSQL
- **Database:** PostgreSQL 16
- **External API:** yr.no Locationforecast 2.0
- **Dev environment:** Docker Compose (local only)

## Project Structure

```
weather-bingo/
├── api/              # Rust REST API (Axum)
├── frontend/         # React + TypeScript (Vite)
├── data/             # Race GPX files & seed data
├── docker-compose.yml
├── specs.md          # Full technical spec (source of truth)
├── AGENTS.md         # This file
└── README.md
```

## Development Rules

### General
- Always read `specs.md` before implementing a feature — it is the source of truth for data models, API contracts, and UI behaviour.
- Write tests alongside code. Never submit code without corresponding tests.
- Minimum 80% test coverage for business logic.
- Use English for all code, comments, and documentation.
- Keep commits focused and atomic — one logical change per commit.

### Rust (API)
- Use `sqlx` for database access with compile-time checked queries where possible.
- Use `axum` for HTTP routing.
- Error handling: use `thiserror` for custom error types, return proper HTTP status codes (see specs.md §4.5).
- Tests: unit tests in `#[cfg(test)]` modules. No integration tests or wiremock — expand unit tests with mock data instead.
- Run `cargo fmt` and `cargo clippy` before committing.
- Respect yr.no Terms of Service: always send the correct `User-Agent` header, honour `Expires` and `If-Modified-Since` headers.

### TypeScript (Frontend)
- Use functional components with hooks.
- Use TanStack Query for API data fetching.
- Style with Tailwind CSS using the project colour palette (see specs.md §5.6).
- Import colours from `styles/theme.ts` — never hardcode hex values in components.
- Tests: Vitest + React Testing Library for components, MSW for API mocking.
- Run `npm run lint` and `npm run typecheck` before committing.
- Mobile-first responsive design.

### Database
- Migrations managed via `sqlx migrate`.
- Never overwrite forecast data — every fetch creates a new row (append-only pattern).
- Deduplication on write: `ON CONFLICT DO NOTHING` using partial unique index on `(checkpoint_id, forecast_time, yr_model_run_at) WHERE yr_model_run_at IS NOT NULL`.
- Use UUIDs for all primary keys.
- All timestamps must be `TIMESTAMPTZ`.

### API Design
- All endpoints under `/api/v1/`.
- JSON responses only.
- Return `X-Forecast-Stale: true` header when serving cached data that couldn't be refreshed.
- Calculated fields (`feels_like_c`, `precipitation_type`) are computed by the API, not stored from yr.no.
- Unified `Weather` struct with `#[serde(skip_serializing_if = "Option::is_none")]` — detail-only fields are omitted when `None` (race endpoint) and included when present (single-checkpoint endpoint).

## Architecture

### Extract-on-Read Pattern

The API uses an **extract-on-read** architecture for forecast data:

1. The full yr.no JSON response (~10 days of timeseries) is cached in the `yr_responses` table, keyed by `checkpoint_id` (FK to `checkpoints`).
2. When a forecast is requested, the API ensures the yr.no cache is fresh, then **extracts the relevant forecast entry in-memory** from the cached JSON.
3. Extracted forecasts are also written to the `forecasts` table for historical tracking (append-only, deduplicated via `ON CONFLICT DO NOTHING`).

This avoids the bug where new checkpoints at already-cached locations would have no forecast data, since extraction happens at read time rather than write time. The `checkpoint_id` FK ensures a direct lookup — there are no coordinate-equality queries for cache matching.

### Pacing

Pacing is computed **server-side only** using elevation-adjusted cost factors. The API returns `expected_time` for each checkpoint in the race forecast response. The frontend reads this directly — there is no client-side pacing code.

### Background Poller

A background `tokio::spawn` task proactively fetches yr.no forecasts for all checkpoints of upcoming races. This ensures every model run is captured even when no users are browsing. Key details:

- **Schedule:** Expires-driven — sleeps until `MIN(expires_at) + 30s`, clamped to [1 min, 30 min].
- **Retry:** Up to 5 retries with 2-min delay when yr.no returns 304 (no new data yet).
- **Time bands:** For each checkpoint, forecasts are extracted for hourly slots covering realistic arrival times (10–30 km/h).
- **State:** In-memory `Arc<RwLock<PollerState>>`, exposed via `/api/v1/poller/status`.
- **Implementation:** `services/poller.rs` (logic + tests), `routes/poller.rs` (status endpoint).

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/races` | List all races |
| GET | `/api/v1/races/:id/course` | Parsed course GPS points (lat/lon/ele array) |
| GET | `/api/v1/races/:id/checkpoints` | All checkpoints for a race |
| GET | `/api/v1/forecasts/checkpoint/:checkpoint_id` | Full forecast for a checkpoint |
| GET | `/api/v1/forecasts/checkpoint/:checkpoint_id/history` | Historical forecast evolution |
| GET | `/api/v1/forecasts/race/:race_id` | Simplified forecasts for all checkpoints |
| GET | `/api/v1/health` | Health check |
| GET | `/api/v1/poller/status` | Background poller status |

> Note: There is no `GET /api/v1/races/:id` single-race detail endpoint. Race metadata comes from the list endpoint; course data from the course endpoint.

## Colour Palette

The UI uses a dark theme with warm charcoal neutrals:

| Role | Hex | Usage |
|------|-----|-------|
| Background | `#0D0D0C` | App background |
| Surface | `#171614` | Sidebar, cards |
| Surface Alt | `#1F1E1C` | Hover states, input fields, map overlays |
| Primary | `#2DD4A8` | Buttons, active states |
| Primary Hover | `#34EBB9` | Button hover, focus rings |
| Secondary | `#14B8A6` | Secondary actions |
| Accent Warm | `#F5A623` | Highlights, temperature |
| Accent Cool | `#7C8CF5` | Wind, info states |
| Accent Rose | `#D4687A` | Title, race course, checkpoints, slider |
| Text Primary | `#F0EEEB` | Main text |
| Text Secondary | `#9E9A93` | Labels, captions |
| Text Muted | `#8A8580` | Placeholders, disabled text (WCAG AA 4.5:1) |
| Border | `#2C2A27` | Dividers, card borders, subtle lines |
| Error | `#EF4444` | Error states |
| Success | `#2DD4A8` | Success feedback (same as Primary) |

### Chart Colours

| # | Hex | Name | Usage |
|---|-----|------|-------|
| 1 | `#2DD4A8` | Emerald mint | Temperature |
| 2 | `#14B8A6` | Teal | Feels-like temperature |
| 3 | `#F5A623` | Golden amber | Wind speed |
| 4 | `#7C8CF5` | Lavender blue | Precipitation |
| 5 | `#34EBB9` | Bright mint | Humidity |
| 6 | `#5A7A6E` | Faded green | Cloud cover |
| 7 | `#D4687A` | Dusty rose | Elevation profile |

Uncertainty ranges (percentile bands) are rendered at **15% opacity** of the same colour.

## Key Conventions

- Forecast freshness is controlled by yr.no's `Expires` header — there is no configurable staleness threshold.
- Pacing model: elevation-adjusted (server-side only). Even pacing as fallback for flat courses.
- yr.no endpoint: `https://api.met.no/weatherapi/locationforecast/2.0/complete`
- User-Agent for yr.no: `WeatherBingo/0.1 github.com/LC-Zurich-Doppelstock/weather-bingo`
- HTTP timeout for yr.no: 30 seconds.
- Docker DB credentials: user=`wb`, password=`wb_dev`, database=`weather_bingo`.

### UI Patterns

- **Collapsible sections:** The `ElevationProfile` component implements a collapsible pattern using a header button with chevron icon, toggling `max-h-0 overflow-hidden` / `max-h-[200px]` with `transition-all duration-200`. Reuse this pattern for any future collapsible sections.
- **Elevation Profile:** `components/ElevationProfile/ElevationProfile.tsx` — Recharts AreaChart below the map, desktop-only (`hidden lg:block`), collapsible, bidirectional hover sync via `hoveredCheckpointId` pattern.
- **Geo utilities:** `utils/geo.ts` provides `haversineDistance()` and `computeElevationProfile()` for computing cumulative distance along a GPS track.
