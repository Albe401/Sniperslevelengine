//! Separate broker outbox. A reserved ID is never automatically submitted twice,
//! including after a crash or an uncertain HTTP response.
use rusqlite::{params, Connection};
use std::path::Path;
pub struct BrokerOrders(Connection);
impl BrokerOrders {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let c = Connection::open(path)?;
        c.busy_timeout(std::time::Duration::from_secs(5))?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS alpaca_paper_outbox(client_id TEXT PRIMARY KEY, intent TEXT NOT NULL, state TEXT NOT NULL, response TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS alpaca_paper_observations(client_id TEXT PRIMARY KEY, broker_status TEXT, broker_response TEXT, last_error TEXT, checked_at REAL NOT NULL);")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS alpaca_bot_runtime(id INTEGER PRIMARY KEY CHECK(id=1),payload TEXT NOT NULL);")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS alpaca_bot_sessions(buy_id TEXT PRIMARY KEY,payload TEXT NOT NULL,updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS alpaca_bot_assets(symbol TEXT PRIMARY KEY,payload TEXT NOT NULL);")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS alpaca_bot_universe(session TEXT PRIMARY KEY,payload TEXT NOT NULL);")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS alpaca_bot_decisions(id INTEGER PRIMARY KEY,buy_id TEXT NOT NULL,event_key TEXT NOT NULL,at_ms INTEGER NOT NULL,payload TEXT NOT NULL,UNIQUE(buy_id,event_key));")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS alpaca_bot_risk_locks(session TEXT PRIMARY KEY,payload TEXT NOT NULL);")?;
        Ok(Self(c))
    }
    pub fn reserve(&self, id: &str, intent: &str) -> rusqlite::Result<bool> {
        Ok(self.0.execute("INSERT OR IGNORE INTO alpaca_paper_outbox(client_id,intent,state) VALUES(?1,?2,'pending')",params![id,intent])?==1)
    }
    pub fn load_assets(&self)->rusqlite::Result<Vec<String>> {
        let mut q=self.0.prepare("SELECT payload FROM alpaca_bot_assets ORDER BY symbol")?;
        let rows=q.query_map([],|r|r.get(0))?.collect();rows
    }
    pub fn load_bot_universe(&self,date:&str)->rusqlite::Result<Option<String>>{
        use rusqlite::OptionalExtension;
        self.0.query_row("SELECT payload FROM alpaca_bot_universe WHERE session=?1",[date],|r|r.get(0)).optional()
    }
    pub fn save_bot_universe(&self,date:&str,payload:&str)->rusqlite::Result<()>{
        let tx=self.0.unchecked_transaction()?;
        tx.execute("INSERT OR IGNORE INTO alpaca_bot_universe(session,payload) VALUES(?1,?2)",params![date,payload])?;
        let saved:String=tx.query_row("SELECT payload FROM alpaca_bot_universe WHERE session=?1",[date],|r|r.get(0))?;
        if saved!=payload{return Err(rusqlite::Error::InvalidQuery);}
        tx.commit()
    }
    pub fn load_runtime(&self) -> rusqlite::Result<Option<String>> {
        use rusqlite::OptionalExtension;
        self.0
            .query_row(
                "SELECT payload FROM alpaca_bot_runtime WHERE id=1",
                [],
                |r| r.get(0),
            )
            .optional()
    }
    pub fn save_runtime(&self, payload: &str) -> rusqlite::Result<()> {
        let value: serde_json::Value =
            serde_json::from_str(payload).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let tx = self.0.unchecked_transaction()?;
        tx.execute("INSERT INTO alpaca_bot_runtime(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",[payload])?;
        if let Some(id) = value["buy_id"].as_str().filter(|s| !s.is_empty()) {
            tx.execute("INSERT INTO alpaca_bot_sessions(buy_id,payload) VALUES(?1,?2) ON CONFLICT(buy_id) DO UPDATE SET payload=excluded.payload,updated_at=CURRENT_TIMESTAMP",params![id,payload])?;
        }
        if let Some(symbol)=value["symbol"].as_str() {
            let root=snipers_core::options::underlying(&format!("O:{symbol}")).map(str::to_owned).ok_or(rusqlite::Error::InvalidQuery)?;
            tx.execute("INSERT INTO alpaca_bot_assets(symbol,payload) VALUES(?1,?2) ON CONFLICT(symbol) DO UPDATE SET payload=excluded.payload",params![root,payload])?;
        }
        tx.commit()
    }
    pub fn bot_history(&self) -> rusqlite::Result<Vec<serde_json::Value>> {
        let mut statement = self
            .0
            .prepare("SELECT payload FROM alpaca_bot_sessions ORDER BY rowid DESC LIMIT 100")?;
        let rows = statement.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(|_| rusqlite::Error::InvalidQuery))
            .collect()
    }
    /// Append-only evidence. Retries cannot replace the first observation.
    pub fn record_bot_decision(&self, id: &str, key: &str, at: u64, payload: &str) -> rusqlite::Result<()> {
        serde_json::from_str::<serde_json::Value>(payload).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let at=i64::try_from(at).map_err(|_|rusqlite::Error::InvalidQuery)?;
        self.0.execute("INSERT OR IGNORE INTO alpaca_bot_decisions(buy_id,event_key,at_ms,payload) VALUES(?1,?2,?3,?4)",params![id,key,at,payload])?;
        Ok(())
    }
    pub fn bot_decisions(&self, id: &str) -> rusqlite::Result<Vec<serde_json::Value>> {
        let mut q=self.0.prepare("SELECT payload FROM alpaca_bot_decisions WHERE buy_id=?1 ORDER BY at_ms,id")?;
        let rows=q.query_map([id],|r|r.get::<_,String>(0))?.map(|r|serde_json::from_str(&r?).map_err(|_|rusqlite::Error::InvalidQuery)).collect();rows
    }
    /// Account all completed trades, including overnight positions closed today.
    /// A triggered lock survives restart, recovery in P/L and changes to the limit.
    pub fn daily_risk(&self,date:&str,limit:f64)->rusqlite::Result<serde_json::Value>{
        if !limit.is_finite()||limit<=0. {return Err(rusqlite::Error::InvalidQuery);}
        let tx=self.0.unchecked_transaction()?;
        let mut pnl=0.;
        {
            let mut q=tx.prepare("SELECT payload FROM alpaca_bot_sessions")?;
            for row in q.query_map([],|r|r.get::<_,String>(0))?{
                let v:serde_json::Value=serde_json::from_str(&row?).map_err(|_|rusqlite::Error::InvalidQuery)?;
                let Some(exit)=v["exit_time"].as_str() else {continue;};
                let ms=snipers_core::calendar::rfc3339_ms(exit).ok_or(rusqlite::Error::InvalidQuery)?;
                if snipers_core::calendar::date_at(ms).map(|d|d.to_string()).as_deref()!=Some(date){continue;}
                let entry=v["entry_premium"].as_f64().filter(|n|n.is_finite()&&*n>0.).ok_or(rusqlite::Error::InvalidQuery)?;
                let exit=v["exit_premium"].as_f64().filter(|n|n.is_finite()&&*n>=0.).ok_or(rusqlite::Error::InvalidQuery)?;
                pnl+=(exit-entry)*100.;
            }
        }
        if !pnl.is_finite(){return Err(rusqlite::Error::InvalidQuery);}
        if pnl<=-limit {tx.execute("INSERT OR IGNORE INTO alpaca_bot_risk_locks(session,payload) VALUES(?1,?2)",params![date,serde_json::json!({"limit":limit,"realized_pnl":pnl}).to_string()])?;}
        let blocked:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM alpaca_bot_risk_locks WHERE session=?1)",[date],|r|r.get(0))?;
        tx.commit()?;
        Ok(serde_json::json!({"date":date,"realized_pnl":pnl,"limit":limit,"blocked":blocked,"basis":"filled-premiums-before-fees"}))
    }
    pub fn not_submitted(&self, id: &str, reason: &str) -> rusqlite::Result<()> {
        self.0.execute("UPDATE alpaca_paper_outbox SET state='not_sent',response=?2,updated_at=CURRENT_TIMESTAMP WHERE client_id=?1 AND state='pending'",params![id,reason])?;
        Ok(())
    }
    pub fn was_not_submitted(&self, id: &str) -> rusqlite::Result<bool> {
        use rusqlite::OptionalExtension;
        Ok(self
            .0
            .query_row(
                "SELECT state='not_sent' FROM alpaca_paper_outbox WHERE client_id=?1",
                [id],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false))
    }
    pub fn finish(&self, id: &str, response: &str, success: bool) -> rusqlite::Result<()> {
        self.0.execute("UPDATE alpaca_paper_outbox SET state=?2,response=?3,updated_at=CURRENT_TIMESTAMP WHERE client_id=?1",params![id,if success{"acknowledged"}else{"needs_review"},response])?;
        Ok(())
    }
    pub fn recent(&self) -> rusqlite::Result<Vec<serde_json::Value>> {
        let mut s=self.0.prepare("SELECT o.client_id,o.intent,o.state,o.response,o.created_at,o.updated_at,b.broker_status,b.broker_response,b.last_error,b.checked_at FROM alpaca_paper_outbox o LEFT JOIN alpaca_paper_observations b ON b.client_id=o.client_id ORDER BY o.rowid DESC LIMIT 100")?;
        let rows=s.query_map([],|r|Ok(serde_json::json!({"client_order_id":r.get::<_,String>(0)?,"intent":r.get::<_,String>(1)?,"state":r.get::<_,String>(2)?,"response":r.get::<_,Option<String>>(3)?,"created_at":r.get::<_,String>(4)?,"updated_at":r.get::<_,String>(5)?,"broker_status":r.get::<_,Option<String>>(6)?,"broker_response":r.get::<_,Option<String>>(7)?,"last_error":r.get::<_,Option<String>>(8)?,"checked_at_julian":r.get::<_,Option<f64>>(9)?})))?;
        rows.collect()
    }
    pub fn reconciliation_candidates(&self) -> rusqlite::Result<Vec<String>> {
        let mut s=self.0.prepare("SELECT o.client_id FROM alpaca_paper_outbox o LEFT JOIN alpaca_paper_observations b ON b.client_id=o.client_id WHERE o.state <> 'not_sent' AND (b.broker_status IS NULL OR b.broker_status NOT IN ('filled','canceled','expired','rejected','replaced')) ORDER BY COALESCE(b.checked_at,0),o.rowid LIMIT 20")?;
        let rows = s.query_map([], |r| r.get(0))?;
        rows.collect()
    }
    /// Preserve the last verified broker observation when a subsequent lookup fails.
    pub fn observe(
        &self,
        id: &str,
        result: Result<&serde_json::Value, &str>,
    ) -> rusqlite::Result<()> {
        match result {
            Ok(v) => {
                if v["client_order_id"].as_str() != Some(id) || v["status"].as_str().is_none() {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                self.0.execute("INSERT INTO alpaca_paper_observations(client_id,broker_status,broker_response,checked_at) VALUES(?1,?2,?3,julianday('now')) ON CONFLICT(client_id) DO UPDATE SET broker_status=excluded.broker_status,broker_response=excluded.broker_response,last_error=NULL,checked_at=excluded.checked_at",params![id,v["status"].as_str(),v.to_string()])?;
            }
            Err(e) => {
                self.0.execute("INSERT INTO alpaca_paper_observations(client_id,last_error,checked_at) VALUES(?1,?2,julianday('now')) ON CONFLICT(client_id) DO UPDATE SET last_error=excluded.last_error,checked_at=excluded.checked_at",params![id,e])?;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daily_loss_lock_includes_carried_trades_and_survives_restart_and_recovery(){
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("risk.sqlite");
        let db=BrokerOrders::open(&path).unwrap();
        db.save_runtime(r#"{"buy_id":"loss","date":"2026-09-29","exit_time":"2026-09-30T17:30:00Z","entry_premium":5,"exit_premium":3}"#).unwrap();
        assert_eq!(db.daily_risk("2026-09-30",200.).unwrap()["blocked"],true);drop(db);
        let db=BrokerOrders::open(&path).unwrap();
        db.save_runtime(r#"{"buy_id":"gain","exit_time":"2026-09-30T18:00:00Z","entry_premium":1,"exit_premium":4}"#).unwrap();
        let risk=db.daily_risk("2026-09-30",500.).unwrap();assert_eq!(risk["realized_pnl"],100.);assert_eq!(risk["blocked"],true);
        assert_eq!(db.daily_risk("2026-10-01",200.).unwrap()["blocked"],false);
        assert!(db.daily_risk("2026-10-01",f64::NAN).is_err());
    }
    #[test]
    fn decision_evidence_survives_restart_and_retry_without_rewriting() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("audit.sqlite");
        let db=BrokerOrders::open(&path).unwrap();
        db.record_bot_decision("trade","entry",1,r#"{"action":"entry","spot":230.136}"#).unwrap();
        db.record_bot_decision("trade","entry",2,r#"{"action":"entry","spot":999}"#).unwrap();
        db.record_bot_decision("trade","exit:stale",3,r#"{"action":"exit","reason":"stale"}"#).unwrap();
        db.record_bot_decision("other","entry",4,"{}").unwrap();
        assert!(db.record_bot_decision("trade","invalid",5,"broken").is_err());drop(db);
        let db=BrokerOrders::open(&path).unwrap();let rows=db.bot_decisions("trade").unwrap();
        assert_eq!(rows.len(),2);assert_eq!(rows[0]["spot"],230.136);assert_eq!(rows[1]["reason"],"stale");
    }
    #[test] fn concurrent_asset_runtime_survives_restart_without_overwriting_other_position(){
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("portfolio.sqlite");let db=BrokerOrders::open(&path).unwrap();
        db.save_runtime(r#"{"buy_id":"one","symbol":"QQQ261016C00740000","phase":"open","reserved_premium":900}"#).unwrap();
        db.save_runtime(r#"{"buy_id":"two","symbol":"SPY261016C00600000","phase":"buy_pending","reserved_premium":950}"#).unwrap();drop(db);
        let db=BrokerOrders::open(&path).unwrap();let rows=db.load_assets().unwrap();assert_eq!(rows.len(),2);assert_eq!(db.bot_history().unwrap().len(),2);
        assert!(rows.iter().any(|v|v.contains("QQQ")));assert!(rows.iter().any(|v|v.contains("SPY")));
        db.save_bot_universe("2026-09-29","first-selection").unwrap();
        assert!(db.save_bot_universe("2026-09-29","replacement").is_err());
        assert_eq!(db.load_bot_universe("2026-09-29").unwrap().as_deref(),Some("first-selection"));
    }
    #[test]
    fn only_explicit_preflight_failure_is_safe_to_retire() {
        let db = BrokerOrders::open(Path::new(":memory:")).unwrap();
        assert!(!db.was_not_submitted("absent").unwrap());
        db.reserve("not-sent", "{}").unwrap();
        assert!(!db.was_not_submitted("not-sent").unwrap());
        db.not_submitted("not-sent", "stale quote before POST")
            .unwrap();
        assert!(db.was_not_submitted("not-sent").unwrap());
        assert!(db.reconciliation_candidates().unwrap().is_empty());
        assert!(!db.reserve("not-sent", "{}").unwrap());
        db.reserve("uncertain", "{}").unwrap();
        db.finish("uncertain", "timeout", false).unwrap();
        db.not_submitted("uncertain", "cannot overwrite uncertain POST")
            .unwrap();
        assert!(!db.was_not_submitted("uncertain").unwrap());
    }
    #[test]
    fn bot_history_preserves_both_entries_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.sqlite");
        let db = BrokerOrders::open(&path).unwrap();
        db.save_runtime(r#"{"buy_id":"first","phase":"open"}"#)
            .unwrap();
        db.save_runtime(r#"{"buy_id":"first","phase":"done","exit_time":"2026-09-28T14:00:00Z"}"#)
            .unwrap();
        db.save_runtime(r#"{"buy_id":"second","phase":"buy_pending"}"#)
            .unwrap();
        assert!(db.save_runtime("invalid json").is_err());
        drop(db);
        let db = BrokerOrders::open(&path).unwrap();
        let rows = db.bot_history().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["buy_id"], "second");
        assert_eq!(rows[1]["phase"], "done");
        assert_eq!(rows[1]["exit_time"], "2026-09-28T14:00:00Z");
    }
    #[test]
    fn reconciliation_recovers_partial_fills_without_releasing_id() {
        let db = BrokerOrders::open(Path::new(":memory:")).unwrap();
        db.reserve("snipers-a", "{}").unwrap();
        db.observe("snipers-a",Ok(&serde_json::json!({"client_order_id":"snipers-a","status":"partially_filled","filled_qty":"1"}))).unwrap();
        assert_eq!(db.reconciliation_candidates().unwrap(), vec!["snipers-a"]);
        db.observe("snipers-a", Err("HTTP 503")).unwrap();
        let rows = db.recent().unwrap();
        assert_eq!(rows[0]["broker_status"], "partially_filled");
        assert_eq!(rows[0]["last_error"], "HTTP 503");
        db.observe("snipers-a",Ok(&serde_json::json!({"client_order_id":"snipers-a","status":"filled","filled_qty":"2"}))).unwrap();
        assert!(db.reconciliation_candidates().unwrap().is_empty());
        assert!(!db.reserve("snipers-a", "{}").unwrap());
        assert!(db.recent().unwrap()[0]["last_error"].is_null());
    }
    #[test]
    fn missing_order_and_foreign_response_never_allow_resubmission() {
        let db = BrokerOrders::open(Path::new(":memory:")).unwrap();
        db.reserve("snipers-a", "{}").unwrap();
        db.observe("snipers-a", Err("HTTP 404")).unwrap();
        assert_eq!(db.reconciliation_candidates().unwrap(), vec!["snipers-a"]);
        assert!(db
            .observe(
                "snipers-a",
                Ok(&serde_json::json!({"client_order_id":"foreign","status":"filled"}))
            )
            .is_err());
        assert!(!db.reserve("snipers-a", "{}").unwrap());
    }
    #[test]
    fn reconciliation_reaches_older_orders_outside_recent_page() {
        let db = BrokerOrders::open(Path::new(":memory:")).unwrap();
        for n in 0..105 {
            db.reserve(&format!("snipers-{n}"), "{}").unwrap();
        }
        assert_eq!(db.recent().unwrap().len(), 100);
        assert_eq!(db.reconciliation_candidates().unwrap()[0], "snipers-0");
        for id in db.reconciliation_candidates().unwrap() {
            db.observe(&id, Err("network")).unwrap();
        }
        assert_eq!(db.reconciliation_candidates().unwrap()[0], "snipers-20");
    }
    #[test]
    fn reserve_survives_restart_and_rejects_changed_payload() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("broker.sqlite");
        let db = BrokerOrders::open(&p).unwrap();
        assert!(db.reserve("snipers-1", "original").unwrap());
        drop(db);
        let db = BrokerOrders::open(&p).unwrap();
        assert!(!db.reserve("snipers-1", "changed").unwrap());
        assert_eq!(db.recent().unwrap()[0]["state"], "pending");
        db.finish("snipers-1", "timeout", false).unwrap();
        assert!(!db.reserve("snipers-1", "original").unwrap());
        assert_eq!(db.recent().unwrap()[0]["state"], "needs_review");
    }
    #[test]
    fn independent_ids_and_broker_acknowledgment() {
        let db = BrokerOrders::open(Path::new(":memory:")).unwrap();
        assert!(db.reserve("snipers-1", "{}").unwrap());
        assert!(db.reserve("snipers-2", "{}").unwrap());
        db.finish("snipers-1", "{}", true).unwrap();
        assert_eq!(db.recent().unwrap().len(), 2);
    }
    #[test]
    fn concurrent_connections_allow_one_reservation() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("broker.sqlite");
        let a = BrokerOrders::open(&p).unwrap();
        let b = BrokerOrders::open(&p).unwrap();
        assert!(a.reserve("snipers-same", "{}").unwrap());
        assert!(!b.reserve("snipers-same", "{}").unwrap());
    }
}
