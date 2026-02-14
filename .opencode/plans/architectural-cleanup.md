# Architectural Rethink + Full Cleanup Plan

## Overview

This plan covers a comprehensive refactor of the Weather Bingo codebase, addressing bugs, dead code, architectural issues, and frontend cleanup identified during holistic review. Organized into 5 sequential phases, each producing a clean commit.

**Key design decisions:**
- **Extract-on-read:** Read forecast data from yr_responses cache directly, don't require pre-inserted `forecasts` rows
- **Single weather type:** Eliminate `RaceWeatherSimple` / `ForecastWeather` duplication
- **Server-only pacing:** Remove client-side pacing code, use API's `expected_time`
- **Deduplication on write:** `ON CONFLICT DO NOTHING` on `(checkpoint_id, forecast_time, yr_model_run_at)` — no duplicate forecast rows

---

## Phase 1: API Bug Fixes & Safety

No behavior change from the user's perspective. Fixes production risks and removes dead code.

### 1a. Replace `unwrap()` panics with proper error handling

**File: `api/src/services/forecast.rs`**

At **line 535**, replace:
```rust
return Ok(results.into_iter().map(|r| r.unwrap()).collect());
```
With:
```rust
return results
    .into_iter()
    .enumerate()
    .map(|(i, r)| {
        r.ok_or_else(|| {
            AppError::InternalError(format!(
                "Missing resolved forecast for checkpoint index {}", i
            ))
        })
    })
    .collect();
```

At **line 624**, replace:
```rust
Ok(results.into_iter().map(|r| r.unwrap()).collect())
```
With the same pattern.

### 1b. Add HTTP timeout to yr.no client

**File: `api/src/services/yr.rs` line 174**

Replace:
```rust
let client = reqwest::Client::builder()
    .build()
    .expect("Failed to build HTTP client");
```
With:
```rust
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(30))
    .build()
    .expect("Failed to build HTTP client");
```

### 1c. Add `X-Forecast-Stale` header to race endpoint

**File: `api/src/routes/forecasts.rs`**

Change `get_race_forecast` return type from `Result<Json<RaceForecastResponse>, AppError>` to `Result<(HeaderMap, Json<RaceForecastResponse>), AppError>`.

After building `checkpoint_forecasts`, check if any `resolved` has `is_stale == true`:
```rust
let any_stale = resolved.iter().any(|r| r.is_stale);
let mut headers = HeaderMap::new();
if any_stale {
    headers.insert("X-Forecast-Stale", "true".parse().unwrap());
}
Ok((headers, Json(response)))
```

### 1d. Remove dead code

**File: `api/src/db/queries.rs`** — Delete:
- `forecast_exists_for_model_run` (lines 421-452) — dead code, stale comment references "bulk-insert architecture"
- `bulk_insert_forecasts` (lines 511-584) — dead code, replaced by `insert_forecast`

**File: `api/src/services/yr.rs`** — Delete:
- `extract_all_forecasts` (lines 360-401) — dead code, replaced by `extract_forecasts_at_times`
- `extract_forecast_at_time` (lines 263-280) — dead code outside tests

For `extract_forecast_at_time`: it's used by tests. Move it into the `#[cfg(test)]` module instead of having it as `#[allow(dead_code)]` in production code.

Also delete the associated tests for `extract_all_forecasts`:
- `test_extract_all_forecasts` (lines 716-807)
- `test_extract_all_forecasts_empty` (lines 809-820)

### 1e. Fix `IntoResponse` to avoid unnecessary String clones

**File: `api/src/errors.rs` line 33**

Change:
```rust
let (status, message) = match &self {
    AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
    AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
    AppError::ExternalServiceError(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
    AppError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
```
To:
```rust
let (status, message) = match self {
    AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
    AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
    AppError::ExternalServiceError(msg) => (StatusCode::BAD_GATEWAY, msg),
    AppError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
```
(Takes ownership instead of cloning)

### 1f. Remove `Deserialize` derive from `YrCachedResponse`

**File: `api/src/db/models.rs` line 9**

Remove `Deserialize` from the derive list — it's never deserialized from JSON.

### 1g. Add `LIMIT 1` to `get_yr_cached_response_any`

**File: `api/src/db/queries.rs` line 204-208**

Add `LIMIT 1` to the SQL query for defensive coding.

### Verification
- `cargo test` — all tests pass (count should decrease by 2 from removed extract_all tests)
- `cargo clippy` — no warnings (dead_code warning for `is_stale` will be fixed in next phase)

---

## Phase 2: Extract-on-Read Architecture

The core refactor. Changes how forecast data flows from yr.no cache to API responses.

### Current flow (extract-on-write):
```
request → check forecasts table → if stale, fetch yr.no → insert into forecasts → query forecasts table → respond
```

### New flow (extract-on-read):
```
request → ensure yr.no cache fresh → read raw_response from yr_responses → extract in-memory → respond
          └→ also write to forecasts table for history (ON CONFLICT DO NOTHING)
```

### 2a. Split `ensure_yr_timeseries` into two functions

**File: `api/src/services/forecast.rs`**

Replace `ensure_yr_timeseries` (lines 224-323) with:

```rust
/// Ensure the yr.no cache is fresh for a location. Does NOT extract forecasts.
/// Returns the cached raw_response (fresh or just-fetched).
async fn ensure_yr_cache_fresh(
    pool: &PgPool,
    yr_client: &YrClient,
    lat_dec: Decimal,
    lon_dec: Decimal,
    ele_dec: Decimal,
) -> Result<serde_json::Value, AppError> {
    // 1. Check for a non-expired cached response
    if let Some(cached) = queries::get_yr_cached_response(pool, lat_dec, lon_dec, ele_dec).await? {
        return Ok(cached.raw_response);
    }

    // 2. Cache miss or expired — conditional request with If-Modified-Since
    let existing = queries::get_yr_cached_response_any(pool, lat_dec, lon_dec, ele_dec).await?;
    let if_modified_since = existing.as_ref().and_then(|c| c.last_modified.as_deref());

    let lat = lat_dec.to_f64().unwrap_or(0.0);
    let lon = lon_dec.to_f64().unwrap_or(0.0);
    let alt = ele_dec.to_f64().unwrap_or(0.0);

    match yr_client.fetch_timeseries(lat, lon, alt, if_modified_since).await? {
        YrTimeseriesResult::NewData { raw_json, expires, last_modified } => {
            let expires_at = expires.as_deref()
                .map(parse_expires_header)
                .unwrap_or_else(|| Utc::now() + Duration::hours(1));

            queries::upsert_yr_cached_response(
                pool, lat_dec, lon_dec, ele_dec,
                Utc::now(), expires_at, last_modified.as_deref(), &raw_json,
            ).await?;

            Ok(raw_json)
        }
        YrTimeseriesResult::NotModified => {
            if let Some(cached) = existing {
                // Just bump expires_at — don't re-write the full JSON blob
                let new_expires = Utc::now() + Duration::hours(1);
                queries::update_yr_cache_expiry(pool, lat_dec, lon_dec, ele_dec, new_expires).await?;
                Ok(cached.raw_response)
            } else {
                Err(AppError::ExternalServiceError(
                    "yr.no returned 304 but no cached data exists".to_string(),
                ))
            }
        }
    }
}
```

### 2b. Add `update_yr_cache_expiry` query (lightweight expiry bump)

**File: `api/src/db/queries.rs`** — Add new function:

```rust
/// Update only the expires_at on an existing yr.no cached response.
/// Much cheaper than upserting the full raw_response JSON blob.
pub async fn update_yr_cache_expiry(
    pool: &PgPool,
    latitude: Decimal,
    longitude: Decimal,
    elevation_m: Decimal,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE yr_responses SET expires_at = $4
         WHERE latitude = $1 AND longitude = $2 AND elevation_m = $3"
    )
    .bind(latitude)
    .bind(longitude)
    .bind(elevation_m)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}
```

### 2c. Update `insert_forecast` to use `ON CONFLICT DO NOTHING`

**File: `api/src/db/queries.rs`**

Change `insert_forecast` SQL from plain INSERT to:
```sql
INSERT INTO forecasts (...) VALUES (...)
ON CONFLICT (checkpoint_id, forecast_time, yr_model_run_at)
    WHERE yr_model_run_at IS NOT NULL
DO NOTHING
RETURNING ...
```

Change return type from `Result<Forecast, sqlx::Error>` to `Result<Option<Forecast>, sqlx::Error>` and use `fetch_optional` instead of `fetch_one` (since ON CONFLICT DO NOTHING returns nothing when skipped).

### 2d. Rewrite `resolve_forecast` (single checkpoint)

**File: `api/src/services/forecast.rs`**

```rust
pub async fn resolve_forecast(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoint: &Checkpoint,
    forecast_time: DateTime<Utc>,
    staleness_secs: u64,
) -> Result<(Option<Forecast>, bool), AppError> {
    // Try to get fresh yr.no data
    let raw_json = match ensure_yr_cache_fresh(
        pool, yr_client,
        checkpoint.latitude, checkpoint.longitude, checkpoint.elevation_m,
    ).await {
        Ok(json) => json,
        Err(e) => {
            // yr.no failed — fall back to cached forecast from DB
            let cached = queries::get_latest_forecast(pool, checkpoint.id, forecast_time).await?;
            if let Some(forecast) = cached {
                tracing::warn!("yr.no unavailable, returning stale data: {}", e);
                return Ok((Some(forecast), true));
            }
            return Err(AppError::ExternalServiceError(format!(
                "yr.no unavailable and no cached data: {}", e
            )));
        }
    };

    // Extract forecast from cached JSON (extract-on-read)
    let parsed = extract_forecasts_at_times(&raw_json, &[forecast_time])?;
    let maybe_parsed = parsed.into_iter().next().flatten();

    match maybe_parsed {
        Some(ref forecast) => {
            // Write to forecasts table for history tracking (ON CONFLICT DO NOTHING)
            let params = build_single_insert_params(checkpoint.id, forecast, Utc::now());
            let _ = queries::insert_forecast(pool, params).await?;

            // Build Forecast from parsed data (or query from DB for consistency)
            let db_forecast = queries::get_latest_forecast(pool, checkpoint.id, forecast_time).await?;
            Ok((db_forecast, false))
        }
        None => {
            // Beyond yr.no horizon — no forecast available
            Ok((None, false))
        }
    }
}
```

This fixes the cache-valid-but-no-extracted-forecast bug: we always extract from the cached JSON, regardless of whether the cache was already valid or just fetched.

### 2e. Rewrite `resolve_race_forecasts` (batch)

Similar pattern but batched:

1. Collect unique locations from checkpoints
2. `ensure_yr_cache_fresh` for each location (parallel)
3. Extract forecasts from cached JSON for all checkpoints (in-memory)
4. Write to forecasts table for history (ON CONFLICT DO NOTHING, batch)
5. Build results

Key changes:
- Step 5 (re-query DB) goes away — we build results from in-memory parsed data
- Remove `is_stale` from `ResolvedForecast` — the race handler checks if any location fetch errored
- Fix N+1: no more per-checkpoint DB queries after fetch

### 2f. Remove `ResolvedForecast.is_stale`

Since we're rewriting `resolve_race_forecasts`, change the return type. The function returns `Vec<Option<Forecast>>` (None = not available, Some = available). Staleness is communicated separately: if any yr.no fetch failed but stale DB cache exists, the function returns the stale data + sets a flag.

New return type:
```rust
pub struct RaceForecasts {
    pub forecasts: Vec<Option<Forecast>>,
    pub any_stale: bool,
}
```

### 2g. Pass ownership of JSON to `extract_forecasts_at_times`

**File: `api/src/services/yr.rs`**

Change signature from:
```rust
pub fn extract_forecasts_at_times(
    raw_json: &serde_json::Value,
```
To:
```rust
pub fn extract_forecasts_at_times(
    raw_json: serde_json::Value,
```

Remove the `.clone()` at line 296 (`serde_json::from_value(raw_json.clone())`).

Update callers to pass ownership. Where callers need the JSON for history write too, clone at the call site (but most callers don't need it after extraction).

### Verification
- `cargo test` — all tests pass
- `cargo clippy` — clean (no more dead_code warning for `is_stale`)
- Manual test: add a new checkpoint at existing location, verify forecast is available during valid cache window

---

## Phase 3: Unified Weather Type

### 3a. Create single `Weather` struct

**File: `api/src/routes/forecasts.rs`**

Replace `ForecastWeather` and `RaceWeatherSimple` with:

```rust
#[derive(Debug, Serialize, ToSchema)]
pub struct Weather {
    pub temperature_c: f64,
    pub temperature_percentile_10_c: Option<f64>,
    pub temperature_percentile_90_c: Option<f64>,
    pub feels_like_c: f64,
    pub wind_speed_ms: f64,
    pub wind_speed_percentile_10_ms: Option<f64>,
    pub wind_speed_percentile_90_ms: Option<f64>,
    pub wind_direction_deg: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wind_gust_ms: Option<f64>,
    pub precipitation_mm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precipitation_min_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precipitation_max_mm: Option<f64>,
    pub precipitation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub humidity_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dew_point_c: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_cover_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uv_index: Option<f64>,
    pub symbol_code: String,
}
```

For checkpoint detail: all fields present → `skip_serializing_if` doesn't trigger.
For race overview: set `wind_gust_ms`, `precipitation_min_mm`, `precipitation_max_mm`, `humidity_pct`, `dew_point_c`, `cloud_cover_pct`, `uv_index` to `None` → they're omitted from JSON.

### 3b. Add two `From` constructors

```rust
impl Weather {
    /// Full weather from a forecast (checkpoint detail view)
    pub fn full(f: &Forecast) -> Self { ... }

    /// Simplified weather for race overview (omits detail-only fields)
    pub fn simplified(f: &Forecast) -> Self {
        Self {
            // ... same required fields ...
            wind_gust_ms: None,           // omitted in race view
            precipitation_min_mm: None,   // omitted in race view
            precipitation_max_mm: None,   // omitted in race view
            humidity_pct: None,           // omitted in race view
            dew_point_c: None,            // omitted in race view
            cloud_cover_pct: None,        // omitted in race view
            uv_index: None,              // omitted in race view
            // ... rest ...
        }
    }
}
```

### 3c. Update all response structs

Replace `ForecastWeather` with `Weather` in:
- `ForecastResponse.weather`
- `ForecastHistoryEntry.weather`
- `RaceForecastCheckpoint.weather` (was `RaceWeatherSimple`)

### 3d. Update frontend types

**File: `frontend/src/api/types.ts`**

Replace inline weather type in `RaceForecastCheckpoint` with `ForecastWeather | null`.

The extra fields (`wind_gust_ms`, `humidity_pct`, etc.) won't be present in the race response JSON (skipped by serde), so they'll be `undefined` in TypeScript. Adjust the interface: make the detail-only fields `number | null | undefined` (or keep them optional with `?`).

Actually simpler: just make the race checkpoint weather type reference `ForecastWeather` and make the detail-only fields optional (`?:` instead of `:`).

### Verification
- `cargo test` — passes
- `npx tsc --noEmit` — passes
- Manual test: race forecast response only includes the 11 required fields, checkpoint detail includes all 18

---

## Phase 4: Frontend Cleanup

### 4a. Remove client-side pacing

**Delete:** `frontend/src/utils/pacing.ts`
**Delete:** `frontend/src/hooks/usePassThroughTime.ts`
**Delete:** Tests: `frontend/src/utils/pacing.test.ts`

**Update `Sidebar.tsx`:** Instead of computing pass-through time client-side, pass the `expected_time` from the race forecast response to `CheckpointDetail` for the datetime query parameter.

This requires the Sidebar to have access to the race forecast response's checkpoint data. Flow:
- `CourseOverview` already receives `raceForecast` which has `checkpoints[].expected_time`
- When user selects a checkpoint, `Sidebar` looks up the `expected_time` from `raceForecast.checkpoints` for that checkpoint ID
- Passes that datetime to `useForecast` instead of `usePassThroughTime`

### 4b. Add error state handling

**File: `frontend/src/components/Sidebar/Sidebar.tsx`**

Destructure `isError` and `error` from `useForecast` and `useRaceForecast`. Add error UI:
```tsx
if (isError) {
  return (
    <div className="text-error p-4">
      <p>Failed to load forecast data.</p>
      <button onClick={() => refetch()}>Retry</button>
    </div>
  );
}
```

**File: `frontend/src/components/Sidebar/CourseOverview.tsx`**

Similar — handle error state from the race forecast hook. Show "Failed to load" instead of infinite "Loading...".

### 4c. Replace hardcoded colors

**File: `frontend/src/components/Sidebar/CourseOverview.tsx`**

Replace:
```ts
backgroundColor: "#171614",
border: "1px solid #2C2A27",
color: "#F0EEEB",
```
With:
```ts
backgroundColor: colors.surface,
border: `1px solid ${colors.border}`,
color: colors.textPrimary,
```

Replace hardcoded `"#6B6762"` in XAxis/YAxis tick fills with `colors.textMuted`.

### 4d. Remove dead code

- `frontend/src/utils/formatting.ts` — Remove `formatDistance` (unused, lines 21-23)
- `frontend/src/components/Map/RaceMap.tsx` — Remove unused `mapRef` (line 67)
- `frontend/src/styles/theme.ts` — Remove unused `chartColors.humidity` and `chartColors.cloudCover` (lines 33-34)

### 4e. Fix `Checkpoint` type

**File: `frontend/src/api/types.ts`**

Remove `race_id` from `Checkpoint` interface (line 20) — the API doesn't send it.

### 4f. Fix `CourseOverview` non-null assertions

Replace:
```tsx
{raceForecast!.race_name}
```
With early return / guard:
```tsx
if (!raceForecast) return null;
// Now raceForecast is narrowed
{raceForecast.race_name}
```

### 4g. Accessibility improvements

- Add `role="img"` and `aria-label` to chart container divs in `CourseOverview` and `MiniTimeline`
- Add `aria-valuetext={formatDuration(value)}` to TargetTimeInput slider
- Add `aria-busy="true"` and `role="status"` to CourseOverview loading skeleton (matching CheckpointDetail pattern)

### 4h. Remove `calculateAllPassTimes` from pacing.ts

Already handled by 4a (deleting the whole file).

### Verification
- `npx tsc --noEmit` — passes
- `npx eslint src/` — passes
- `npx vitest run` — passes (some tests will need updating since pacing utils are removed)

---

## Phase 5: Tests & Docs

### 5a. Fix Sidebar test mocks

**File: `frontend/src/components/Sidebar/Sidebar.test.tsx`**

Add `forecast_available: true` to `mockRaceForecast.checkpoints` and `mockForecast`.

### 5b. Add unit tests for CourseOverview

**New file: `frontend/src/components/Sidebar/CourseOverview.test.tsx`**

Test scenarios:
- Renders loading skeletons when data is loading
- Renders "no forecast available" when all checkpoints have `forecast_available: false`
- Renders charts when data is present
- Renders partial data warning when some checkpoints have unavailable forecasts
- Handles error state from race forecast hook

### 5c. Expand API unit tests

**File: `api/src/services/forecast.rs`** — Add tests for:
- `build_single_insert_params` with various yr.no data shapes
- `calculate_pass_time_fractions` edge cases (single checkpoint, all same elevation)

### 5d. Fix text-muted contrast

**File: `frontend/src/index.css`**

Change `--color-text-muted: #6B6762` to `--color-text-muted: #8A8580` (meets WCAG AA 4.5:1).

**File: `frontend/src/styles/theme.ts`**

Update `textMuted` to match: `"#8A8580"`.

### 5e. Update specs.md

- Section 9.2: Update to reflect `/api/v1/races/{id}/course` returning `CoursePoint[]` instead of `course_gpx`
- Add note about extract-on-read architecture in section 4
- Add note about `ON CONFLICT DO NOTHING` deduplication in section 3

### Verification
- `cargo test` — all pass
- `cargo clippy` — clean
- `npx tsc --noEmit` — clean
- `npx eslint src/` — clean
- `npx vitest run` — all pass

---

## Files Modified (Summary)

### API
| File | Phase | Changes |
|------|-------|---------|
| `services/forecast.rs` | 1, 2 | Replace unwraps, rewrite resolve_forecast/resolve_race_forecasts, split ensure_yr_timeseries |
| `services/yr.rs` | 1, 2 | Add timeout, remove dead code, change extract_forecasts_at_times to take ownership |
| `routes/forecasts.rs` | 1, 3 | Add stale header to race, unified Weather type |
| `db/queries.rs` | 1, 2 | Remove dead code, add update_yr_cache_expiry, ON CONFLICT DO NOTHING |
| `db/models.rs` | 1 | Remove unused Deserialize derive |
| `errors.rs` | 1 | Take ownership in IntoResponse |

### Frontend
| File | Phase | Changes |
|------|-------|---------|
| `api/types.ts` | 3, 4 | Unified Weather type, remove race_id from Checkpoint |
| `utils/pacing.ts` | 4 | DELETE |
| `utils/pacing.test.ts` | 4 | DELETE |
| `hooks/usePassThroughTime.ts` | 4 | DELETE |
| `components/Sidebar/Sidebar.tsx` | 4 | Error states, use expected_time from race response |
| `components/Sidebar/CourseOverview.tsx` | 4 | Error states, fix colors, fix non-null assertions, a11y |
| `components/Sidebar/MiniTimeline.tsx` | 4 | a11y improvements |
| `components/Controls/TargetTimeInput.tsx` | 4 | aria-valuetext |
| `components/Map/RaceMap.tsx` | 4 | Remove unused mapRef |
| `styles/theme.ts` | 4, 5 | Remove unused colors, fix text-muted |
| `utils/formatting.ts` | 4 | Remove formatDistance |
| `index.css` | 5 | Fix text-muted contrast |
| `Sidebar.test.tsx` | 5 | Fix mock data |
| `CourseOverview.test.tsx` | 5 | NEW — test suite |

### Docs
| File | Phase | Changes |
|------|-------|---------|
| `specs.md` | 5 | Update race detail endpoint, architecture notes |
