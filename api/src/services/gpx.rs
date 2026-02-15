//! GPX file parser for race and checkpoint data.
//!
//! Reads GPX files with Weather Bingo extensions (`wb:` namespace) to extract:
//! - Race metadata: name, year, start_time, distance_km
//! - Checkpoints: waypoints with `<type>checkpoint</type>` and `<wb:distance_km>`
//! - Full GPX XML for storage in the database

use chrono::{DateTime, FixedOffset};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use std::path::Path;
use thiserror::Error;
use utoipa::ToSchema;

/// Errors that can occur during GPX parsing.
#[derive(Debug, Error)]
pub enum GpxError {
    #[error("IO error reading GPX file: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid field value for '{field}': {message}")]
    InvalidValue { field: String, message: String },
}

/// Parsed race data from a GPX file.
#[derive(Debug, Clone)]
pub struct GpxRace {
    /// Race name from `<metadata><name>`
    pub name: String,
    /// Race year from `<wb:year>`
    pub year: i32,
    /// Race start time from `<wb:start_time>`
    pub start_time: DateTime<FixedOffset>,
    /// Total race distance in km from `<wb:distance_km>`
    pub distance_km: f64,
    /// Checkpoints extracted from `<wpt>` elements with `<type>checkpoint</type>`
    pub checkpoints: Vec<GpxCheckpoint>,
    /// The full GPX XML content (for storage in DB)
    pub gpx_xml: String,
}

/// A checkpoint parsed from a GPX waypoint.
#[derive(Debug, Clone)]
pub struct GpxCheckpoint {
    /// Checkpoint name from `<name>`
    pub name: String,
    /// Latitude from `lat` attribute
    pub latitude: f64,
    /// Longitude from `lon` attribute
    pub longitude: f64,
    /// Elevation in metres from `<ele>`
    pub elevation_m: f64,
    /// Distance from start in km from `<wb:distance_km>`
    pub distance_km: f64,
}

/// Parse a GPX file from disk and extract race + checkpoint data.
pub fn parse_gpx_file(path: &Path) -> Result<GpxRace, GpxError> {
    let gpx_xml = std::fs::read_to_string(path)?;
    parse_gpx(&gpx_xml)
}

/// Parse GPX XML content and extract race + checkpoint data.
pub fn parse_gpx(gpx_xml: &str) -> Result<GpxRace, GpxError> {
    let mut reader = Reader::from_str(gpx_xml);

    let mut race_name: Option<String> = None;
    let mut race_year: Option<i32> = None;
    let mut race_start_time: Option<DateTime<FixedOffset>> = None;
    let mut race_distance_km: Option<f64> = None;

    let mut checkpoints: Vec<GpxCheckpoint> = Vec::new();

    // Current waypoint state (while inside a <wpt> element)
    let mut in_wpt = false;
    let mut wpt_lat: f64 = 0.0;
    let mut wpt_lon: f64 = 0.0;
    let mut wpt_name: Option<String> = None;
    let mut wpt_ele: Option<f64> = None;
    let mut wpt_type: Option<String> = None;
    let mut wpt_distance_km: Option<f64> = None;

    // Track nesting context
    let mut in_metadata = false;
    let mut in_metadata_extensions = false;
    let mut in_wb_race = false;
    let mut in_wpt_extensions = false;
    let mut in_author = false;

    // Current element name (for capturing text content)
    let mut current_element: Option<String> = None;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local_name = local_name_str(e.name().as_ref());

                match local_name.as_str() {
                    "metadata" => {
                        in_metadata = true;
                    }
                    "extensions" if in_metadata && !in_wpt => {
                        in_metadata_extensions = true;
                    }
                    "extensions" if in_wpt => {
                        in_wpt_extensions = true;
                    }
                    "race" if in_metadata_extensions => {
                        in_wb_race = true;
                    }
                    "author" if in_metadata => {
                        in_author = true;
                    }
                    "name"
                        if in_metadata && !in_wb_race && !in_metadata_extensions && !in_author =>
                    {
                        current_element = Some("metadata_name".to_string());
                    }
                    "year" if in_wb_race => {
                        current_element = Some("wb_year".to_string());
                    }
                    "start_time" if in_wb_race => {
                        current_element = Some("wb_start_time".to_string());
                    }
                    "distance_km" if in_wb_race => {
                        current_element = Some("wb_distance_km".to_string());
                    }
                    "wpt" => {
                        in_wpt = true;
                        wpt_name = None;
                        wpt_ele = None;
                        wpt_type = None;
                        wpt_distance_km = None;
                        // Extract lat/lon attributes
                        for attr in e.attributes().flatten() {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let val = std::str::from_utf8(&attr.value).unwrap_or("");
                            match key {
                                "lat" => {
                                    wpt_lat = val.parse().unwrap_or_else(|e| {
                                        tracing::warn!(
                                            "Malformed wpt lat='{}': {}, defaulting to 0.0",
                                            val,
                                            e,
                                        );
                                        0.0
                                    });
                                }
                                "lon" => {
                                    wpt_lon = val.parse().unwrap_or_else(|e| {
                                        tracing::warn!(
                                            "Malformed wpt lon='{}': {}, defaulting to 0.0",
                                            val,
                                            e,
                                        );
                                        0.0
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    "name" if in_wpt && !in_wpt_extensions => {
                        current_element = Some("wpt_name".to_string());
                    }
                    "ele" if in_wpt => {
                        current_element = Some("wpt_ele".to_string());
                    }
                    "type" if in_wpt && !in_wpt_extensions => {
                        current_element = Some("wpt_type".to_string());
                    }
                    "distance_km" if in_wpt_extensions => {
                        current_element = Some("wpt_distance_km".to_string());
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if let Some(ref elem) = current_element {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if !text.is_empty() {
                        match elem.as_str() {
                            "metadata_name" => race_name = Some(text),
                            "wb_year" => {
                                race_year =
                                    Some(text.parse().map_err(|_| GpxError::InvalidValue {
                                        field: "wb:year".to_string(),
                                        message: format!("not a valid integer: '{}'", text),
                                    })?);
                            }
                            "wb_start_time" => {
                                race_start_time =
                                    Some(DateTime::parse_from_rfc3339(&text).map_err(|e| {
                                        GpxError::InvalidValue {
                                            field: "wb:start_time".to_string(),
                                            message: format!(
                                                "not valid RFC3339: '{}' ({})",
                                                text, e
                                            ),
                                        }
                                    })?);
                            }
                            "wb_distance_km" => {
                                race_distance_km =
                                    Some(text.parse().map_err(|_| GpxError::InvalidValue {
                                        field: "wb:distance_km".to_string(),
                                        message: format!("not a valid number: '{}'", text),
                                    })?);
                            }
                            "wpt_name" => wpt_name = Some(text),
                            "wpt_ele" => wpt_ele = Some(text.parse().unwrap_or(0.0)),
                            "wpt_type" => wpt_type = Some(text),
                            "wpt_distance_km" => {
                                wpt_distance_km =
                                    Some(text.parse().map_err(|_| GpxError::InvalidValue {
                                        field: "wb:distance_km (waypoint)".to_string(),
                                        message: format!("not a valid number: '{}'", text),
                                    })?);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local_name = local_name_str(e.name().as_ref());
                current_element = None;

                match local_name.as_str() {
                    "metadata" => {
                        in_metadata = false;
                    }
                    "author" if in_author => {
                        in_author = false;
                    }
                    "extensions" if in_metadata_extensions && !in_wpt => {
                        in_metadata_extensions = false;
                    }
                    "extensions" if in_wpt_extensions => {
                        in_wpt_extensions = false;
                    }
                    "race" if in_wb_race => {
                        in_wb_race = false;
                    }
                    "wpt" => {
                        // Finalize waypoint — only include if type == "checkpoint"
                        if wpt_type.as_deref() == Some("checkpoint") {
                            let name = wpt_name.take().ok_or_else(|| {
                                GpxError::MissingField("waypoint <name> for checkpoint".to_string())
                            })?;
                            let distance_km = wpt_distance_km.ok_or_else(|| {
                                GpxError::MissingField(format!(
                                    "wb:distance_km for checkpoint '{}'",
                                    name
                                ))
                            })?;
                            checkpoints.push(GpxCheckpoint {
                                name,
                                latitude: wpt_lat,
                                longitude: wpt_lon,
                                elevation_m: wpt_ele.unwrap_or(0.0),
                                distance_km,
                            });
                        }
                        in_wpt = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(GpxError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    let name = race_name.ok_or_else(|| GpxError::MissingField("metadata/name".to_string()))?;
    let year = race_year.ok_or_else(|| GpxError::MissingField("wb:year".to_string()))?;
    let start_time =
        race_start_time.ok_or_else(|| GpxError::MissingField("wb:start_time".to_string()))?;
    let distance_km =
        race_distance_km.ok_or_else(|| GpxError::MissingField("wb:distance_km".to_string()))?;

    if checkpoints.is_empty() {
        return Err(GpxError::MissingField(
            "at least one waypoint with <type>checkpoint</type>".to_string(),
        ));
    }

    Ok(GpxRace {
        name,
        year,
        start_time,
        distance_km,
        checkpoints,
        gpx_xml: gpx_xml.to_string(),
    })
}

/// A single coordinate point along the race course.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CoursePoint {
    /// Latitude (WGS84)
    pub lat: f64,
    /// Longitude (WGS84)
    pub lon: f64,
    /// Elevation in metres above sea level
    pub ele: f64,
}

/// Extract track points from GPX XML as `[{lat, lon, ele}]` coordinates.
///
/// Reads `<trkpt>` elements from `<trkseg>` sections, extracting the `lat`/`lon`
/// attributes and nested `<ele>` element. Points without elevation default to 0.
pub fn extract_track_points(gpx_xml: &str) -> Result<Vec<CoursePoint>, GpxError> {
    let mut reader = Reader::from_str(gpx_xml);
    let mut points = Vec::new();

    let mut in_trkpt = false;
    let mut trkpt_lat: f64 = 0.0;
    let mut trkpt_lon: f64 = 0.0;
    let mut trkpt_ele: Option<f64> = None;
    let mut reading_ele = false;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = local_name_str(e.name().as_ref());
                match local.as_str() {
                    "trkpt" => {
                        in_trkpt = true;
                        trkpt_ele = None;
                        for attr in e.attributes().flatten() {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let val = std::str::from_utf8(&attr.value).unwrap_or("");
                            match key {
                                "lat" => {
                                    trkpt_lat = val.parse().unwrap_or_else(|e| {
                                        tracing::warn!(
                                            "Malformed trkpt lat='{}': {}, defaulting to 0.0",
                                            val,
                                            e,
                                        );
                                        0.0
                                    });
                                }
                                "lon" => {
                                    trkpt_lon = val.parse().unwrap_or_else(|e| {
                                        tracing::warn!(
                                            "Malformed trkpt lon='{}': {}, defaulting to 0.0",
                                            val,
                                            e,
                                        );
                                        0.0
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    "ele" if in_trkpt => {
                        reading_ele = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if reading_ele {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if !text.is_empty() {
                        trkpt_ele = Some(text.parse().unwrap_or(0.0));
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name_str(e.name().as_ref());
                match local.as_str() {
                    "ele" if in_trkpt => {
                        reading_ele = false;
                    }
                    "trkpt" => {
                        points.push(CoursePoint {
                            lat: trkpt_lat,
                            lon: trkpt_lon,
                            ele: trkpt_ele.unwrap_or(0.0),
                        });
                        in_trkpt = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(GpxError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(points)
}

/// Extract the local name from a potentially namespaced XML element name.
/// e.g. `{http://...}name` -> `name`, `wb:name` -> `name`, `name` -> `name`
fn local_name_str(full: &[u8]) -> String {
    let s = std::str::from_utf8(full).unwrap_or("");
    // Handle `prefix:local` (namespace prefix)
    if let Some(pos) = s.rfind(':') {
        return s[pos + 1..].to_string();
    }
    // Handle `{uri}local` (expanded name, unlikely with quick-xml but defensive)
    if let Some(pos) = s.rfind('}') {
        return s[pos + 1..].to_string();
    }
    s.to_string()
}

/// Scan a directory for `*.gpx` files and parse each one.
pub fn load_races_from_dir(dir: &Path) -> Result<Vec<GpxRace>, GpxError> {
    let mut races = Vec::new();
    if !dir.exists() {
        tracing::warn!("Data directory does not exist: {}", dir.display());
        return Ok(races);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "gpx") {
            tracing::info!("Loading race from GPX: {}", path.display());
            match parse_gpx_file(&path) {
                Ok(race) => {
                    tracing::info!(
                        "  Parsed race '{}' ({}) with {} checkpoints",
                        race.name,
                        race.year,
                        race.checkpoints.len()
                    );
                    races.push(race);
                }
                Err(e) => {
                    tracing::error!("  Failed to parse {}: {}", path.display(), e);
                }
            }
        }
    }
    Ok(races)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_GPX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1"
     xmlns:wb="https://github.com/LC-Zurich-Doppelstock/weather-bingo/gpx"
     version="1.1" creator="test">
  <metadata>
    <name>Test Race</name>
    <extensions>
      <wb:race>
        <wb:year>2026</wb:year>
        <wb:start_time>2026-03-01T08:00:00+01:00</wb:start_time>
        <wb:distance_km>50</wb:distance_km>
      </wb:race>
    </extensions>
  </metadata>
  <wpt lat="61.1" lon="13.3">
    <ele>350</ele>
    <name>Start</name>
    <type>checkpoint</type>
    <extensions>
      <wb:distance_km>0</wb:distance_km>
    </extensions>
  </wpt>
  <wpt lat="61.0" lon="14.5">
    <ele>165</ele>
    <name>Finish</name>
    <type>checkpoint</type>
    <extensions>
      <wb:distance_km>50</wb:distance_km>
    </extensions>
  </wpt>
  <wpt lat="61.05" lon="13.9">
    <ele>200</ele>
    <name>Scenic Viewpoint</name>
    <type>poi</type>
  </wpt>
  <trk><name>Test</name><trkseg>
    <trkpt lat="61.1" lon="13.3"><ele>350</ele></trkpt>
    <trkpt lat="61.0" lon="14.5"><ele>165</ele></trkpt>
  </trkseg></trk>
</gpx>"#;

    #[test]
    fn test_parse_race_metadata() {
        let race = parse_gpx(MINIMAL_GPX).unwrap();
        assert_eq!(race.name, "Test Race");
        assert_eq!(race.year, 2026);
        assert_eq!(race.distance_km, 50.0);
        assert_eq!(race.start_time.to_rfc3339(), "2026-03-01T08:00:00+01:00");
    }

    #[test]
    fn test_parse_checkpoints() {
        let race = parse_gpx(MINIMAL_GPX).unwrap();
        assert_eq!(race.checkpoints.len(), 2); // POI waypoint excluded
        assert_eq!(race.checkpoints[0].name, "Start");
        assert_eq!(race.checkpoints[0].latitude, 61.1);
        assert_eq!(race.checkpoints[0].longitude, 13.3);
        assert_eq!(race.checkpoints[0].elevation_m, 350.0);
        assert_eq!(race.checkpoints[0].distance_km, 0.0);
        assert_eq!(race.checkpoints[1].name, "Finish");
        assert_eq!(race.checkpoints[1].distance_km, 50.0);
    }

    #[test]
    fn test_non_checkpoint_waypoints_excluded() {
        let race = parse_gpx(MINIMAL_GPX).unwrap();
        // "Scenic Viewpoint" has type "poi", should not be included
        assert!(!race
            .checkpoints
            .iter()
            .any(|c| c.name == "Scenic Viewpoint"));
    }

    #[test]
    fn test_gpx_xml_preserved() {
        let race = parse_gpx(MINIMAL_GPX).unwrap();
        assert!(race.gpx_xml.contains("<gpx"));
        assert!(race.gpx_xml.contains("Test Race"));
    }

    #[test]
    fn test_missing_race_name_errors() {
        let gpx = r#"<?xml version="1.0"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1"
     xmlns:wb="https://github.com/LC-Zurich-Doppelstock/weather-bingo/gpx"
     version="1.1" creator="test">
  <metadata>
    <extensions>
      <wb:race>
        <wb:year>2026</wb:year>
        <wb:start_time>2026-03-01T08:00:00+01:00</wb:start_time>
        <wb:distance_km>50</wb:distance_km>
      </wb:race>
    </extensions>
  </metadata>
  <wpt lat="61.1" lon="13.3">
    <ele>350</ele>
    <name>Start</name>
    <type>checkpoint</type>
    <extensions><wb:distance_km>0</wb:distance_km></extensions>
  </wpt>
</gpx>"#;
        let result = parse_gpx(gpx);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metadata/name"));
    }

    #[test]
    fn test_missing_wb_year_errors() {
        let gpx = r#"<?xml version="1.0"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1"
     xmlns:wb="https://github.com/LC-Zurich-Doppelstock/weather-bingo/gpx"
     version="1.1" creator="test">
  <metadata>
    <name>Test</name>
    <extensions>
      <wb:race>
        <wb:start_time>2026-03-01T08:00:00+01:00</wb:start_time>
        <wb:distance_km>50</wb:distance_km>
      </wb:race>
    </extensions>
  </metadata>
  <wpt lat="61.1" lon="13.3">
    <ele>350</ele>
    <name>Start</name>
    <type>checkpoint</type>
    <extensions><wb:distance_km>0</wb:distance_km></extensions>
  </wpt>
</gpx>"#;
        let result = parse_gpx(gpx);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("wb:year"));
    }

    #[test]
    fn test_no_checkpoints_errors() {
        let gpx = r#"<?xml version="1.0"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1"
     xmlns:wb="https://github.com/LC-Zurich-Doppelstock/weather-bingo/gpx"
     version="1.1" creator="test">
  <metadata>
    <name>Test</name>
    <extensions>
      <wb:race>
        <wb:year>2026</wb:year>
        <wb:start_time>2026-03-01T08:00:00+01:00</wb:start_time>
        <wb:distance_km>50</wb:distance_km>
      </wb:race>
    </extensions>
  </metadata>
</gpx>"#;
        let result = parse_gpx(gpx);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("checkpoint"));
    }

    #[test]
    fn test_parse_vasaloppet_gpx() {
        let gpx = include_str!("../../../data/vasaloppet-2026.gpx");
        let race = parse_gpx(gpx).unwrap();
        assert_eq!(race.name, "Vasaloppet");
        assert_eq!(race.year, 2026);
        assert_eq!(race.distance_km, 90.0);
        assert_eq!(race.checkpoints.len(), 9);
        assert_eq!(race.checkpoints[0].name, "Berga (Start)");
        assert_eq!(race.checkpoints[0].distance_km, 0.0);
        assert_eq!(race.checkpoints[8].name, "Mora (Finish)");
        assert_eq!(race.checkpoints[8].distance_km, 90.0);
    }

    #[test]
    fn test_extract_track_points_minimal() {
        let points = extract_track_points(MINIMAL_GPX).unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].lat, 61.1);
        assert_eq!(points[0].lon, 13.3);
        assert_eq!(points[0].ele, 350.0);
        assert_eq!(points[1].lat, 61.0);
        assert_eq!(points[1].lon, 14.5);
        assert_eq!(points[1].ele, 165.0);
    }

    #[test]
    fn test_extract_track_points_vasaloppet() {
        let gpx = include_str!("../../../data/vasaloppet-2026.gpx");
        let points = extract_track_points(gpx).unwrap();
        // The Vasaloppet GPX has 384 track points
        assert!(
            points.len() > 100,
            "Expected many track points, got {}",
            points.len()
        );
        // First and last points should have valid coordinates
        assert!(points[0].lat > 60.0 && points[0].lat < 62.0);
        assert!(points[0].lon > 13.0 && points[0].lon < 15.0);
        assert!(points[0].ele > 0.0);
        let last = points.last().unwrap();
        assert!(last.lat > 60.0 && last.lat < 62.0);
    }

    #[test]
    fn test_extract_track_points_no_tracks() {
        let gpx = r#"<?xml version="1.0"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1" version="1.1" creator="test">
  <wpt lat="61.1" lon="13.3"><ele>350</ele><name>A Point</name></wpt>
</gpx>"#;
        let points = extract_track_points(gpx).unwrap();
        assert!(points.is_empty());
    }

    #[test]
    fn test_extract_track_points_missing_ele() {
        let gpx = r#"<?xml version="1.0"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1" version="1.1" creator="test">
  <trk><trkseg>
    <trkpt lat="61.1" lon="13.3"></trkpt>
  </trkseg></trk>
</gpx>"#;
        let points = extract_track_points(gpx).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].ele, 0.0); // defaults to 0
    }
}
