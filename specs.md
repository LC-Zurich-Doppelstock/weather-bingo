# Weather Bingo — Technical Specification

> **Version:** 0.1.0 (Draft)
> **Date:** 2026-02-13
> **Owner:** LC-Zurich-Doppelstock

---

## 1. Overview

Weather Bingo is a web application that helps cross-country skiers plan for race day by visualising weather forecasts along a race course. Users select a race, set their target finish time, and explore detailed weather conditions at checkpoints or along the entire course.

The app stores both current and historical forecasts, allowing users to observe how weather predictions evolve over time — critical for understanding snow conditions and race-day preparation.

---

## 2. Architecture

```
┌─────────────┐       ┌─────────────┐       ┌──────────────┐       ┌─────────────┐
│             │       │             │       │              │       │             │
│  Frontend   │──────▶│   REST API  │──────▶│  PostgreSQL  │       │   yr.no     │
│ (TypeScript)│◀──────│   (Rust)    │◀──────│              │       │   API       │
│             │       │             │──────▶│              │       │             │
└─────────────┘       │             │◀──────────────────────       └──────┬──────┘
                      │             │─────────────────────────────────────▶│
                      │             │◀────────────────────────────────────┘
                      └─────────────┘
```

### 2.1 Components

| Component  | Technology         | Purpose                                            |
| ---------- | ------------------ | -------------------------------------------------- |
| Frontend   | TypeScript (React) | Interactive map, charts, race/weather visualisation |
| API        | Rust (Axum)        | REST endpoints, forecast fetching, caching logic    |
| API Docs   | utoipa + Swagger UI | Interactive OpenAPI documentation at `/swagger-ui/` |
| Database   | PostgreSQL         | Stores races, checkpoints, forecasts (current + historic) |
| Weather    | yr.no (MET Norway) | External weather data source                       |
| Dev Infra  | Docker Compose     | Local development environment                      |

### 2.2 Key Design Principles

- **Cache-first**: The API serves forecasts from the database. If data is missing or stale, it fetches from yr.no.
- **Historical preservation**: Forecasts are never overwritten — each fetch is stored as a new record with a `fetched_at` timestamp.
- **Race-agnostic data model**: The schema supports multiple races, locations, and seasons.
- **Test-driven**: All components (API and UI) include comprehensive test suites.

---

## 3. Data Model

### 3.1 Races

```
Table: races
├── id              UUID        PK
├── name            VARCHAR     e.g. "Vasaloppet"
├── year            INT         e.g. 2026
├── start_time      TIMESTAMPTZ e.g. 2026-03-01T08:00:00+01:00
├── course_gpx      TEXT        GPX data (full course geometry)
├── distance_km     DECIMAL     e.g. 90.0
├── created_at      TIMESTAMPTZ
└── updated_at      TIMESTAMPTZ
```

### 3.2 Checkpoints

```
Table: checkpoints
├── id              UUID        PK
├── race_id         UUID        FK → races.id
├── name            VARCHAR     e.g. "Smågan", "Mångsbodarna"
├── distance_km     DECIMAL     Distance from start (km)
├── latitude        DECIMAL(9,6)
├── longitude       DECIMAL(9,6)
├── elevation_m     DECIMAL     Elevation in meters (from GPX)
├── sort_order      INT         Ordering along the course
├── created_at      TIMESTAMPTZ
└── updated_at      TIMESTAMPTZ
```

### 3.3 Forecasts

```
Table: forecasts
├── id              UUID        PK
├── checkpoint_id   UUID        FK → checkpoints.id
├── forecast_time   TIMESTAMPTZ The datetime the forecast is FOR
├── fetched_at      TIMESTAMPTZ When this forecast was retrieved from the source
├── source          VARCHAR     e.g. "yr.no"
│
│   ── Weather Parameters (from yr.no) ──
├── temperature_c               DECIMAL     Air temperature (°C)
├── temperature_percentile_10_c DECIMAL     10th percentile (90% chance above this) (nullable)
├── temperature_percentile_90_c DECIMAL     90th percentile (10% chance above this) (nullable)
├── wind_speed_ms               DECIMAL     Wind speed (m/s)
├── wind_speed_percentile_10_ms DECIMAL     10th percentile wind speed (nullable)
├── wind_speed_percentile_90_ms DECIMAL     90th percentile wind speed (nullable)
├── wind_direction_deg          DECIMAL     Wind direction (degrees)
├── wind_gust_ms                DECIMAL     Max gust speed (nullable, short-range only)
├── precipitation_mm            DECIMAL     Precipitation amount (mm)
├── precipitation_min_mm        DECIMAL     Min likely precipitation (nullable)
├── precipitation_max_mm        DECIMAL     Max likely precipitation (nullable)
├── humidity_pct                DECIMAL     Relative humidity (%)
├── dew_point_c                 DECIMAL     Dew point temperature (°C)
├── cloud_cover_pct             DECIMAL     Cloud cover (%)
├── uv_index                    DECIMAL     UV index (nullable, short-range only)
├── symbol_code                 VARCHAR     yr.no weather symbol code
│
│   ── Calculated by API (not from yr.no) ──
├── feels_like_c                DECIMAL     Wind chill / feels-like (calculated)
├── precipitation_type          VARCHAR     "snow", "rain", "sleet", "none" (inferred from symbol_code + temp)
│
├── created_at              TIMESTAMPTZ
└── raw_response            JSONB       Full raw API response for future use
```

> **Note:** Each forecast fetch from yr.no creates a new row. The `fetched_at` column distinguishes the latest forecast from historical ones for the same `checkpoint_id` + `forecast_time`.

### 3.4 Indexes & Constraints

- `UNIQUE (name, year)` on `races` — enables idempotent upsert during GPX seeding
- `UNIQUE (race_id, sort_order)` on `checkpoints` — enables idempotent upsert during GPX seeding
- `forecasts(checkpoint_id, forecast_time, fetched_at DESC)` — fast lookup of latest forecast per checkpoint/time
- `forecasts(checkpoint_id, fetched_at)` — historical forecast queries
- `checkpoints(race_id, sort_order)` — ordered checkpoint retrieval

---

## 4. API (Rust / Axum)

### 4.1 Endpoints

#### Races

| Method | Path                          | Description                          |
| ------ | ----------------------------- | ------------------------------------ |
| GET    | `/api/v1/races`               | List all available races             |
| GET    | `/api/v1/races/:id`           | Get race details (incl. GPX course)  |
| GET    | `/api/v1/races/:id/checkpoints` | Get all checkpoints for a race     |

#### Forecasts

| Method | Path                                          | Description                                                                 |
| ------ | --------------------------------------------- | --------------------------------------------------------------------------- |
| GET    | `/api/v1/forecasts/checkpoint/:checkpoint_id`  | Latest forecast for a checkpoint. Query params: `datetime` (ISO 8601)       |
| GET    | `/api/v1/forecasts/checkpoint/:checkpoint_id/history` | Historical forecasts for a checkpoint + datetime. Shows forecast evolution. |
| GET    | `/api/v1/forecasts/race/:race_id`              | Latest forecasts for all checkpoints. Query params: `target_duration_hours` |

#### Health

| Method | Path           | Description        |
| ------ | -------------- | ------------------ |
| GET    | `/api/v1/health` | Health check       |

### 4.2 Forecast Resolution Logic

When a forecast is requested:

```
1. Calculate the expected pass-through time for the checkpoint:
   pass_time = race.start_time + (checkpoint.distance_km / race.distance_km) * target_duration

2. Look up the latest forecast in DB for (checkpoint_id, pass_time):
   SELECT * FROM forecasts
   WHERE checkpoint_id = :id
     AND forecast_time = closest(:pass_time)
   ORDER BY fetched_at DESC
   LIMIT 1

3. If no forecast exists OR the latest fetch is older than the source's update interval:
   → Fetch from yr.no Locationforecast 2.0
   → Store the full response in DB
   → Return to client

4. Otherwise:
   → Return cached forecast from DB
```

### 4.3 Forecast Freshness

- The API checks for new forecast data from yr.no when serving a request if the most recent `fetched_at` for that location is older than the configured staleness threshold (default: **60 seconds**, configurable via `FORECAST_STALENESS_SECS`).
- yr.no's `Expires` / `Last-Modified` headers should be respected to avoid unnecessary calls.
- yr.no API usage must comply with their [Terms of Service](https://api.met.no/doc/TermsOfService) (identify via `User-Agent` header).

### 4.3.1 Configuration (Environment Variables)

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | Yes | — | PostgreSQL connection string |
| `YR_USER_AGENT` | No | `WeatherBingo/0.1 github.com/LC-Zurich-Doppelstock/weather-bingo` | User-Agent for yr.no API requests |
| `PORT` | No | `8080` | HTTP server listen port |
| `FORECAST_STALENESS_SECS` | No | `60` | Seconds before a cached forecast is considered stale |
| `DATA_DIR` | No | `./data` | Directory containing GPX files for race seeding at startup |

### 4.4 yr.no Integration

- **Endpoint:** `https://api.met.no/weatherapi/locationforecast/2.0/complete` (use `complete` for percentile data)
- **Parameters:** `lat`, `lon`, `altitude` (recommended for accurate temperature correction)
- **Rate limiting:** Respect `Expires` header; use `If-Modified-Since` for conditional requests.
- **User-Agent:** `WeatherBingo/0.1 github.com/LC-Zurich-Doppelstock/weather-bingo`

#### Data from yr.no (Nordic region)

| Parameter | yr.no variable | Availability |
|---|---|---|
| Temperature | `air_temperature` | All ranges |
| Temp uncertainty | `air_temperature_percentile_10`, `_90` | All ranges |
| Wind speed | `wind_speed` | All ranges |
| Wind uncertainty | `wind_speed_percentile_10`, `_90` | All ranges |
| Wind gust | `wind_speed_of_gust` | Short-range only (0–60h) |
| Wind direction | `wind_from_direction` | All ranges |
| Precipitation | `precipitation_amount` | Period data |
| Precip uncertainty | `precipitation_amount_min`, `_max` | Period data |
| Humidity | `relative_humidity` | All ranges |
| Dew point | `dew_point_temperature` | All ranges |
| Cloud cover | `cloud_area_fraction` | All ranges |
| UV index | `ultraviolet_index_clear_sky` | Short-range only (0–60h) |
| Weather symbol | `symbol_code` | Period data |

#### Calculated by our API

| Parameter | Method |
|---|---|
| **Feels-like / wind chill** | Calculated using the North American Wind Chill Index formula: `13.12 + 0.6215T - 11.37V^0.16 + 0.3965TV^0.16` (T in °C, V in km/h). Applied when T ≤ 10°C and V ≥ 4.8 km/h. |
| **Precipitation type** | Inferred from `symbol_code` (e.g., `snow`, `lightrain`, `sleet`). Fallback: temperature-based heuristic (< 0°C → snow, 0–2°C → sleet, > 2°C → rain). |

#### Historical forecast data

yr.no provides historical Nordic forecast model data in NetCDF format via their [thredds server](https://thredds.met.no/thredds/metno.html). This can be used as a data source for historical forecast evolution. Integration is a future enhancement.

### 4.5 Error Handling

| Scenario                     | HTTP Status | Behaviour                                       |
| ---------------------------- | ----------- | ------------------------------------------------ |
| yr.no unavailable            | 200 (stale) | Return cached data with `X-Forecast-Stale: true` header |
| yr.no unavailable, no cache  | 502         | Return error with message                        |
| Invalid race/checkpoint ID   | 404         | Standard not-found response                      |
| Invalid query parameters     | 400         | Validation error details                         |

### 4.6 API Documentation (OpenAPI / Swagger)

The API automatically generates an OpenAPI 3.0 specification using `utoipa` and serves interactive documentation via `utoipa-swagger-ui`:

| Path | Description |
|------|-------------|
| `/swagger-ui/` | Interactive Swagger UI |
| `/api-docs/openapi.json` | OpenAPI JSON specification |

All route handlers, request parameters, and response types are annotated with `utoipa::ToSchema` and `utoipa::path` macros for automatic documentation.

---

## 5. Frontend (TypeScript / React)

### 5.1 Tech Stack

| Concern         | Library / Tool                     |
| --------------- | ---------------------------------- |
| Framework       | React 18+ with TypeScript          |
| Build tool      | Vite                               |
| Map             | Leaflet + React-Leaflet (OpenStreetMap) |
| Charts          | Recharts (lightweight, responsive) |
| HTTP client     | Fetch API / TanStack Query         |
| Styling         | Tailwind CSS v4 (`@tailwindcss/vite` plugin, `@theme` in CSS) |
| Testing         | Vitest + React Testing Library     |
| Responsive      | Mobile-first with Tailwind breakpoints |

### 5.2 Layout

```
┌──────────────────────────────────────────────────────────┐
│  Header: Logo / App Name          Race Dropdown ▾        │
├──────────────────────────────────┬───────────────────────┤
│                                  │                       │
│                                  │     Sidebar           │
│         Map                      │  ┌─────────────────┐  │
│   (race course + checkpoints)    │  │ Target Time ⏱    │  │
│                                  │  │ [slider/input]   │  │
│                                  │  ├─────────────────┤  │
│                                  │  │                 │  │
│                                  │  │ Weather Detail  │  │
│                                  │  │ (checkpoint or  │  │
│                                  │  │  course graph)  │  │
│                                  │  │                 │  │
│                                  │  └─────────────────┘  │
├──────────────────────────────────┴───────────────────────┤
│  Footer (attribution: yr.no, OSM)                        │
└──────────────────────────────────────────────────────────┘
```

On **mobile**, the sidebar collapses below the map as a bottom sheet / drawer.

### 5.3 User Flow

1. **Select a race** from the dropdown (default: Vasaloppet 2026).
2. The **map** zooms to the race area and draws the course polyline + checkpoint markers.
3. **Set target race time** via a slider or input field (e.g., 5h – 15h, default: 8h).
4. **Interact:**
   - **Click a checkpoint marker** → Sidebar shows detailed weather for that checkpoint at the calculated pass-through time, including a mini-timeline showing conditions ~1h before and after.
   - **Click the race course** (or a "Course Overview" button) → Sidebar shows a compact graph of weather along the entire course (x-axis: km, y-axis: key parameters).

### 5.4 Checkpoint Detail View (Sidebar)

When a checkpoint is selected, the sidebar displays:

```
┌─────────────────────────────────┐
│  📍 Smågan (24 km)              │
│  Expected: 10:24 CET            │
├─────────────────────────────────┤
│                                 │
│  🌡  -4°C  (feels like -9°C)   │
│       p10/p90: -6°C to -2°C    │
│                                 │
│  💨  3.2 m/s NW  (gust 6.8)    │
│       p10/p90: 2.0 – 5.1 m/s   │
│                                 │
│  🌨  Snow 0.4 mm/h              │
│       range: 0.1 – 0.8 mm/h    │
│                                 │
│  💧  Humidity: 82%              │
│  ☁  Cloud cover: 90%           │
│                                 │
├─────────────────────────────────┤
│  ⏱ Mini Timeline (09:00–12:00) │
│  ┌─────────────────────────────┐│
│  │ temp ───────•───────        ││
│  │ precip ▁▂▃▅▇█▇▅▃▂          ││
│  └─────────────────────────────┘│
│        09   10  [10:24] 11  12  │
├─────────────────────────────────┤
│  📊 Forecast History            │
│  "Show how this forecast has    │
│   changed over time" [button]   │
└─────────────────────────────────┘
```

### 5.5 Course Overview Graph (Sidebar)

When the full course is selected:

```
┌──────────────────────────────────┐
│  📈 Weather Along the Course     │
├──────────────────────────────────┤
│                                  │
│  Temperature (°C)                │
│  ┌──────────────────────────┐    │
│  │    ╲    ╱──╲             │    │
│  │     ╲──╱    ╲___         │    │
│  └──────────────────────────┘    │
│  0km    20    40    60    90km    │
│                                  │
│  Precipitation (mm/h)            │
│  ┌──────────────────────────┐    │
│  │         ▃▅▇▅▃            │    │
│  │  ▁▂▃▅▇█      ▅▃▂▁       │    │
│  └──────────────────────────┘    │
│  0km    20    40    60    90km    │
│                                  │
│  Wind Speed (m/s)                │
│  ┌──────────────────────────┐    │
│  │  ───╱──╲───╱──────       │    │
│  └──────────────────────────┘    │
│  0km    20    40    60    90km    │
│                                  │
│  Checkpoints marked with ◆      │
└──────────────────────────────────┘
```

- Compact, stacked mini-charts (sparkline style).
- Uncertainty ranges shown as shaded bands where available.
- Checkpoint positions marked on x-axis.

### 5.6 Colour Scheme

The UI colour palette will be derived from a user-provided reference image. Colours will be extracted and mapped to a modern, sleek design system:

Derived from the reference artwork (tropical botanical pattern on black background):

| Role              | Hex         | Colour             | Usage                                        |
| ----------------- | ----------- | ------------------ | -------------------------------------------- |
| **Background**    | `#0A0F0D`   | Near-black green   | App background, main canvas                  |
| **Surface**       | `#141E1B`   | Dark jungle        | Sidebar, cards, elevated elements            |
| **Surface Alt**   | `#1C2B27`   | Deep teal-black    | Hover states, input fields, map overlays     |
| **Primary**       | `#2DD4A8`   | Emerald mint       | Buttons, active states, links, CTA           |
| **Primary Hover** | `#34EBB9`   | Bright mint        | Button hover, focus rings                    |
| **Secondary**     | `#14B8A6`   | Teal               | Secondary actions, chart accents             |
| **Accent Warm**   | `#F5A623`   | Golden amber       | Highlights, warnings, temperature indicator  |
| **Accent Cool**   | `#7C8CF5`   | Lavender blue      | Info states, wind indicators, secondary data |
| **Text Primary**  | `#F0F7F4`   | Off-white green    | Main text, headings                          |
| **Text Secondary**| `#8BA89E`   | Muted sage         | Labels, captions, secondary text             |
| **Text Muted**    | `#5A7A6E`   | Faded green        | Placeholders, disabled text                  |
| **Border**        | `#2A3F38`   | Dark green-grey    | Dividers, card borders, subtle lines         |
| **Error**         | `#EF4444`   | Red                | Error states, critical alerts                |
| **Success**       | `#2DD4A8`   | Emerald mint (=primary) | Success feedback                        |

#### Chart Colour Palette

For weather data visualisation, the following ordered set is used:

| #  | Hex         | Name             | Usage                          |
| -- | ----------- | ---------------- | ------------------------------ |
| 1  | `#2DD4A8`   | Emerald mint     | Temperature                    |
| 2  | `#14B8A6`   | Teal             | Feels-like temperature         |
| 3  | `#7C8CF5`   | Lavender blue    | Wind speed                     |
| 4  | `#F5A623`   | Golden amber     | Precipitation                  |
| 5  | `#34EBB9`   | Bright mint      | Humidity                       |
| 6  | `#5A7A6E`   | Faded green      | Cloud cover                    |

Uncertainty ranges (percentile bands) are rendered as the same colour at **15% opacity**.

#### Design Principles

- **Dark-first**: The UI uses a dark theme inspired by the black background of the artwork.
- **Nature-forward**: Greens and teals dominate, keeping the organic, botanical feel.
- **High contrast**: Bright mint and golden amber ensure readability and clear CTAs against the dark background.
- **Subtle depth**: Surface layers use progressively lighter shades of dark green to create depth without harsh borders.
- **Warm accents sparingly**: Golden amber is reserved for attention-drawing elements (temperature highlights, warnings) — never overused.

---

## 6. Testing Strategy

### 6.1 API (Rust)

| Layer           | Tool            | Coverage                                           |
| --------------- | --------------- | -------------------------------------------------- |
| Unit tests      | `#[cfg(test)]`  | Forecast resolution logic, time calculations, data parsing |
| Integration     | `axum::test`    | Endpoint responses, DB queries, error handling     |
| External mocks  | `wiremock`      | yr.no API responses (success, failure, rate limit) |
| DB tests        | `sqlx` test     | Migrations, queries against test database          |

### 6.2 Frontend (TypeScript)

| Layer            | Tool                           | Coverage                                       |
| ---------------- | ------------------------------ | ---------------------------------------------- |
| Unit tests       | Vitest                         | Utility functions, time/distance calculations  |
| Component tests  | Vitest + React Testing Library | Sidebar, dropdowns, inputs, chart rendering    |
| Integration      | Vitest + MSW (Mock Service Worker) | API interaction, loading/error states      |
| E2E (future)     | Playwright                     | Full user flows (optional, for later)          |

### 6.3 Test Conventions

- Tests live alongside source files: `foo.ts` → `foo.test.ts`, `foo.rs` → `#[cfg(test)] mod tests`.
- Vitest is configured inline in `vite.config.ts` (no separate `vitest.config.ts`), using `jsdom` environment and a global setup file at `src/test-setup.ts` (loads `@testing-library/jest-dom` matchers and polyfills like `ResizeObserver`).
- CI should run all tests before merge (to be set up later).
- Minimum coverage target: **80%** for business logic.

---

## 7. Seed Data

Race and checkpoint data is stored in GPX files under `data/`. The GPX file is the **single source of truth** for race metadata and checkpoint positions.

### 7.1 GPX Format

GPX files use standard GPX 1.1 with a custom `wb:` XML namespace for Weather Bingo extensions:

```xml
<gpx xmlns="http://www.topografix.com/GPX/1/1"
     xmlns:wb="https://github.com/LC-Zurich-Doppelstock/weather-bingo/gpx"
     version="1.1" creator="WeatherBingo">
  <metadata>
    <name>Vasaloppet</name>           <!-- Race name -->
    <extensions>
      <wb:race>
        <wb:year>2026</wb:year>
        <wb:start_time>2026-03-01T08:00:00+01:00</wb:start_time>
        <wb:distance_km>90</wb:distance_km>
      </wb:race>
    </extensions>
  </metadata>

  <!-- Checkpoints: waypoints with <type>checkpoint</type> -->
  <wpt lat="61.1056" lon="13.3042">
    <ele>350</ele>
    <name>Berga (Start)</name>
    <type>checkpoint</type>
    <extensions>
      <wb:distance_km>0</wb:distance_km>
    </extensions>
  </wpt>
  <!-- ... more waypoints ... -->

  <!-- Full course track for map rendering -->
  <trk>
    <name>Vasaloppet</name>
    <trkseg>
      <trkpt lat="..." lon="..."><ele>...</ele></trkpt>
      <!-- ... -->
    </trkseg>
  </trk>
</gpx>
```

Key conventions:
- Race metadata lives in `<metadata><extensions><wb:race>` (year, start_time, distance_km).
- Checkpoints are `<wpt>` elements with `<type>checkpoint</type>`. Non-checkpoint waypoints (e.g. `<type>poi</type>`) are ignored.
- Each checkpoint must have `<wb:distance_km>` in its extensions.
- The `<trk>` element provides the full course geometry for map rendering.

### 7.2 Startup Seeding

On startup (after running database migrations), the API:

1. Scans `DATA_DIR` (default `./data`) for `*.gpx` files.
2. Parses each file using the `services::gpx` module.
3. Upserts each race and its checkpoints into the database using `INSERT ... ON CONFLICT`:
   - Races are matched by `(name, year)`.
   - Checkpoints are matched by `(race_id, sort_order)`.
4. This is **idempotent** — re-running on the same data is a no-op.

### 7.3 Current Data

- **`data/vasaloppet-2026.gpx`** — Vasaloppet 2026 (90 km, Berga/Sälen to Mora, 9 checkpoints). Coordinates sourced from the [official track profile on Wikipedia](https://en.wikipedia.org/wiki/Vasaloppet#Track_profile).

---

## 8. Development Environment

### 8.1 Docker Compose Setup

```yaml
services:
  db:
    image: postgres:16-alpine
    ports: ["5431:5432"]
    environment:
      POSTGRES_DB: weather_bingo
      POSTGRES_USER: wb
      POSTGRES_PASSWORD: wb_dev
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U wb -d weather_bingo"]
      interval: 5s
      timeout: 5s
      retries: 5

  api:
    build:
      context: .
      dockerfile: api/Dockerfile
    ports: ["8080:8080"]
    environment:
      DATABASE_URL: postgres://wb:wb_dev@db:5432/weather_bingo
      YR_USER_AGENT: "WeatherBingo/0.1 github.com/LC-Zurich-Doppelstock/weather-bingo"
      PORT: "8080"
      DATA_DIR: /app/data
    volumes:
      - ./data:/app/data:ro
    depends_on:
      db:
        condition: service_healthy

  frontend:
    build:
      context: ./frontend
      target: dev
    ports: ["3000:3000"]
    environment:
      VITE_API_URL: http://api:8080
    depends_on:
      - api

volumes:
  pgdata:
```

> **Note:** The DB host port is `5431` (not `5432`) to avoid conflicts with a local PostgreSQL instance. Inside the Docker network, containers connect on the standard port `5432`.

### 8.2 Project Structure

```
weather-bingo/
├── api/                        # Rust API
│   ├── Cargo.toml
│   ├── Dockerfile
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs           # Configuration / env vars
│   │   ├── db/
│   │   │   ├── mod.rs
│   │   │   ├── models.rs       # DB models (Race, Checkpoint, Forecast)
│   │   │   └── queries.rs      # SQL queries (incl. upsert for GPX seeding)
│   │   ├── routes/
│   │   │   ├── mod.rs
│   │   │   ├── races.rs        # Race endpoints
│   │   │   ├── forecasts.rs    # Forecast endpoints
│   │   │   └── health.rs       # Health check
│   │   ├── services/
│   │   │   ├── mod.rs
│   │   │   ├── forecast.rs     # Forecast resolution logic
│   │   │   ├── gpx.rs          # GPX parser (wb: namespace extensions)
│   │   │   └── yr.rs           # yr.no API client
│   │   └── errors.rs           # Error types
│   └── migrations/             # SQL migrations (sqlx)
│
├── frontend/                   # React + TypeScript
│   ├── package.json
│   ├── Dockerfile              # Multi-stage: dev (Vite) / production (nginx)
│   ├── nginx.conf.template     # Production nginx config (API proxy + SPA)
│   ├── vite.config.ts          # Vite + Vitest config (inline test block)
│   ├── tsconfig.json
│   ├── src/
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── index.css           # Tailwind v4 + @theme colour tokens
│   │   ├── test-setup.ts       # Vitest global setup (jest-dom, polyfills)
│   │   ├── vite-env.d.ts
│   │   ├── api/                # API client & types
│   │   │   ├── client.ts
│   │   │   └── types.ts
│   │   ├── components/
│   │   │   ├── Map/
│   │   │   │   ├── RaceMap.tsx
│   │   │   │   ├── CoursePolyline.tsx
│   │   │   │   └── CheckpointMarker.tsx
│   │   │   ├── Sidebar/
│   │   │   │   ├── Sidebar.tsx
│   │   │   │   ├── CheckpointDetail.tsx
│   │   │   │   ├── CourseOverview.tsx
│   │   │   │   └── MiniTimeline.tsx
│   │   │   ├── Controls/
│   │   │   │   ├── RaceSelector.tsx
│   │   │   │   └── TargetTimeInput.tsx
│   │   │   ├── Layout/
│   │   │   │   ├── Header.tsx
│   │   │   │   └── Footer.tsx
│   │   │   └── ErrorBoundary.tsx
│   │   ├── hooks/              # Custom React hooks
│   │   │   ├── useRace.ts
│   │   │   ├── useForecast.ts
│   │   │   └── usePassThroughTime.ts
│   │   ├── utils/              # Helpers (time calc, formatting)
│   │   │   ├── pacing.ts
│   │   │   └── formatting.ts
│   │   └── styles/
│   │       └── theme.ts        # Colour palette constants
│   └── public/
│
├── data/                       # Race GPX files (seed data)
│   └── vasaloppet-2026.gpx
├── docker-compose.yml
├── AGENTS.md
├── README.md
└── specs.md
```

### 8.3 Cloud Deployment (Railway)

The application is deployable to [Railway](https://railway.com/) as a PoC/staging environment. Railway auto-builds from the GitHub repo on push to `main`.

**Architecture:**

| Service | Railway Type | Build Source | Public |
|---------|-------------|-------------|--------|
| PostgreSQL | Railway plugin (managed) | — | No |
| API | Docker service | `api/Dockerfile` (context: repo root) | No (internal only) |
| Frontend | Docker service | `frontend/Dockerfile` (full build, `production` stage) | Yes (public domain) |

**How it works:**

- The **API Dockerfile** uses the repo root as its build context, so it can `COPY data/ ./data/` to bake GPX seed files into the image (no volume mounts on Railway).
- The **frontend Dockerfile** is multi-stage. Locally, `docker-compose` targets the `dev` stage (Vite dev server). On Railway, the full build runs through to the `production` stage, which builds static assets and serves them via nginx.
- **nginx** reverse-proxies `/api/`, `/swagger-ui/`, and `/api-docs/` requests to the API service using Railway's internal DNS (`http://api.railway.internal:<PORT>`). The `API_URL` is injected via environment variable and substituted into `nginx.conf.template` at container startup.
- The frontend API client uses **relative paths** (`/api/v1/...`), so the nginx proxy is transparent — no frontend code changes needed.

**Railway environment variables:**

| Service | Variable | Value |
|---------|----------|-------|
| API | `DATABASE_URL` | `${{Postgres.DATABASE_URL}}` (auto-injected) |
| API | `YR_USER_AGENT` | `WeatherBingo/0.1 github.com/LC-Zurich-Doppelstock/weather-bingo` |
| API | `DATA_DIR` | `/app/data` |
| Frontend | `API_URL` | `http://api.railway.internal:${{api.PORT}}` |

**Watch paths** (to avoid unnecessary rebuilds):
- API: `api/**`, `data/**`
- Frontend: `frontend/**`

---

## 9. API Contracts (JSON)

### 9.1 GET `/api/v1/races`

**Response:**
```json
[
  {
    "id": "uuid",
    "name": "Vasaloppet",
    "year": 2026,
    "start_time": "2026-03-01T08:00:00+01:00",
    "distance_km": 90.0
  }
]
```

### 9.2 GET `/api/v1/races/:id`

**Response:**
```json
{
  "id": "uuid",
  "name": "Vasaloppet",
  "year": 2026,
  "start_time": "2026-03-01T08:00:00+01:00",
  "distance_km": 90.0,
  "course_gpx": "<gpx>...</gpx>"
}
```

### 9.3 GET `/api/v1/races/:id/checkpoints`

**Response:**
```json
[
  {
    "id": "uuid",
    "name": "Smågan",
    "distance_km": 11.0,
    "latitude": 61.128,
    "longitude": 13.41,
    "elevation_m": 540,
    "sort_order": 2
  }
]
```

### 9.4 GET `/api/v1/forecasts/checkpoint/:checkpoint_id?datetime=ISO8601`

**Response:**
```json
{
  "checkpoint_id": "uuid",
  "checkpoint_name": "Smågan",
  "forecast_time": "2026-03-01T10:24:00+01:00",
  "fetched_at": "2026-02-28T14:30:00Z",
  "source": "yr.no",
  "stale": false,
  "weather": {
    "temperature_c": -4.0,
    "temperature_percentile_10_c": -6.0,
    "temperature_percentile_90_c": -2.0,
    "feels_like_c": -9.0,
    "wind_speed_ms": 3.2,
    "wind_speed_percentile_10_ms": 2.0,
    "wind_speed_percentile_90_ms": 5.1,
    "wind_direction_deg": 315,
    "wind_gust_ms": 6.8,
    "precipitation_mm": 0.4,
    "precipitation_min_mm": 0.1,
    "precipitation_max_mm": 0.8,
    "precipitation_type": "snow",
    "humidity_pct": 82,
    "dew_point_c": -6.2,
    "cloud_cover_pct": 90,
    "uv_index": 0.3,
    "symbol_code": "heavysnow"
  }
}
```

### 9.5 GET `/api/v1/forecasts/checkpoint/:checkpoint_id/history?datetime=ISO8601`

**Response:**
```json
{
  "checkpoint_id": "uuid",
  "checkpoint_name": "Smågan",
  "forecast_time": "2026-03-01T10:24:00+01:00",
  "history": [
    {
      "fetched_at": "2026-02-25T12:00:00Z",
      "weather": { "temperature_c": -2.0, "..." : "..." }
    },
    {
      "fetched_at": "2026-02-27T12:00:00Z",
      "weather": { "temperature_c": -3.5, "..." : "..." }
    },
    {
      "fetched_at": "2026-02-28T14:30:00Z",
      "weather": { "temperature_c": -4.0, "..." : "..." }
    }
  ]
}
```

### 9.6 GET `/api/v1/forecasts/race/:race_id?target_duration_hours=8`

**Response:**
```json
{
  "race_id": "uuid",
  "race_name": "Vasaloppet",
  "target_duration_hours": 8.0,
  "checkpoints": [
    {
      "checkpoint_id": "uuid",
      "name": "Berga (Start)",
      "distance_km": 0,
      "expected_time": "2026-03-01T08:00:00+01:00",
      "weather": {
        "temperature_c": -5.0,
        "temperature_percentile_10_c": -7.0,
        "temperature_percentile_90_c": -3.0,
        "feels_like_c": -10.0,
        "wind_speed_ms": 2.1,
        "wind_speed_percentile_10_ms": 1.2,
        "wind_speed_percentile_90_ms": 3.5,
        "wind_direction_deg": 315,
        "precipitation_mm": 0.2,
        "precipitation_type": "snow",
        "symbol_code": "lightsnow"
      }
    },
    {
      "checkpoint_id": "uuid",
      "name": "Sm\u00e5gan",
      "distance_km": 11,
      "expected_time": "2026-03-01T09:58:00+01:00",
      "weather": { "..." : "..." }
    }
  ]
}
```

> **Note:** The race-level endpoint includes uncertainty ranges (p10/p90 for temperature and wind) to support the CourseOverview shaded band charts. Percentile fields are nullable — they may be absent for long-range forecasts.

---

## 10. Pacing Model

### 10.1 Current: Even Pacing

```
pass_through_time(checkpoint) =
  race.start_time + target_duration × (checkpoint.distance_km / race.distance_km)
```

### 10.2 Future: Elevation-Adjusted Pacing

A future improvement will adjust pacing based on elevation profile from GPX data, allocating more time to uphill segments and less to downhill. The data model already stores `elevation_m` per checkpoint to support this.

---

## 11. Non-Functional Requirements

| Requirement      | Target                                                  |
| ---------------- | ------------------------------------------------------- |
| Response time    | API < 500ms (cached), < 3s (yr.no fetch)                |
| Mobile support   | Fully responsive, mobile-first                          |
| Browser support  | Latest 2 versions of Chrome, Firefox, Safari, Edge      |
| Accessibility    | Semantic HTML, ARIA labels, keyboard navigation         |
| Language         | English only                                            |
| Authentication   | None (public app)                                       |
| yr.no compliance | Proper User-Agent, respect caching headers              |

---

## 12. Future Enhancements (Out of Scope for v1)

- [ ] Elevation-adjusted pacing model
- [ ] Forecast interpolation (sub-hourly granularity)
- [ ] Additional races (Birkebeinerrennet, Marcialonga, Engadin Skimarathon, …)
- [ ] Historical weather source for past seasons
- [ ] PWA / offline support
- [ ] Push notifications for significant forecast changes
- [ ] E2E tests with Playwright
- [ ] CI/CD pipeline
- [ ] ~~Cloud deployment~~ (Railway — see §8.3)

---

## 13. Open Items & Action Items

| #  | Item                                     | Status      |
| -- | ---------------------------------------- | ----------- |
| 1  | Colour scheme derived from tropical botanical artwork    | Done |
| 2  | GPX track for Vasaloppet 2026 with `wb:` namespace extensions for race metadata and checkpoint distances | Done |
| 3  | Checkpoint coordinates sourced from Wikipedia track profile. Verify against official race data when available. | Done (mock) |
| 4  | Historical weather data source: yr.no thredds server (NetCDF, Nordic region) | Documented (future) |
| 5  | yr.no API review: feels-like must be calculated, uncertainty via percentiles, precip type inferred from symbol_code | Done |
| 6  | GPX-based startup seeding with `wb:` namespace, upsert logic, `DATA_DIR` config | Done |
| 7  | OpenAPI / Swagger UI documentation via `utoipa` | Done |
| 8  | p10/p90 uncertainty bands in CourseOverview charts and race forecast API | Done |
| 9  | Railway cloud deployment: multi-stage frontend Dockerfile, nginx reverse proxy, repo-root API build context | Done |
