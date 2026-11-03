//! Market data for terminal wallpapers ("markets" capability).
//!
//! The wallpaper lists instruments in its properties (`watchlist`, `portfolio`,
//! `indices`): ISINs (e.g. `IE00B4L5Y983`) or tickers (`AAPL`, `ENI.MI`,
//! `^GSPC`, `EURUSD=X`, `BTC-EUR`). ISINs are resolved to a listing on the
//! user's preferred exchange (`market` property) and cached on disk.
//!
//! Data comes from Yahoo Finance's public chart endpoints: free and keyless,
//! but delayed for many exchanges (typically 15 min in Europe) and for
//! personal, informational use only.

use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Preferred listing: (exchange codes used by Yahoo, ticker suffix).
fn market_exchanges(market: &str) -> (&'static [&'static str], &'static str) {
    match market {
        "us" => (&["NMS", "NYQ", "NGM", "NCM", "ASE", "PCX", "BTS", "NAS"], ""),
        "milan" => (&["MIL"], ".MI"),
        "xetra" => (&["GER"], ".DE"),
        "frankfurt" => (&["FRA"], ".F"),
        "paris" => (&["PAR"], ".PA"),
        "amsterdam" => (&["AMS"], ".AS"),
        "london" => (&["LSE"], ".L"),
        "madrid" => (&["MCE"], ".MC"),
        "swiss" => (&["EBS"], ".SW"),
        "tokyo" => (&["JPX"], ".T"),
        "toronto" => (&["TOR"], ".TO"),
        "hongkong" => (&["HKG"], ".HK"),
        _ => (&[], ""),
    }
}

pub fn is_isin(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 12
        && b[0].is_ascii_uppercase()
        && b[1].is_ascii_uppercase()
        && b[2..11].iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && b[11].is_ascii_digit()
}

/// Instruments requested by one wallpaper.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Request {
    pub ids: BTreeSet<String>,
    pub market: String,
    pub currency: String,
    pub refresh_secs: u64,
}

fn tokens(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|t| t.trim().to_ascii_uppercase())
        .filter(|t| !t.is_empty() && t.len() <= 24 && t.chars().all(|c| c.is_ascii_alphanumeric() || "^=.-_".contains(c)))
}

/// Read the instrument lists from a wallpaper's property values.
pub fn request_from(values: &Map<String, Value>) -> Request {
    let s = |k: &str| values.get(k).and_then(Value::as_str).unwrap_or("");
    let mut ids = BTreeSet::new();
    // "#" starts a comment until the end of the line (names, notes).
    let strip = |text: &str| text.lines().map(|l| l.split('#').next().unwrap_or("").to_string()).collect::<Vec<_>>().join("\n");
    ids.extend(tokens(&strip(s("watchlist"))));
    ids.extend(tokens(&strip(s("indices"))));
    // Portfolio lines: "ISIN_or_TICKER quantity average_price" — only the first token is an instrument.
    for line in strip(s("portfolio")).lines() {
        if let Some(first) = tokens(line).next() {
            if !first.chars().all(|c| c.is_ascii_digit() || c == '.') {
                ids.insert(first);
            }
        }
    }
    let ids: BTreeSet<String> = ids.into_iter().take(80).collect();
    Request {
        ids,
        market: s("market").to_ascii_lowercase(),
        currency: {
            let c = s("currency").trim().to_ascii_uppercase();
            if c.len() == 3 { c } else { "EUR".into() }
        },
        refresh_secs: values.get("refresh").and_then(Value::as_f64).unwrap_or(60.0).clamp(15.0, 3600.0) as u64,
    }
}

fn agent() -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new();
    if let Ok(tls) = native_tls::TlsConnector::new() {
        b = b.tls_connector(std::sync::Arc::new(tls));
    }
    b.timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Wallgates")
        .build()
}

/// Choose the best listing from a Yahoo search response.
pub fn pick_symbol(search: &Value, isin: &str, market: &str) -> Option<String> {
    let quotes = search.get("quotes")?.as_array()?;
    let usable: Vec<&Value> = quotes
        .iter()
        .filter(|q| q.get("symbol").and_then(Value::as_str).is_some())
        .filter(|q| q.get("isYahooFinance").and_then(Value::as_bool).unwrap_or(true))
        .collect();
    let (exchanges, suffix) = market_exchanges(market);
    fn exch<'a>(q: &&'a Value) -> &'a str {
        q.get("exchange").and_then(Value::as_str).unwrap_or("")
    }
    let sym = |q: &Value| q["symbol"].as_str().unwrap().to_string();
    if let Some(q) = usable.iter().find(|q| exchanges.contains(&exch(q))) {
        return Some(sym(q));
    }
    if !suffix.is_empty() {
        if let Some(q) = usable.iter().find(|q| q["symbol"].as_str().unwrap().ends_with(suffix)) {
            return Some(sym(q));
        }
    }
    // US securities: prefer the primary US listing.
    if isin.starts_with("US") {
        let (us, _) = market_exchanges("us");
        if let Some(q) = usable.iter().find(|q| us.contains(&exch(q))) {
            return Some(sym(q));
        }
    }
    usable.first().map(|q| sym(q))
}

/// Normalise a Yahoo chart response into a quote.
pub fn parse_chart(id: &str, chart: &Value) -> Option<Value> {
    let r = chart.pointer("/chart/result/0")?;
    let m = r.get("meta")?;
    let price = m.get("regularMarketPrice").and_then(Value::as_f64)?;
    let prev = m
        .get("chartPreviousClose")
        .or_else(|| m.get("previousClose"))
        .and_then(Value::as_f64)
        .unwrap_or(price);
    let closes: Vec<f64> = r
        .pointer("/indicators/quote/0/close")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    // Downsample the intraday series to <= 60 points for sparklines.
    let step = (closes.len() / 60).max(1);
    let spark: Vec<f64> = closes.iter().step_by(step).map(|v| (v * 10000.0).round() / 10000.0).collect();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let period = |k: &str, f: &str| m.pointer(&format!("/currentTradingPeriod/{k}/{f}")).and_then(Value::as_i64);
    let state = match (period("regular", "start"), period("regular", "end")) {
        (Some(s), Some(e)) if now >= s && now < e => "open",
        (Some(s), _) if period("pre", "start").is_some_and(|p| now >= p) && now < s => "pre",
        (_, Some(e)) if period("post", "end").is_some_and(|p| now < p) && now >= e => "post",
        _ => "closed",
    };
    let change = price - prev;
    Some(json!({
        "id": id,
        "symbol": m.get("symbol"),
        "name": m.get("shortName").or_else(|| m.get("longName")).cloned().unwrap_or_else(|| json!(id)),
        "exchange": m.get("fullExchangeName").or_else(|| m.get("exchangeName")),
        "currency": m.get("currency"),
        "type": m.get("instrumentType"),
        "price": price,
        "previousClose": prev,
        "change": change,
        "changePct": if prev != 0.0 { change / prev * 100.0 } else { 0.0 },
        "dayHigh": m.get("regularMarketDayHigh"),
        "dayLow": m.get("regularMarketDayLow"),
        "volume": m.get("regularMarketVolume"),
        "high52": m.get("fiftyTwoWeekHigh"),
        "low52": m.get("fiftyTwoWeekLow"),
        "time": m.get("regularMarketTime"),
        "timezone": m.get("exchangeTimezoneName"),
        "state": state,
        "spark": spark,
    }))
}

pub struct Markets {
    cache_file: PathBuf,
    symbols: HashMap<String, String>,
}

impl Markets {
    pub fn new(data_dir: &Path) -> Self {
        let cache_file = data_dir.join("markets-symbols.json");
        let symbols = std::fs::read(&cache_file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { cache_file, symbols }
    }

    fn resolve(&mut self, agent: &ureq::Agent, id: &str, market: &str) -> Result<String, String> {
        if !is_isin(id) {
            return Ok(id.to_string());
        }
        let key = format!("{id}|{market}");
        if let Some(s) = self.symbols.get(&key) {
            return Ok(s.clone());
        }
        let v: Value = agent
            .get("https://query1.finance.yahoo.com/v1/finance/search")
            .query("q", id)
            .query("quotesCount", "10")
            .query("newsCount", "0")
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let symbol = pick_symbol(&v, id, market).ok_or_else(|| format!("{id}: not found"))?;
        self.symbols.insert(key, symbol.clone());
        let _ = std::fs::write(&self.cache_file, serde_json::to_vec_pretty(&self.symbols).unwrap_or_default());
        Ok(symbol)
    }

    fn chart(agent: &ureq::Agent, symbol: &str) -> Result<Value, String> {
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}",
            percent_encoding::utf8_percent_encode(symbol, percent_encoding::NON_ALPHANUMERIC)
        );
        agent
            .get(&url)
            .query("range", "1d")
            .query("interval", "5m")
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())
    }

    /// Fetch everything the requests need. Never fails as a whole: per-instrument errors are reported.
    pub fn fetch(&mut self, requests: &[Request]) -> Value {
        let agent = agent();
        let mut quotes = Vec::new();
        let mut errors = Vec::new();
        let mut currencies = BTreeSet::new();
        let mut bases = BTreeSet::new();
        let mut done = BTreeSet::new();
        for req in requests {
            bases.insert(req.currency.clone());
            for id in &req.ids {
                if !done.insert(format!("{id}|{}", req.market)) {
                    continue;
                }
                let result = self
                    .resolve(&agent, id, &req.market)
                    .and_then(|sym| Self::chart(&agent, &sym))
                    .and_then(|c| parse_chart(id, &c).ok_or_else(|| format!("{id}: no data")));
                match result {
                    Ok(q) => {
                        if let Some(c) = q.get("currency").and_then(Value::as_str) {
                            currencies.insert(if c == "GBp" { "GBP".to_string() } else { c.to_string() });
                        }
                        quotes.push(q);
                    }
                    Err(e) => errors.push(e),
                }
            }
        }
        // FX rates to each requested base currency (for portfolio totals).
        let mut fx = Map::new();
        for base in &bases {
            for c in &currencies {
                if c == base {
                    continue;
                }
                let pair = format!("{c}{base}=X");
                if let Ok(v) = Self::chart(&agent, &pair) {
                    if let Some(rate) = v.pointer("/chart/result/0/meta/regularMarketPrice").and_then(Value::as_f64) {
                        fx.insert(format!("{c}{base}"), json!(rate));
                    }
                }
            }
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        json!({
            "updated": now,
            "source": "Yahoo Finance (delayed)",
            "quotes": quotes,
            "fx": fx,
            "errors": errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isin_detection() {
        assert!(is_isin("IE00B4L5Y983"));
        assert!(is_isin("US0378331005"));
        assert!(!is_isin("AAPL"));
        assert!(!is_isin("IE00B4L5Y98X"));
    }

    #[test]
    fn reads_request_from_properties() {
        let v: Map<String, Value> = serde_json::from_value(json!({
            "watchlist": "aapl, IE00B4L5Y983 # iShares Core MSCI World\nENI.MI",
            "portfolio": "US0378331005 10 150.5\n# comment\nVWCE.DE 3 100",
            "indices": "^GSPC ^FTSEMIB.MI",
            "market": "Milan", "currency": "eur", "refresh": 5
        }))
        .unwrap();
        let r = request_from(&v);
        assert!(r.ids.contains("AAPL") && r.ids.contains("IE00B4L5Y983") && r.ids.contains("US0378331005"));
        assert!(r.ids.contains("VWCE.DE") && r.ids.contains("^GSPC"));
        assert!(!r.ids.contains("10") && !r.ids.contains("150.5"));
        assert!(!r.ids.contains("ISHARES") && !r.ids.contains("CORE"));
        assert_eq!(r.market, "milan");
        assert_eq!(r.currency, "EUR");
        assert_eq!(r.refresh_secs, 15);
    }

    #[test]
    fn picks_preferred_listing() {
        let search = json!({ "quotes": [
            { "symbol": "IWDA.AS", "exchange": "AMS", "isYahooFinance": true },
            { "symbol": "SWDA.L", "exchange": "LSE", "isYahooFinance": true },
            { "symbol": "SWDA.MI", "exchange": "MIL", "isYahooFinance": true },
            { "symbol": "EUNL.DE", "exchange": "GER", "isYahooFinance": true }
        ]});
        assert_eq!(pick_symbol(&search, "IE00B4L5Y983", "milan").as_deref(), Some("SWDA.MI"));
        assert_eq!(pick_symbol(&search, "IE00B4L5Y983", "xetra").as_deref(), Some("EUNL.DE"));
        assert_eq!(pick_symbol(&search, "IE00B4L5Y983", "auto").as_deref(), Some("IWDA.AS"));
        let us = json!({ "quotes": [
            { "symbol": "APC.F", "exchange": "FRA" }, { "symbol": "AAPL", "exchange": "NMS" }
        ]});
        assert_eq!(pick_symbol(&us, "US0378331005", "auto").as_deref(), Some("AAPL"));
    }

    #[test]
    fn parses_chart() {
        let chart = json!({ "chart": { "result": [{
            "meta": { "symbol": "SWDA.MI", "currency": "EUR", "shortName": "iShares Core MSCI World",
                      "regularMarketPrice": 104.0, "chartPreviousClose": 100.0, "fullExchangeName": "Milan",
                      "regularMarketDayHigh": 104.5, "regularMarketDayLow": 99.8,
                      "currentTradingPeriod": { "regular": { "start": 0, "end": 1 } } },
            "indicators": { "quote": [{ "close": [100.0, null, 101.5, 104.0] }] }
        }], "error": null }});
        let q = parse_chart("IE00B4L5Y983", &chart).unwrap();
        assert_eq!(q["symbol"], json!("SWDA.MI"));
        assert!((q["changePct"].as_f64().unwrap() - 4.0).abs() < 1e-9);
        assert_eq!(q["spark"].as_array().unwrap().len(), 3);
        assert_eq!(q["state"], json!("closed"));
        assert!(parse_chart("X", &json!({ "chart": { "result": null } })).is_none());
    }
}
