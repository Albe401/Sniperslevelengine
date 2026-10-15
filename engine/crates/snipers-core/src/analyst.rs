//! Optional narrative only. This module performs no network requests or orders.
use serde::Serialize;
use serde_json::{json, Value};

pub const MARKET_INSTRUCTIONS: &str = include_str!("../resources/analyst-market.txt");
pub const SYSTEM_INSTRUCTIONS: &str = include_str!("../resources/analyst-system.txt");
pub const MAX_SNAPSHOT_CHARS: usize = 85_000;

#[derive(Clone, Copy)]
pub enum Mode {
    Market,
    System,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Model,
    Identity,
    Stale,
    Payload,
    SensitiveField,
    Budget,
    Response,
}

// Defense in depth for engine DTOs. This is not a general-purpose secret scrubber:
// callers must pass only market data, never environment/configuration objects.
pub fn validate(value: &Value, depth: usize) -> Result<(), Error> {
    if depth > 24 {
        return Err(Error::Payload);
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let key: String = key
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect();
                if [
                    "apikey",
                    "secret",
                    "password",
                    "authorization",
                    "accesstoken",
                    "refreshtoken",
                    "credential",
                    "connectionstring",
                ]
                .iter()
                .any(|word| key.contains(word))
                {
                    return Err(Error::SensitiveField);
                }
                validate(value, depth + 1)?;
            }
        }
        Value::Array(items) => {
            if items.len() > 10_000 {
                return Err(Error::Budget);
            }
            for item in items {
                validate(item, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}

/// Builds a request for inspection only; sending remains disabled in this delivery.
/// `event_ms` is the engine's evidence timestamp, not the time this DTO was built.
pub fn request(
    model: &str,
    symbol: &str,
    payload: &Value,
    event_ms: u64,
    now: u64,
    verified: bool,
    mode: Mode,
) -> Result<Value, Error> {
    if model.trim().is_empty() || model.len() > 128 || model.chars().any(char::is_control) {
        return Err(Error::Model);
    }
    if symbol.is_empty()
        || symbol.len() > 16
        || !symbol
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'.')
        || payload.get("symbol").and_then(Value::as_str) != Some(symbol)
    {
        return Err(Error::Identity);
    }
    if !verified
        || event_ms > now
        || now - event_ms > 15_000
        || crate::calendar::date_at(now).is_none()
        || crate::calendar::date_at(event_ms) != crate::calendar::date_at(now)
    {
        return Err(Error::Stale);
    }
    validate(payload, 0)?;
    let serialized = serde_json::to_string(payload).map_err(|_| Error::Payload)?;
    // Never truncate decision gates to fit a model context.
    if serialized.chars().count() > MAX_SNAPSHOT_CHARS {
        return Err(Error::Budget);
    }
    let mut package = json!({"source":"SNIPERS_LEVEL_ENGINE", "symbol":symbol, "evidence_timestamp_ms":event_ms, "ai_request_generated_at_ms":now});
    let (field, instructions) = match mode {
        Mode::Market => ("market_engine_data", MARKET_INSTRUCTIONS),
        Mode::System => ("SYSTEM_SNAPSHOT", SYSTEM_INSTRUCTIONS),
    };
    package[field] = payload.clone();
    Ok(
        json!({"model":model,"instructions":instructions,"input":package.to_string(),"max_output_tokens":3500,"store":false}),
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Narrative {
    pub analysis: String,
    pub source: &'static str,
    pub advisory_only: bool,
}

/// Raw Responses API output, not the SDK's synthesized `output_text` property.
/// A narrative cannot grant admission: no decision/permission field is returned.
pub fn response(raw: &str) -> Result<Narrative, Error> {
    if raw.len() > 1_000_000 {
        return Err(Error::Budget);
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| Error::Response)?;
    if value.get("status").and_then(Value::as_str) != Some("completed")
        || value.get("error").is_some_and(|v| !v.is_null())
        || value
            .get("incomplete_details")
            .is_some_and(|v| !v.is_null())
    {
        return Err(Error::Response);
    }
    let output = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or(Error::Response)?;
    let mut parts = Vec::new();
    for item in output {
        match item.get("type").and_then(Value::as_str) {
            Some("reasoning") => continue,
            Some("message") => (),
            _ => return Err(Error::Response),
        }
        if item.get("role").and_then(Value::as_str) != Some("assistant")
            || item.get("status").and_then(Value::as_str) != Some("completed")
        {
            return Err(Error::Response);
        }
        for content in item
            .get("content")
            .and_then(Value::as_array)
            .ok_or(Error::Response)?
        {
            if content.get("type").and_then(Value::as_str) != Some("output_text") {
                return Err(Error::Response);
            }
            parts.push(
                content
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(Error::Response)?,
            );
        }
    }
    let analysis = parts.join("\n").trim().to_string();
    if analysis.is_empty() || analysis.chars().count() > 50_000 {
        return Err(Error::Response);
    }
    Ok(Narrative {
        analysis,
        source: "OPTIONAL_AI_NARRATIVE",
        advisory_only: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: u64 = 1_790_260_000_000;
    #[test]
    fn request_preserves_engine_gates_and_has_no_tools_or_storage() {
        let payload =
            json!({"symbol":"QQQ","decision_runtime":{"entry_confirmed":false,"action":"WAIT"}});
        let built = request(
            "configured-model",
            "QQQ",
            &payload,
            NOW,
            NOW,
            true,
            Mode::System,
        )
        .unwrap();
        let input: Value = serde_json::from_str(built["input"].as_str().unwrap()).unwrap();
        assert_eq!(input["SYSTEM_SNAPSHOT"], payload);
        assert_eq!(built["store"], false);
        assert_eq!(built["max_output_tokens"], 3500);
        assert!(built.get("tools").is_none());
        assert_eq!(built["instructions"], SYSTEM_INSTRUCTIONS);
    }
    #[test]
    fn rejects_mixed_stale_future_sensitive_and_oversized_data() {
        let good = json!({"symbol":"QQQ"});
        assert_eq!(
            request("m", "SPY", &good, NOW, NOW, true, Mode::Market),
            Err(Error::Identity)
        );
        for (event, verified) in [(NOW - 15_001, true), (NOW + 1, true), (NOW, false)] {
            assert_eq!(
                request("m", "QQQ", &good, event, NOW, verified, Mode::Market),
                Err(Error::Stale)
            );
        }
        let secret = json!({"symbol":"QQQ","nested":[{"OPENAI_API_KEY":"placeholder"}]});
        assert_eq!(
            request("m", "QQQ", &secret, NOW, NOW, true, Mode::Market),
            Err(Error::SensitiveField)
        );
        let big = json!({"symbol":"QQQ","context":"x".repeat(MAX_SNAPSHOT_CHARS)});
        assert_eq!(
            request("m", "QQQ", &big, NOW, NOW, true, Mode::Market),
            Err(Error::Budget)
        );
    }
    #[test]
    fn completed_text_only_and_no_permission_in_output() {
        let good = json!({"status":"completed","output":[{"type":"reasoning"},{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"WAIT: evidencia insuficiente."}]}]});
        let result = response(&good.to_string()).unwrap();
        assert!(result.advisory_only);
        assert!(serde_json::to_value(result)
            .unwrap()
            .get("entry_confirmed")
            .is_none());
        for status in ["incomplete", "failed", "in_progress"] {
            let mut bad = good.clone();
            bad["status"] = json!(status);
            assert!(response(&bad.to_string()).is_err());
        }
        let mut refusal = good.clone();
        refusal["output"][1]["content"][0]["type"] = json!("refusal");
        assert!(response(&refusal.to_string()).is_err());
        let mut tool = good;
        tool["output"][1]["type"] = json!("function_call");
        assert!(response(&tool.to_string()).is_err());
        assert!(response("{broken").is_err());
    }
}
