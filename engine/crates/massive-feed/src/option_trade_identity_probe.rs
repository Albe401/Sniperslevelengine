//! Small original-response samples to distinguish source identity gaps from parsing.
use crate::{
    credentials::Credentials,
    history::checked_url,
    live::now_ms,
    option_activity_dataset,
    option_activity_history::{self, reported_identity, Report},
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Duration,
};

pub fn analyze(rows: &[Value]) -> Value {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut keys = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut verified = 0;
    let mut duplicates = 0;
    for (i, row) in rows.iter().enumerate() {
        if let Some(object) = row.as_object() {
            keys.extend(object.keys().cloned());
        }
        let sequence = if row.get("sequence_number").is_none() {
            "missing"
        } else if row["sequence_number"].is_null() {
            "null"
        } else if row["sequence_number"].as_u64() == Some(0) {
            "zero"
        } else if row["sequence_number"].as_u64().is_some_and(|n| n > 0) {
            "positive-integer"
        } else {
            "other-type-or-invalid"
        };
        *counts.entry(format!("sequence:{sequence}")).or_default() += 1;
        let id = if row.get("id").is_none() {
            "missing"
        } else if row["id"].is_null() {
            "null"
        } else if row["id"].as_str() == Some("") {
            "empty-string"
        } else if row["id"].as_str() == Some("0") {
            "zero-string"
        } else if row["id"].as_str().is_some() {
            "nonempty-string"
        } else {
            "other-type"
        };
        *counts.entry(format!("id:{id}")).or_default() += 1;
        if let Some((identity, true)) = reported_identity(row, i + 1) {
            verified += 1;
            if !ids.insert(identity) {
                duplicates += 1;
            }
        }
    }
    json!({"rows":rows.len(),"fieldCounts":counts,"originalFieldNames":keys,"parserIdentityVerifiedRows":verified,
        "duplicateParserIdentitiesInSample":duplicates,"identityExtractorSharedWithProductionParser":true,
        "status":if rows.is_empty(){"EMPTY_SAMPLE"}else if verified==rows.len() && duplicates==0{"SOURCE_IDENTITIES_USABLE_IN_SAMPLE"}
            else if verified==0{"NO_USABLE_SOURCE_IDENTITIES_IN_SAMPLE"}else{"MIXED_OR_DUPLICATE_IDENTITIES_IN_SAMPLE"},
        "fullContractCertified":false,"fullSessionCoverageProven":false})
}

pub async fn probe(report: &Report, key: &Credentials, directory: &Path) -> Result<Value, String> {
    let now = now_ms();
    let session = match snipers_core::calendar::day_bounds(&report.request.date)
        .and_then(|(t, _)| snipers_core::calendar::date_at(t))
        .map(snipers_core::calendar::schedule)
    {
        Some(snipers_core::calendar::Day::Open(s)) if s.close_ms < now => s,
        _ => return Err("Solo sesiones cerradas".into()),
    };
    if report.request.underlying != "QQQ"
        || snipers_core::options::underlying(&report.request.ticker) != Some("QQQ")
    {
        return Err("Contrato fuera de alcance".into());
    }
    let path = format!("/v3/trades/{}", report.request.ticker);
    let mut url =
        checked_url(&format!("https://api.massive.com{path}"), &path).map_err(str::to_owned)?;
    url.query_pairs_mut()
        .append_pair("timestamp.gte", &(session.open_ms * 1_000_000).to_string())
        .append_pair("timestamp.lt", &(session.close_ms * 1_000_000).to_string())
        .append_pair("sort", "timestamp")
        .append_pair("order", "asc")
        .append_pair("limit", "1000");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "REST no disponible")?;
    let body = option_activity_history::get(&client, key, url).await?;
    let rows = body["results"]
        .as_array()
        .filter(|a| a.len() <= 1000)
        .ok_or("Respuesta sin filas válidas")?;
    // Preserve source field values without persisting URLs, request IDs or credentials.
    let fields = [
        "sequence_number",
        "id",
        "exchange",
        "sip_timestamp",
        "participant_timestamp",
        "price",
        "size",
        "decimal_size",
        "conditions",
        "correction",
    ];
    let projected: Vec<_> = rows
        .iter()
        .map(|r| {
            let mut out = serde_json::Map::new();
            for name in fields {
                if let Some(v) = r.get(name) {
                    out.insert(name.into(), v.clone());
                }
            }
            Value::Object(out)
        })
        .collect();
    let evidence = json!({"version":"option-original-trade-identity-evidence-v1","receivedMs":now_ms(),"date":report.request.date,
        "ticker":report.request.ticker,"endpointPath":path,"pageLimit":1000,"rows":projected,"nextPagePresent":body["next_url"].as_str().is_some(),
        "scope":"first-page-RTH-sample","executionEnabled":false});
    let file =
        option_activity_dataset::save_json(directory, "original-identity-sample", &evidence)?;
    let mut result = analyze(rows);
    result["date"] = json!(report.request.date);
    result["ticker"] = json!(report.request.ticker);
    result["evidenceFile"] = json!(file);
    result["archivedReportIdentityCounts"] = json!({"unidentified":report.unidentified,"trades":report.trades.len(),"rowsReceived":report.rows_received,
        "retrievalComplete":report.retrieval_complete,"aggregationComplete":report.aggregation_complete});
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_zero_and_empty_are_not_parser_losses() {
        let r = analyze(&[json!({"sequence_number":0,"id":""}), json!({"id":null})]);
        assert_eq!(r["parserIdentityVerifiedRows"], 0);
        assert_eq!(r["fieldCounts"]["sequence:zero"], 1);
        assert_eq!(r["status"], "NO_USABLE_SOURCE_IDENTITIES_IN_SAMPLE");
    }
    #[test]
    fn positive_sequences_and_string_ids_preserve_exact_identity() {
        let rows = [
            json!({"sequence_number":7}),
            json!({"sequence_number":0,"id":"abc","exchange":46}),
        ];
        assert_eq!(reported_identity(&rows[0], 1), Some(("seq:7".into(), true)));
        assert_eq!(
            reported_identity(&rows[1], 2),
            Some(("id:46:abc".into(), true))
        );
        assert_eq!(analyze(&rows)["parserIdentityVerifiedRows"], 2);
        assert_eq!(
            analyze(&[rows[0].clone(), rows[0].clone()])["duplicateParserIdentitiesInSample"],
            1
        );
    }
    #[test]
    fn wrong_types_and_missing_exchange_are_not_invented_ids() {
        let rows = [json!({"sequence_number":"7","id":123}), json!({"id":"abc"})];
        assert_eq!(analyze(&rows)["parserIdentityVerifiedRows"], 0);
        assert_eq!(reported_identity(&rows[1], 2), None);
        assert_eq!(analyze(&[])["status"], "EMPTY_SAMPLE");
    }
}
