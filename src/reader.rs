use crate::types::Records;
use serde_json::Value as Json;
use toml::Value as Toml;
use walkdir::WalkDir;

pub fn read_data(path: &str) -> Result<Records, anyhow::Error> {
    let mut records: Records = Vec::new();

    for entry in WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let b = std::fs::read(entry.path())?;
            let s = String::from_utf8_lossy(&b);
            let toml: Toml = toml::from_str(&s)?;
            let json = convert(toml);
            let data = serde_json::to_string(&json)?;
            let record = vec![entry.path().display().to_string(), data];

            records.push(record);
        }
    }

    Ok(records)
}

fn convert(toml: Toml) -> Json {
    match toml {
        Toml::String(s) => Json::String(s),
        Toml::Integer(i) => Json::Number(i.into()),
        Toml::Float(f) => {
            let n = serde_json::Number::from_f64(f).expect("float infinite and nan not allowed");
            Json::Number(n)
        }
        Toml::Boolean(b) => Json::Bool(b),
        Toml::Array(arr) => Json::Array(arr.into_iter().map(convert).collect()),
        Toml::Table(table) => {
            Json::Object(table.into_iter().map(|(k, v)| (k, convert(v))).collect())
        }
        Toml::Datetime(dt) => Json::String(dt.to_string()),
    }
}
