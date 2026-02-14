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
- Tests: unit tests in `#[cfg(test)]` modules, integration tests in `tests/`, mock yr.no with `wiremock`.
- Run `cargo fmt` and `cargo clippy` before committing.
- Respect yr.no Terms of Service: always send the correct `User-Agent` header, honour `Expires` and `If-Modified-Since` headers.

### TypeScript (Frontend)
- Use functional components with hooks.
- Use TanStack Query for API data fetching.
- Style with Tailwind CSS using the project colour palette (see specs.md §5.6).
- Tests: Vitest + React Testing Library for components, MSW for API mocking.
- Run `npm run lint` and `npm run typecheck` before committing.
- Mobile-first responsive design.

### Database
- Migrations managed via `sqlx migrate`.
- Never overwrite forecast data — every fetch creates a new row (append-only pattern).
- Use UUIDs for all primary keys.
- All timestamps must be `TIMESTAMPTZ`.

### API Design
- All endpoints under `/api/v1/`.
- JSON responses only.
- Return `X-Forecast-Stale: true` header when serving cached data that couldn't be refreshed.
- Calculated fields (`feels_like_c`, `precipitation_type`) are computed by the API, not stored from yr.no.

## Colour Palette

The UI uses a dark theme with warm charcoal neutrals:

| Role | Hex | Usage |
|------|-----|-------|
| Background | `#0D0D0C` | App background |
| Surface | `#171614` | Sidebar, cards |
| Primary | `#2DD4A8` | Buttons, active states |
| Secondary | `#14B8A6` | Secondary actions |
| Accent Warm | `#F5A623` | Highlights, temperature |
| Accent Cool | `#7C8CF5` | Wind, info states |
| Accent Rose | `#D4687A` | Title, race course, checkpoints, slider |
| Text Primary | `#F0EEEB` | Main text |
| Text Secondary | `#9E9A93` | Labels, captions |
| Error | `#EF4444` | Error states |

## Key Conventions

- Forecast freshness threshold: 1 minute (re-fetch from yr.no if older).
- Pacing model: even pacing (`pass_time = start + duration × distance_fraction`).
- yr.no endpoint: `https://api.met.no/weatherapi/locationforecast/2.0/complete`
- User-Agent for yr.no: `WeatherBingo/0.1 github.com/LC-Zurich-Doppelstock/weather-bingo`
