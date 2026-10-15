//! Read-only compatibility audit; never imports, resumes or observes positions.
use rusqlite::{types::ValueRef, Connection, OpenFlags};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: audit_legacy_positions PATH")?;
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.pragma_update(None, "query_only", true)?;
    let mut query = db.prepare("SELECT * FROM paper_trades WHERE status='OPEN' ORDER BY id")?;
    let columns = query
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut rows = query.query([])?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        let mut record = serde_json::Map::new();
        for (i, name) in columns.iter().enumerate() {
            let value = match row.get_ref(i)? {
                ValueRef::Null => serde_json::Value::Null,
                ValueRef::Integer(n) => n.into(),
                ValueRef::Real(n) => serde_json::Number::from_f64(n)
                    .ok_or("invalid number")?
                    .into(),
                ValueRef::Text(s) => std::str::from_utf8(s)?.into(),
                ValueRef::Blob(_) => return Err("unexpected binary field".into()),
            };
            record.insert(name.clone(), value);
        }
        let record = serde_json::Value::Object(record);
        let trade = snipers_core::legacy_position::restore(&record, now)
            .map_err(|e| format!("position {}: {e:?}", record["id"]))?;
        println!(
            "position {} / {}: compatible; original remains OPEN; restored closed={}",
            record["id"],
            record["symbol"],
            trade.closed_ms.is_some()
        );
        count += 1;
    }
    println!("{count} open positions checked read-only; no records written.");
    Ok(())
}
