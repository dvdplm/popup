use std::collections::HashMap;

use serde::Deserialize;

// Blitzortung WebSocket servers
pub const BLITZSERVERS: [&'static str; 3] = [
    "wss://ws1.blitzortung.org",
    "wss://ws7.blitzortung.org",
    "wss://ws8.blitzortung.org",
];

// Magic handshake to start receiving lightning strikes
pub const BLITZ_HANDSHAKE: &[u8] = b"{\"a\":111}";

/// Lightning strike data structure matching Blitzortung's format
#[derive(Debug, Deserialize)]
pub struct LightningStrike {
    /// Timestamp in microseconds since epoch
    pub time: u64,
    /// Latitude
    pub lat: f64,
    /// Longitude
    pub lon: f64,
    /// Altitude
    pub alt: f64,
    /// Polarity
    pub pol: i32,
    /// MDS value
    pub mds: u32,
    /// MCG value
    pub mcg: u32,
    /// Status
    pub status: u32,
    /// Region
    pub region: u32,
    /// Signal data array
    pub sig: Vec<SignalData>,
    /// Delay information
    pub delay: Option<f64>,
    pub lonc: u32,
    pub latc: u32,
}

#[derive(Debug, Deserialize)]
pub struct SignalData {
    /// Station ID (?)
    pub sta: u32,
    /// is the number of nanoseconds since the last full second with 9 digits after the decimal point
    pub time: u64,
    pub lat: f64,
    pub lon: f64,
    pub alt: isize,
    pub status: u32,
}

/// LZW decoder for Blitzortung compressed messages
pub fn decode(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    let data: Vec<char> = input.chars().collect();
    let mut dictionary: HashMap<u32, Vec<char>> = HashMap::new();
    let mut result = String::with_capacity(input.len() * 2);

    let curr_char = data[0];
    let mut old_phrase = vec![curr_char];
    result.push(curr_char);
    let mut code: u32 = 256;

    for i in 1..data.len() {
        let curr_code = data[i] as u32;

        let phrase = if curr_code < 256 {
            vec![data[i]]
        } else {
            dictionary.get(&curr_code).cloned().unwrap_or_else(|| {
                let mut new_phrase = old_phrase.clone();
                new_phrase.push(old_phrase[0]);
                new_phrase
            })
        };

        result.extend(phrase.iter());

        let mut new_dict_entry = old_phrase.clone();
        new_dict_entry.push(phrase[0]);
        dictionary.insert(code, new_dict_entry);
        code += 1;
        old_phrase = phrase;
    }

    result
}
