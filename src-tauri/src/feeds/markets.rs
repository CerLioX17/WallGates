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
        "stuttgart" => (&["STU"], ".SG"),
        "munich" => (&["MUN"], ".MU"),
        "berlin" => (&["BER"], ".BE"),
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

/// Euro-denominated venues on Yahoo Finance (exchange code, ticker suffix), most liquid / longest hours first.
const EUR_LISTINGS: &[(&str, &str)] = &[
    ("GER", ".DE"), ("FRA", ".F"), ("STU", ".SG"), ("MUN", ".MU"), ("BER", ".BE"), ("DUS", ".DU"), ("HAM", ".HM"),
    ("HAN", ".HA"), ("MIL", ".MI"), ("AMS", ".AS"), ("PAR", ".PA"), ("MCE", ".MC"), ("BRU", ".BR"), ("VIE", ".VI"),
    ("LIS", ".LS"), ("HEL", ".HE"), ("ISE", ".IR"),
];

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
    /// Also fetch ~2 years of daily closes (portfolio performance over time).
    pub history: bool,
    /// Live price source: "ls" = Lang & Schwarz Exchange through onvista (what Trade Republic shows),
    /// "tradegate", "gettex", or "yahoo".
    pub source: String,
    /// Per-instrument choice (`listings` property): id → venue keyword (xetra, frankfurt, ls, tradegate…) or Yahoo symbol.
    pub listings: std::collections::BTreeMap<String, String>,
}

/// Venue keywords understood in the `listings` property and the `priceSource` option.
pub fn is_onvista_venue(v: &str) -> bool {
    matches!(v, "ls" | "lang" | "lsx" | "tradegate" | "gettex")
}

/// Parse `listings` lines: `ISIN venue-or-symbol` (`#` comments allowed).
pub fn parse_listings(text: &str) -> std::collections::BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let l = l.split('#').next().unwrap_or("");
            let mut it = l.split(|c: char| c.is_whitespace() || c == ';' || c == '=' || c == ',').filter(|t| !t.is_empty());
            let id = it.next()?.trim().to_ascii_uppercase();
            let spec = it.next()?.trim().to_string();
            Some((id, spec))
        })
        .collect()
}

/// Split one CSV line, honouring double quotes ("a,b" and "" escapes).
pub fn csv_fields(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => { cur.push('"'); chars.next(); }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Instruments found in a Trade Republic transaction export (CSV).
/// Crypto (`asset_class` CRYPTO) is quoted against the euro, e.g. `XRP` → `XRP-EUR`.
pub fn trade_republic_ids(csv: &str) -> BTreeSet<String> {
    let mut lines = csv.lines();
    let header = lines.next().map(csv_fields).unwrap_or_default();
    let col = |name: &str| header.iter().position(|h| h.trim().eq_ignore_ascii_case(name));
    let (Some(sym), Some(class)) = (col("symbol"), col("asset_class")) else { return BTreeSet::new() };
    let mut ids = BTreeSet::new();
    for line in lines {
        let f = csv_fields(line);
        let (Some(id), Some(c)) = (f.get(sym), f.get(class)) else { continue };
        let id = id.trim().to_ascii_uppercase();
        if id.is_empty() || id.len() > 16 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        if c.eq_ignore_ascii_case("CRYPTO") {
            ids.insert(format!("{id}-EUR"));
        } else if is_isin(&id) {
            ids.insert(id);
        }
    }
    ids
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
    let tr = s("tradeRepublicCsv");
    ids.extend(trade_republic_ids(tr));
    let ids: BTreeSet<String> = ids.into_iter().take(80).collect();
    Request {
        history: !tr.trim().is_empty() || values.get("history").and_then(Value::as_bool).unwrap_or(false),
        source: {
            let v = s("priceSource").trim().to_ascii_lowercase();
            if v.is_empty() { if tr.trim().is_empty() { "yahoo".into() } else { "ls".into() } } else { v }
        },
        listings: parse_listings(s("listings")),
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
    let euro = matches!(market, "xetra" | "frankfurt" | "milan" | "paris" | "amsterdam" | "madrid" | "eur");
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
    // Euro investors (Trade Republic, Scalable, …): any EUR listing beats a USD/GBP one, so the price and its
    // daily change are in euro. German venues first: they quote almost everything and trade until 22:00.
    if euro {
        for (code, sfx) in EUR_LISTINGS {
            if let Some(q) = usable.iter().find(|q| exch(q) == *code || q["symbol"].as_str().unwrap().ends_with(sfx)) {
                return Some(sym(q));
            }
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
    /// symbol → (fetched at, daily [unix time, close] pairs). Daily data changes slowly: refreshed every 3 h.
    history: HashMap<String, (std::time::Instant, Value)>,
}

/// First JSON object (depth-first) that has a numeric `last` field: onvista's quote block.
fn find_quote(v: &Value) -> Option<&Map<String, Value>> {
    match v {
        Value::Object(m) => {
            if m.get("last").and_then(Value::as_f64).is_some() {
                return Some(m);
            }
            if let Some(q) = m.get("quote").and_then(find_quote) {
                return Some(q);
            }
            m.values().find_map(find_quote)
        }
        Value::Array(a) => a.iter().find_map(find_quote),
        _ => None,
    }
}

/// Every quote block (object with a numeric `last`) in an onvista snapshot, e.g. one per trading venue.
fn all_quotes<'a>(v: &'a Value, out: &mut Vec<&'a Map<String, Value>>) {
    match v {
        Value::Object(m) => {
            if m.get("last").and_then(Value::as_f64).is_some() {
                out.push(m);
            }
            for x in m.values() {
                all_quotes(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| all_quotes(x, out)),
        _ => {}
    }
}

/// Venue names onvista uses, for each keyword.
fn venue_names(venue: &str) -> &'static [&'static str] {
    match venue {
        "tradegate" => &["tradegate"],
        "gettex" => &["gettex"],
        "ls" | "lang" | "lsx" => &["lang & schwarz", "ls exchange", "l&s", "lang und schwarz", "lsx"],
        _ => &[],
    }
}

/// Normalise an onvista snapshot into a quote, preferring the requested venue
/// ("ls" = Lang & Schwarz Exchange, Trade Republic's venue; its previous close is Trade Republic's reference).
pub fn parse_onvista_venue(id: &str, snap: &Value, venue: &str) -> Option<Value> {
    let mut list = Vec::new();
    all_quotes(snap, &mut list);
    let market_name = |q: &Map<String, Value>| {
        q.get("market").map(|m| format!("{} {} {}",
            m.get("name").and_then(Value::as_str).unwrap_or(""), m.get("nameExchange").and_then(Value::as_str).unwrap_or(""),
            m.get("codeExchange").and_then(Value::as_str).unwrap_or("")).to_lowercase()).unwrap_or_default()
    };
    let names = venue_names(venue);
    let q = list.iter().copied().find(|q| { let n = market_name(q); names.iter().any(|x| n.contains(x)) })
        .or_else(|| find_quote(snap))?;
    parse_onvista_quote(id, snap, q)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_onvista(id: &str, snap: &Value) -> Option<Value> {
    parse_onvista_venue(id, snap, "")
}

fn parse_onvista_quote(id: &str, snap: &Value, q: &Map<String, Value>) -> Option<Value> {
    let price = q.get("last").and_then(Value::as_f64)?;
    let prev = q.get("previousLast").or_else(|| q.get("previousClose")).and_then(Value::as_f64).unwrap_or(price);
    let name = snap.pointer("/instrument/name").or_else(|| snap.get("name")).cloned().unwrap_or_else(|| json!(id));
    let change = price - prev;
    Some(json!({
        "id": id, "symbol": id, "name": name,
        "exchange": q.get("market").and_then(|m| m.get("name")).cloned().unwrap_or_else(|| json!("onvista")),
        "currency": q.get("isoCurrency").and_then(Value::as_str).unwrap_or("EUR"),
        "price": price, "previousClose": prev, "change": change,
        "changePct": if prev != 0.0 { change / prev * 100.0 } else { 0.0 },
        "time": q.get("datetimeLast"), "previousTime": q.get("datetimePreviousLast"), "state": "closed", "spark": [], "source": "onvista",
    }))
}

/// Daily closes from a Yahoo chart response, as `[[unix, close], ...]`.
pub fn parse_history(chart: &Value) -> Value {
    let r = chart.pointer("/chart/result/0");
    let ts = r.and_then(|r| r.get("timestamp")).and_then(Value::as_array);
    let cl = r.and_then(|r| r.pointer("/indicators/quote/0/close")).and_then(Value::as_array);
    let (Some(ts), Some(cl)) = (ts, cl) else { return json!([]) };
    Value::Array(
        ts.iter()
            .zip(cl)
            .filter_map(|(t, c)| Some(json!([t.as_i64()?, (c.as_f64()? * 10000.0).round() / 10000.0])))
            .collect(),
    )
}

impl Markets {
    pub fn new(data_dir: &Path) -> Self {
        let cache_file = data_dir.join("markets-symbols-v2.json"); // v2: EUR-listing preference (v1 cached USD lines)
        let symbols = std::fs::read(&cache_file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { cache_file, symbols, history: HashMap::new() }
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
        Self::chart_range(agent, symbol, "1d", "5m")
    }

    fn chart_range(agent: &ureq::Agent, symbol: &str, range: &str, interval: &str) -> Result<Value, String> {
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}",
            percent_encoding::utf8_percent_encode(symbol, percent_encoding::NON_ALPHANUMERIC)
        );
        agent
            .get(&url)
            .query("range", range)
            .query("interval", interval)
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())
    }

    /// Fallback for instruments without a Yahoo listing, through onvista's public JSON API.
    fn onvista(agent: &ureq::Agent, isin: &str) -> Result<Value, String> {
        Self::onvista_venue(agent, isin, "")
    }

    fn onvista_venue(agent: &ureq::Agent, isin: &str, venue: &str) -> Result<Value, String> {
        // ISIN → (entityType, entityValue) never changes: look it up once per session.
        static IDS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, (String, String)>>> = std::sync::OnceLock::new();
        let ids = IDS.get_or_init(Default::default);
        let cached = ids.lock().ok().and_then(|m| m.get(isin).cloned());
        let (kind, value) = match cached {
            Some(kv) => kv,
            None => {
                let kv = Self::onvista_lookup(agent, isin)?;
                if let Ok(mut m) = ids.lock() { m.insert(isin.to_string(), kv.clone()); }
                kv
            }
        };
        let snap: Value = agent
            .get(&format!("https://api.onvista.de/api/v1/instruments/{kind}/{value}/snapshot"))
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        parse_onvista_venue(isin, &snap, venue).ok_or_else(|| format!("{isin}: no data"))
    }

    fn onvista_lookup(agent: &ureq::Agent, isin: &str) -> Result<(String, String), String> {
        let search: Value = agent
            .get("https://api.onvista.de/api/v1/instruments/search/facet")
            .query("searchValue", isin)
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let hit = search
            .pointer("/facets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|f| f.get("results").and_then(Value::as_array))
            .flatten()
            .find(|r| r.get("isin").and_then(Value::as_str).map_or(true, |i| i.eq_ignore_ascii_case(isin)))
            .ok_or_else(|| format!("{isin}: not found"))?;
        let (Some(kind), Some(value)) = (hit.get("entityType").and_then(Value::as_str), hit.get("entityValue").and_then(Value::as_str)) else {
            return Err(format!("{isin}: not found"));
        };
        Ok((kind.to_string(), value.to_string()))
    }

    fn daily(&mut self, agent: &ureq::Agent, symbol: &str) -> Value {
        if let Some((at, v)) = self.history.get(symbol) {
            if at.elapsed() < Duration::from_secs(3 * 3600) {
                return v.clone();
            }
        }
        match Self::chart_range(agent, symbol, "2y", "1d") {
            Ok(c) => {
                let v = parse_history(&c);
                self.history.insert(symbol.to_string(), (std::time::Instant::now(), v.clone()));
                v
            }
            Err(_) => self.history.get(symbol).map(|(_, v)| v.clone()).unwrap_or_else(|| json!([])),
        }
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
                // Per-instrument choice beats the global one: a venue keyword or an explicit Yahoo symbol.
                let spec = req.listings.get(id).map(|s| s.trim().to_string()).unwrap_or_default();
                let spec_l = spec.to_ascii_lowercase();
                let venue = if is_onvista_venue(&spec_l) { spec_l.clone() }
                    else if spec.is_empty() && is_onvista_venue(&req.source) { req.source.clone() } else { String::new() };
                let (market, symbol_override) = if spec.is_empty() || is_onvista_venue(&spec_l) { (req.market.clone(), None) }
                    else if market_exchanges(&spec_l).0.is_empty() { (req.market.clone(), Some(spec.clone())) }
                    else { (spec_l.clone(), None) };
                if !done.insert(format!("{id}|{market}|{venue}|{spec}")) {
                    continue;
                }
                let yahoo_symbol = match &symbol_override { Some(s) => Ok(s.clone()), None => self.resolve(&agent, id, &market) };
                let yahoo = |sym: &Result<String, String>| sym.clone()
                    .and_then(|sym| Self::chart(&agent, &sym))
                    .and_then(|c| parse_chart(id, &c).ok_or_else(|| format!("{id}: no data")));
                let result = if !venue.is_empty() && is_isin(id) {
                    // Live price from the chosen venue; Yahoo only as a fallback.
                    Self::onvista_venue(&agent, id, &venue).or_else(|e| yahoo(&yahoo_symbol).map_err(|_| e))
                } else {
                    yahoo(&yahoo_symbol).or_else(|e| if is_isin(id) { Self::onvista(&agent, id).map_err(|_| e) } else { Err(e) })
                };
                let result = result.map(|mut q| {
                    // Daily history always comes from the Yahoo listing (weeks/months/years: venue differences are tiny).
                    if req.history {
                        if let Ok(sym) = &yahoo_symbol {
                            q["history"] = self.daily(&agent, sym);
                            q["historySymbol"] = json!(sym);
                        }
                    }
                    q
                });
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
        let mut fx_prev = Map::new();
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
                    // Yesterday's rate: lets wallpapers compute daily changes in the base currency.
                    if let Some(prev) = v.pointer("/chart/result/0/meta/chartPreviousClose").and_then(Value::as_f64) {
                        fx_prev.insert(format!("{c}{base}"), json!(prev));
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
            "fxPrev": fx_prev,
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
    fn reads_trade_republic_export() {
        let csv = "\"datetime\",\"category\",\"type\",\"asset_class\",\"name\",\"symbol\",\"shares\"\n\
            \"2025-03-19\",\"TRADING\",\"BUY\",\"FUND\",\"Core MSCI World, USD\",\"IE00B4L5Y983\",\"2\"\n\
            \"2026-07-14\",\"TRADING\",\"BUY\",\"CRYPTO\",\"XRP\",\"XRP\",\"103\"\n\
            \"2025-03-13\",\"CASH\",\"TRANSFER_INBOUND\",\"\",\"\",\"\",\"\"\n";
        let ids = trade_republic_ids(csv);
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), vec!["IE00B4L5Y983".to_string(), "XRP-EUR".to_string()]);
        assert_eq!(csv_fields("\"a, b\",\"c\"\"d\",e"), vec!["a, b", "c\"d", "e"]);
        let v: Map<String, Value> = serde_json::from_value(json!({ "tradeRepublicCsv": csv })).unwrap();
        let r = request_from(&v);
        assert!(r.history && r.ids.contains("XRP-EUR"));
        let h = parse_history(&json!({ "chart": { "result": [{ "timestamp": [1, 2, 3], "indicators": { "quote": [{ "close": [10.0, null, 12.5] }] } }] } }));
        assert_eq!(h, json!([[1, 10.0], [3, 12.5]]));
    }

    #[test]
    fn picks_lang_schwarz_venue_and_listing_overrides() {
        let snap = json!({ "instrument": { "name": "Take-Two" }, "quote": { "last": 210.0, "previousLast": 208.0, "isoCurrency": "EUR", "market": { "name": "Xetra" } },
            "quoteList": { "list": [
                { "last": 210.1, "previousLast": 209.0, "isoCurrency": "EUR", "market": { "name": "Tradegate" } },
                { "last": 210.4, "previousLast": 209.04, "isoCurrency": "EUR", "market": { "name": "Lang & Schwarz" } } ] } });
        let q = parse_onvista_venue("US8740541094", &snap, "ls").unwrap();
        assert_eq!(q["price"], json!(210.4));
        assert_eq!(q["exchange"], json!("Lang & Schwarz"));
        assert_eq!(parse_onvista_venue("X", &snap, "tradegate").unwrap()["price"], json!(210.1));
        assert_eq!(parse_onvista_venue("X", &snap, "").unwrap()["price"], json!(210.0));
        let l = parse_listings("IE00B4L5Y983 EUNL.DE # iShares\nus8740541094=tradegate\n\nbad");
        assert_eq!(l.get("IE00B4L5Y983").map(String::as_str), Some("EUNL.DE"));
        assert_eq!(l.get("US8740541094").map(String::as_str), Some("tradegate"));
        assert_eq!(l.len(), 2);
        let v: Map<String, Value> = serde_json::from_value(json!({ "tradeRepublicCsv": "symbol,asset_class\nIE00B4L5Y983,FUND\n" })).unwrap();
        assert_eq!(request_from(&v).source, "ls");
        assert_eq!(request_from(&Map::new()).source, "yahoo");
    }

    #[test]
    fn parses_onvista_snapshot() {
        let snap = json!({ "instrument": { "name": "Turbo Short Alibaba" },
            "quote": { "last": 1.62, "previousLast": 1.8, "isoCurrency": "EUR", "market": { "name": "Lang & Schwarz" } } });
        let q = parse_onvista("DE000FD82214", &snap).unwrap();
        assert_eq!(q["price"], json!(1.62));
        assert_eq!(q["exchange"], json!("Lang & Schwarz"));
        assert!((q["changePct"].as_f64().unwrap() + 10.0).abs() < 1e-9);
        let nested = json!({ "quoteList": { "list": [{ "last": 2.0, "isoCurrency": "EUR" }] } });
        assert_eq!(parse_onvista("X", &nested).unwrap()["price"], json!(2.0));
        assert!(parse_onvista("X", &json!({ "foo": 1 })).is_none());
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
        // A US stock without a Xetra line: a euro investor gets the Frankfurt (EUR) listing, not the USD one.
        assert_eq!(pick_symbol(&us, "US0378331005", "xetra").as_deref(), Some("APC.F"));
        let stu = json!({ "quotes": [
            { "symbol": "TTWO", "exchange": "NMS" }, { "symbol": "TKE.SG", "exchange": "STU" }, { "symbol": "TKE.F", "exchange": "FRA" }
        ]});
        assert_eq!(pick_symbol(&stu, "US8740541094", "xetra").as_deref(), Some("TKE.F"));
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
