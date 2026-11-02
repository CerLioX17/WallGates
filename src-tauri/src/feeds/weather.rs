//! Weather via Open-Meteo (free, no account, no API key). Only contacted when
//! the user enabled weather and a visible wallpaper requests it.

use crate::config::Location;
use serde_json::{json, Value};
use std::time::Duration;

fn agent() -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new();
    if let Ok(tls) = native_tls::TlsConnector::new() {
        b = b.tls_connector(std::sync::Arc::new(tls));
    }
    b.timeout(Duration::from_secs(15))
        .user_agent(concat!("Wallgates/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// WMO weather interpretation codes -> short English description.
pub fn describe(code: i64) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 | 77 => "Snow",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
}

/// Simplified condition for wallpapers to switch on.
pub fn condition(code: i64) -> &'static str {
    match code {
        0 | 1 => "clear",
        2 | 3 => "cloudy",
        45 | 48 => "fog",
        51..=67 | 80..=82 => "rain",
        71..=77 | 85 | 86 => "snow",
        95..=99 => "storm",
        _ => "unknown",
    }
}

pub fn fetch(loc: &Location, fahrenheit: bool) -> Result<Value, String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}\
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m\
         &daily=sunrise,sunset,temperature_2m_max,temperature_2m_min&forecast_days=1&timezone=auto{}",
        loc.latitude,
        loc.longitude,
        if fahrenheit { "&temperature_unit=fahrenheit&wind_speed_unit=mph" } else { "" }
    );
    let v: Value = agent().get(&url).call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    let cur = v.get("current").ok_or("no current weather")?;
    let code = cur.get("weather_code").and_then(Value::as_i64).unwrap_or(-1);
    let day = |k: &str| v.pointer(&format!("/daily/{k}/0")).cloned().unwrap_or(Value::Null);
    Ok(json!({
        "location": loc.name,
        "temperature": cur.get("temperature_2m"),
        "feelsLike": cur.get("apparent_temperature"),
        "humidity": cur.get("relative_humidity_2m"),
        "wind": cur.get("wind_speed_10m"),
        "code": code,
        "condition": condition(code),
        "description": describe(code),
        "isDay": cur.get("is_day").and_then(Value::as_i64).unwrap_or(1) == 1,
        "high": day("temperature_2m_max"),
        "low": day("temperature_2m_min"),
        "sunrise": day("sunrise"),
        "sunset": day("sunset"),
        "units": if fahrenheit { "°F" } else { "°C" },
    }))
}

pub fn geocode(query: &str, language: &str) -> Result<Vec<Location>, String> {
    let q: String = query.chars().take(80).collect();
    let v: Value = agent()
        .get("https://geocoding-api.open-meteo.com/v1/search")
        .query("name", &q)
        .query("count", "6")
        .query("language", language)
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    Ok(v.get("results")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    let name = r.get("name")?.as_str()?;
                    let region = [r.get("admin1"), r.get("country")]
                        .iter()
                        .filter_map(|x| x.and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join(", ");
                    Some(Location {
                        name: if region.is_empty() { name.to_string() } else { format!("{name}, {region}") },
                        latitude: r.get("latitude")?.as_f64()?,
                        longitude: r.get("longitude")?.as_f64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}
