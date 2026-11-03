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
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m,wind_direction_10m,wind_gusts_10m,uv_index,pressure_msl,cloud_cover,precipitation,visibility,dew_point_2m\
         &hourly=temperature_2m,precipitation_probability,weather_code,uv_index\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,precipitation_sum,uv_index_max,sunrise,sunset,wind_speed_10m_max,daylight_duration\
         &forecast_days=4&forecast_hours=24&timezone=auto{}",
        loc.latitude,
        loc.longitude,
        if fahrenheit { "&temperature_unit=fahrenheit&wind_speed_unit=mph" } else { "" }
    );
    let v: Value = agent().get(&url).call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    let cur = v.get("current").ok_or("no current weather")?;
    let code = cur.get("weather_code").and_then(Value::as_i64).unwrap_or(-1);
    let day = |k: &str, i: usize| v.pointer(&format!("/daily/{k}/{i}")).cloned().unwrap_or(Value::Null);

    let days: Vec<Value> = (0..4)
        .filter(|i| !day("time", *i).is_null())
        .map(|i| {
            let c = day("weather_code", i).as_i64().unwrap_or(-1);
            json!({
                "date": day("time", i),
                "code": c,
                "condition": condition(c),
                "description": describe(c),
                "max": day("temperature_2m_max", i),
                "min": day("temperature_2m_min", i),
                "rainChance": day("precipitation_probability_max", i),
                "rain": day("precipitation_sum", i),
                "uvMax": day("uv_index_max", i),
                "windMax": day("wind_speed_10m_max", i),
                "sunrise": day("sunrise", i),
                "sunset": day("sunset", i),
                "daylight": day("daylight_duration", i),
            })
        })
        .collect();
    let hours: Vec<Value> = (0..24)
        .filter_map(|i| {
            let t = v.pointer(&format!("/hourly/time/{i}"))?.clone();
            let c = v.pointer(&format!("/hourly/weather_code/{i}")).and_then(Value::as_i64).unwrap_or(-1);
            Some(json!({
                "time": t,
                "temp": v.pointer(&format!("/hourly/temperature_2m/{i}")),
                "rainChance": v.pointer(&format!("/hourly/precipitation_probability/{i}")),
                "uv": v.pointer(&format!("/hourly/uv_index/{i}")),
                "code": c,
                "condition": condition(c),
            }))
        })
        .collect();

    // Air quality (best effort, separate free API).
    let air = agent()
        .get(&format!(
            "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={:.4}&longitude={:.4}&current=european_aqi,pm2_5,pm10,ozone&timezone=auto",
            loc.latitude, loc.longitude
        ))
        .call()
        .ok()
        .and_then(|r| r.into_json::<Value>().ok())
        .and_then(|a| a.get("current").cloned())
        .map(|a| json!({ "aqi": a.get("european_aqi"), "pm25": a.get("pm2_5"), "pm10": a.get("pm10"), "ozone": a.get("ozone") }))
        .unwrap_or(Value::Null);

    Ok(json!({
        "location": loc.name,
        "temperature": cur.get("temperature_2m"),
        "feelsLike": cur.get("apparent_temperature"),
        "humidity": cur.get("relative_humidity_2m"),
        "dewPoint": cur.get("dew_point_2m"),
        "wind": cur.get("wind_speed_10m"),
        "windDir": cur.get("wind_direction_10m"),
        "gusts": cur.get("wind_gusts_10m"),
        "uv": cur.get("uv_index"),
        "pressure": cur.get("pressure_msl"),
        "clouds": cur.get("cloud_cover"),
        "precipitation": cur.get("precipitation"),
        "visibility": cur.get("visibility"),
        "code": code,
        "condition": condition(code),
        "description": describe(code),
        "isDay": cur.get("is_day").and_then(Value::as_i64).unwrap_or(1) == 1,
        "high": day("temperature_2m_max", 0),
        "low": day("temperature_2m_min", 0),
        "sunrise": day("sunrise", 0),
        "sunset": day("sunset", 0),
        "uvMax": day("uv_index_max", 0),
        "rainChance": day("precipitation_probability_max", 0),
        "days": days,
        "hours": hours,
        "air": air,
        "units": if fahrenheit { "°F" } else { "°C" },
        "windUnits": if fahrenheit { "mph" } else { "km/h" },
        "updated": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
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
