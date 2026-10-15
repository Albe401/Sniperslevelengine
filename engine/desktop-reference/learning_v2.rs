//! Passive v2 collector: shared feed, separate ledger, no broker or model activation.
use massive_feed::{analyst_context, live};
use serde_json::{json, Value};
use std::{io::Write, path::PathBuf, process::{Command, Stdio}};

fn decision(snapshot: &live::LiveState, now: u64) -> Value {
    let mut normalized = snapshot.clone();
    if let Some(tick) = normalized.ticks.get_mut(&snapshot.symbol) {
        if tick.trade_ms.is_some_and(|ms| ms > now && ms <= now.saturating_add(1000)) {
            tick.trade_ms = Some(now);
        }
    }
    let opening = snipers_core::calendar::date_at(now).and_then(|d| match snipers_core::calendar::schedule(d) {
        snipers_core::calendar::Day::Open(s) => Some(s.open_ms), _ => None,
    });
    let phase = if opening.is_some_and(|ms| now < ms) { "PRE" } else { "RTH" };
    let prepared = analyst_context::prepare(&normalized, now).ok();
    let day = prepared.as_ref().map(|p| &p["market_interpretation"]["day_or_next_session"]);
    let scores = day.and_then(|d| d["call_score"].as_f64().zip(d["put_score"].as_f64()))
        .filter(|(c,p)| c.is_finite() && p.is_finite() && *c >= 0. && *p >= 0. && *c <= 100. && *p <= 100. && c+p > 0.);
    let baseline = scores.map(|(c,p)| c/(c+p));
    json!({"symbol":snapshot.symbol,"at":now,"phase":phase,
        "direction":baseline.and_then(|c| if c>0.5 {Some(1)} else if c<0.5 {Some(-1)} else {None}),
        "baseline_call":Value::Null,
        "baseline_evidence":{"kind":"score","source":"analyst_context.day_or_next_session",
            "definition":"call_score / (call_score + put_score); concentration score, not calibrated probability",
            "at":now,"symbol":snapshot.symbol,"phase":phase,"normalized_call_score":baseline,
            "status":if baseline.is_some(){"score_only"}else{"missing_score"}},
        "direction_source":"analyst_context.normalized_call_score",
        "features":[{"name":"incumbent_call_score","symbol":snapshot.symbol,
        "phase":phase,"at":now,"value":baseline,"complete":baseline.is_some(),
        "source":"analyst_context.day_or_next_session","definition":"normalized concentration score; not probability"}]})
}

fn batch(directory: &std::path::Path, frames: Value) -> Result<Value, String> {
    let root = directory.join("learning-v2-runtime");
    let package = root.join("learning_v2");
    std::fs::create_dir_all(&package).map_err(|_| "V2_RUNTIME_STORAGE")?;
    for (name, source) in [
        ("__init__.py", ""),
        ("core.py", include_str!("../../../../scripts/learning_v2/core.py")),
        ("adapter.py", include_str!("../../../../scripts/learning_v2/adapter.py")),
        ("collector.py", include_str!("../../../../scripts/learning_v2/collector.py")),
        ("live_batch.py", include_str!("../../../../scripts/learning_v2/live_batch.py")),
    ] {
        let path = package.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(source) {
            std::fs::write(path, source).map_err(|_| "V2_RUNTIME_STORAGE")?;
        }
    }
    let python = std::env::var_os("SNIPERS_LEARNING_PYTHON").map(PathBuf::from).or_else(|| {
        std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home)
            .join(".cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe"))
            .filter(|p| p.is_file())
    }).unwrap_or_else(|| PathBuf::from("python"));
    let mut command = Command::new(python);
    command.arg("-m").arg("learning_v2.live_batch").arg(directory)
        .current_dir(&root).env("PYTHONPATH", &root).env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|_| "V2_PYTHON_UNAVAILABLE")?;
    let bytes = serde_json::to_vec(&frames).map_err(|_| "V2_INVALID_FRAME")?;
    let write = child.stdin.take().ok_or("V2_INPUT_UNAVAILABLE")?.write_all(&bytes);
    if write.is_err() { let _ = child.kill(); let _ = child.wait(); return Err("V2_INPUT_FAILED".into()); }
    let output = child.wait_with_output().map_err(|_| "V2_WORKER_FAILED")?;
    if !output.status.success() { return Err("V2_WORKER_REJECTED_BATCH".into()); }
    serde_json::from_slice(&output.stdout).map_err(|_| "V2_INVALID_STATUS".into())
}

pub async fn run(directory: PathBuf, state: live::SharedState, mut frames: tokio::sync::mpsc::Receiver<live::LiveState>) {
    let mut required: Vec<String> = Vec::new();
    while let Some(snapshot) = frames.recv().await {
        let now = live::now_ms();
        let mut items = vec![json!({"decision":decision(&snapshot,now),"state":snapshot})];
        for symbol in required.iter().filter(|s| *s != &snapshot.symbol).take(20) {
            // Follow-up uses the same subscriptions even after changing the visible asset.
            let frame = live::bot_frame(&state, symbol, now);
            let phase = frame["chartHistory"]["sessions"][0]["openMs"].as_u64()
                .map(|open| if now < open {"PRE"} else {"RTH"}).unwrap_or("CLOSED");
            items.push(json!({"decision":{"symbol":symbol,"at":now,"phase":phase,"features":[]},"state":frame}));
        }
        let dir = directory.clone();
        let result = tauri::async_runtime::spawn_blocking(move || batch(&dir,json!({"received_ms":now,"frames":items}))).await;
        let status = match result {
            Ok(Ok(status)) => { required = status["required_symbols"].as_array().into_iter().flatten().filter_map(|s|s.as_str().map(str::to_owned)).take(20).collect(); status },
            _ => json!({"integrated":true,"observed_ms":now,"automatic_activation":false,"error":"V2_COLLECTOR_UNAVAILABLE"}),
        };
        let _ = super::learning::write_json(&directory,"learning-v2-status.json",&status);
    }
}
