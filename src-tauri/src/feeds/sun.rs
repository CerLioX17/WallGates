//! Sunrise / sunset (NOAA approximation), used by time-of-day playlists
//! without any network access.

use std::f64::consts::PI;

/// Returns (sunrise, sunset) as minutes after local midnight, or None in polar day/night.
pub fn sun_times(lat: f64, lon: f64, day_of_year: u32, utc_offset_minutes: i32) -> Option<(u32, u32)> {
    let g = 2.0 * PI / 365.0 * (day_of_year as f64 - 1.0);
    let eqtime = 229.18
        * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2.0 * g).cos() - 0.040849 * (2.0 * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos()
        + 0.000907 * (2.0 * g).sin()
        - 0.002697 * (3.0 * g).cos()
        + 0.00148 * (3.0 * g).sin();
    let lat_r = lat.to_radians();
    let cos_ha = (90.833f64.to_radians().cos() / (lat_r.cos() * decl.cos())) - lat_r.tan() * decl.tan();
    if !(-1.0..=1.0).contains(&cos_ha) {
        return None;
    }
    let ha = cos_ha.acos().to_degrees();
    let sunrise_utc = 720.0 - 4.0 * (lon + ha) - eqtime;
    let sunset_utc = 720.0 - 4.0 * (lon - ha) - eqtime;
    let wrap = |m: f64| ((m + utc_offset_minutes as f64).rem_euclid(1440.0)) as u32;
    Some((wrap(sunrise_utc), wrap(sunset_utc)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rome_midsummer() {
        // Rome, 21 June, UTC+2: sunrise ~05:35, sunset ~20:48.
        let (rise, set) = sun_times(41.9, 12.5, 172, 120).unwrap();
        assert!((325..=345).contains(&rise), "sunrise {rise}");
        assert!((1238..=1258).contains(&set), "sunset {set}");
    }

    #[test]
    fn polar_night() {
        assert!(sun_times(78.2, 15.6, 355, 60).is_none());
    }
}
