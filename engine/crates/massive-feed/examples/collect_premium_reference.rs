//! Resumable historical premium collection, bounded concurrency, immutable evidence.
use futures_util::{stream, StreamExt};
use massive_feed::{
    credentials::Credentials,
    live::now_ms,
    option_activity_dataset::{self as dataset, Dataset},
};
use snipers_core::calendar;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

fn existing(directory: &Path, dates: &BTreeSet<String>) -> Result<BTreeSet<String>, String> {
    let mut completed = BTreeSet::new();
    if !directory.exists() {
        return Ok(completed);
    }
    for entry in std::fs::read_dir(directory).map_err(|_| "Destino ilegible")? {
        let p = entry.map_err(|_| "Destino ilegible")?.path();
        if !p
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("dataset-") && s.ends_with(".json"))
        {
            continue;
        }
        if std::fs::metadata(&p).map_err(|_| "Dataset ilegible")?.len() > 64_000_000 {
            return Err("Dataset demasiado grande".into());
        }
        let d: Dataset = serde_json::from_slice(&std::fs::read(p).map_err(|_| "Dataset ilegible")?)
            .map_err(|_| "Dataset incompatible")?;
        if d.underlying != "QQQ"
            || d.version != "option-activity-dataset-v1"
            || !dates.contains(&d.date)
            || !completed.insert(d.date.clone())
        {
            return Err("Dataset repetido o ajeno al plan".into());
        }
    }
    Ok(completed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resume_skips_exact_archives_and_rejects_duplicate_or_foreign_dates() {
        let root = PathBuf::from(format!(
            "target/premium-resume-test-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let dates = BTreeSet::from(["2026-10-09".to_owned()]);
        let mut data = serde_json::json!({"version":"option-activity-dataset-v1","underlying":"QQQ","date":"2026-10-09","collectedMs":1,
            "selection":"fixture","openingReferencePrice":null,"openingReferenceAvailableMs":null,"stockMinutes":0,
            "reports":[],"aggregateReports":[],"contextReports":[],"skippedWindows":{},"samples":[],"errors":[]});
        let first = root.join("dataset-fixture.json");
        let second = root.join("dataset-duplicate.json");
        std::fs::write(&first, data.to_string()).unwrap();
        assert_eq!(existing(&root, &dates).unwrap(), dates);
        std::fs::write(&second, data.to_string()).unwrap();
        assert!(existing(&root, &dates).is_err());
        std::fs::remove_file(&second).unwrap();
        data["date"] = serde_json::json!("2026-10-08");
        std::fs::write(&first, data.to_string()).unwrap();
        assert!(existing(&root, &dates).is_err());
        std::fs::remove_file(&first).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("Usage: collect_premium_reference CUTOFF_DATE SESSIONS:20..60 OUTPUT_DIR CONCURRENCY:1..4; QQQ 3-per-side, 100 pages; existing datasets skipped, not certified by this command".into());
    }
    let count = args[1].parse::<usize>()?;
    let concurrency = args[3].parse::<usize>()?;
    if !(20..=60).contains(&count) || !(1..=4).contains(&concurrency) {
        return Err("Plan fuera de límites".into());
    }
    let dates = dataset::previous_sessions(&args[0], count)?;
    let cutoff =
        calendar::legacy_eastern_ms(&format!("{} 09:30:00", args[0])).ok_or("Corte inválido")?;
    if cutoff <= now_ms() {
        return Err("El corte debe seguir siendo futuro para referencia prospectiva".into());
    }
    let directory = PathBuf::from(&args[2]);
    let completed = existing(&directory, &dates.iter().cloned().collect())?;
    let pending: Vec<_> = dates
        .iter()
        .filter(|d| !completed.contains(*d))
        .cloned()
        .collect();
    let key = Credentials::load()?;
    let plan = serde_json::json!({"version":"premium-reference-collection-plan-v1","createdMs":now_ms(),"cutoffMs":cutoff,"dates":dates,"sessionsRequested":count,
        "alreadyArchivedSessions":completed,"pendingSessions":pending.len(),"concurrency":concurrency,"maxPagesPerContract":100,"contractsPerSession":6,"executionEnabled":false,"ordersSent":0});
    dataset::save_json(&directory, "collection-plan", &plan)?;
    let tasks=pending.into_iter().map(|date| {
        let key=&key; let directory=&directory;
        async move {
            let result=dataset::collect_session_breadth(key,"QQQ",&date,directory,100,true,false,3).await;
            match result {
                Ok(data)=>match dataset::save_json(directory,"dataset",&data) {
                    Ok(file)=>serde_json::json!({"date":date,"status":"ARCHIVED_NOT_YET_CERTIFIED","reports":data.reports.len(),"errors":data.errors,"file":file}),
                    Err(e)=>serde_json::json!({"date":date,"status":"SAVE_FAILED","reason":e})},
                Err(e)=>serde_json::json!({"date":date,"status":"COLLECTION_PENDING","reason":e})
            }
        }
    });
    let mut jobs = stream::iter(tasks).buffer_unordered(concurrency);
    let mut results = vec![];
    while let Some(result) = jobs.next().await {
        println!("{}", result);
        dataset::save_json(&directory, "collection-session", &result)?;
        results.push(result);
    }
    let report = serde_json::json!({"version":"premium-reference-collection-run-v1","completedMs":now_ms(),"plan":plan,"results":results,
        "status":"COLLECTION_FINISHED_REQUIRES_CERTIFICATION","executionEnabled":false,"formulaChanged":false,"ordersSent":0});
    let file = dataset::save_json(&directory, "collection-run", &report)?;
    println!(
        "{}",
        serde_json::json!({"status":report["status"],"file":file})
    );
    Ok(())
}
