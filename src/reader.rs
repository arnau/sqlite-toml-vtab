use walkdir::WalkDir;
use crate::types::Records;


pub fn read_data(path: &str) -> Result<Records, anyhow::Error> {
    let mut records: Records = Vec::new();

    for entry in WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let b = std::fs::read(entry.path())?;
            let s = String::from_utf8_lossy(&b);
            let value: toml::Value = toml::from_str(&s)?;
            let data = serde_json::to_string(&value)?;
            let record = vec![entry.path().display().to_string(), data];

            records.push(record);
        }
    }

    Ok(records)
}
