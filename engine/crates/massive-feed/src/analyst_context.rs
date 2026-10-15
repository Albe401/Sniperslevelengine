//! Local-only narrative context from typed engine outputs; no config or transport.
use crate::live::LiveState;
use serde::Serialize;
use serde_json::{json, Value};
use snipers_core::{analyst::Error, analyst_compact, horizons::Horizon};
fn snake<T: Serialize>(value: &T) -> Value {
    fn visit(v: Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.into_iter()
                    .map(|(k, v)| {
                        let key = if k.chars().all(|c| !c.is_ascii_lowercase()) {
                            k
                        } else {
                            k.chars()
                                .flat_map(|c| {
                                    if c.is_ascii_uppercase() {
                                        vec!['_', c.to_ascii_lowercase()]
                                    } else {
                                        vec![c]
                                    }
                                })
                                .collect()
                        };
                        (key, visit(v))
                    })
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.into_iter().map(visit).collect()),
            v => v,
        }
    }
    visit(serde_json::to_value(value).expect("finite typed engine context"))
}
fn fresh(ms: Option<u64>, now: u64, age: u64) -> bool {
    ms.is_some_and(|ms| {
        ms <= now
            && now - ms <= age
            && snipers_core::calendar::date_at(ms) == snipers_core::calendar::date_at(now)
    })
}
fn wall(w: Option<&snipers_core::structure::Wall>) -> Value {
    w.map(|w|json!({"strike":w.strike,"wall_score":w.score,"wall_strength":w.strength,"wall_ready":w.ready,"wall_oi":w.oi,"wall_volume":w.volume})).unwrap_or(Value::Null)
}
fn liquidity(r: &snipers_core::option_liquidity::Read) -> Value {
    json!({"status":r.status,"liquidity_filter_pass":r.ready,"option_liquidity_confirmed":r.ready,"contracts_tracked":r.contracts_tracked,"contracts_with_bid_ask":r.quoted,"contracts_with_recent_bid_ask":r.recent,"contracts_with_depth":r.depth,"quote_coverage_pct":r.quote_coverage_pct,"recent_quote_coverage_pct":r.recent_coverage_pct,"depth_coverage_pct":r.depth_coverage_pct,"average_spread_pct":r.average_spread_pct})
}
fn wall_flow(w: Option<&crate::interpretation::WallFlow>) -> Value {
    let Some(w) = w else {
        return json!({});
    };
    let mut windows = json!({});
    for row in &w.windows {
        windows[format!("{}m", row.minutes)] =
            json!({"call_zone":snake(&row.call_zone),"put_zone":snake(&row.put_zone)});
    }
    json!({"source":"MASSIVE_WEBSOCKET","entry_confirmed":false,"windows":windows,"subscription":{"status":if w.available{"COVERAGE_AVAILABLE"}else{"WAITING"}}})
}
fn horizon(s: &LiveState, h: &Horizon, now: u64) -> Value {
    let st = &h.structure;
    let ex = &h.exposure;
    let current =
        s.option_snapshot_status.phase == "ready" && fresh(Some(h.observed_ms), now, 900_000);
    let adjusted = s.adjusted_readings.get(&h.mode);
    let reading = adjusted.map(|r| &r.reading).unwrap_or(&st.reading);
    let mut out = snake(reading);
    out["symbol"] = json!(s.symbol);
    out["spot"] = json!(ex.spot);
    out["horizon"] = json!(h.mode);
    out["display_label"] = json!(h.label);
    out["expirations"] = json!(h.expirations);
    out["contracts_loaded"] = json!(ex.contracts);
    out["fallback_used"] = json!(h.mode == "NEXT_SESSION");
    out["fallback_reason"] = if h.mode == "NEXT_SESSION" {
        json!("No hay 0DTE; próxima sesión disponible")
    } else {
        Value::Null
    };
    out["error"] = if current {
        Value::Null
    } else {
        json!("SNAPSHOT_STALE_OR_INCOMPLETE")
    };
    if !current {
        out["side"] = json!("N/A");
        out["reading"] = json!("Esperando datos vigentes");
        out["call_score"] = Value::Null;
        out["put_score"] = Value::Null;
        out["confidence"] = Value::Null;
    }
    let mut levels = json!(st.selected);
    levels["call_wall"] = json!(st.call_wall.as_ref().map(|w| w.strike));
    levels["put_wall"] = json!(st.put_wall.as_ref().map(|w| w.strike));
    levels["liquidity_magnet"] = json!(st.magnet);
    out["levels"] = levels;
    out["main_options_magnet"] = json!(st.selected.get("M1").copied().flatten());
    out["call_wall_detail"] = wall(st.call_wall.as_ref());
    out["put_wall_detail"] = wall(st.put_wall.as_ref());
    for (key, value) in [
        ("gamma_regime", json!(ex.gamma_regime)),
        ("zero_gamma", json!(ex.zero_gamma)),
        ("call_gex", json!(ex.call_gex)),
        ("put_gex", json!(ex.put_gex)),
        ("net_gex", json!(ex.net_gex)),
    ] {
        out[key] = if current && ex.gex_complete {
            value
        } else {
            Value::Null
        };
    }
    let mut dex = snake(ex);
    dex["available"] = json!(current && ex.dex_complete);
    dex["coverage_pct"] = json!(ex.delta_coverage_pct);
    if let Some(change) = s.option_snapshot_status.dex_changes.get(&h.mode) {
        dex["dex_change_available"] =
            json!(change.available && fresh(Some(change.observed_ms), now, 900_000));
        dex["dex_change_side"] = json!(change.side);
        dex["call_dex_change"] = json!(change.call);
        dex["put_dex_change"] = json!(change.put);
        dex["net_dex_change"] = json!(change.net);
        dex["dex_change_pct_gross"] = json!(change.pct_gross);
    }
    out["dex"] = dex;
    for side in ["CALL", "PUT"] {
        let key = format!("{} {side}", h.mode);
        out[format!("{}_position_map", side.to_ascii_lowercase())] =
            snake(&s.position_maps.get(&key));
        if reading.side == side {
            out["position_map_confirmation"] = snake(&s.position_maps.get(&key));
            out["wall_pull_decision"] = snake(&s.wall_pull.get(&key));
        }
        if let Some(route) = st.routes.iter().find(|r| r.direction == side) {
            out[format!("{}_pull", side.to_ascii_lowercase())] = json!({"side":side,"structural_ready":route.structural_ready,"preferred_target":route.pull_target,"extended_target":route.extended,"pull_score":route.score,"reason":route.path_state});
            out[if side == "CALL" {
                "upside_vacuum"
            } else {
                "downside_vacuum"
            }] = json!({"direction":route.direction,"low_strike":route.vacuum_low,"high_strike":route.vacuum_high,"status":route.vacuum_status});
        }
    }
    out["wall_live_flow"] = wall_flow(s.wall_flow.get(&h.mode));
    out["volume_context"] = if h.mode == "DAY" {
        json!(s.local_volume)
    } else {
        json!(snipers_core::local_volume::evaluate(
            &s.symbol,
            &h.mode,
            &Value::Null,
            now
        ))
    };
    out["decision_ready"] = json!(false); // structural context alone never authorizes a paper entry.
    if let Some(a) = adjusted {
        out["dark_pool_context_applied"] = json!(a.trf_applied);
        out["dark_pool_adjustment_points"] = json!(a.trf_score_points);
    }
    out
}
fn coverage(s: &LiveState) -> Value {
    let mut windows = std::collections::BTreeMap::<u64, (usize, usize)>::new();
    for w in &s.flow.contract_windows {
        let item = windows.entry(w.minutes).or_default();
        item.0 += 1;
        if w.covered {
            item.1 += 1;
        }
    }
    json!(windows.into_iter().map(|(minutes,(requested,covered))|json!({"minutes":minutes,"requested_contracts":requested,"covered_contracts":covered,"complete":requested>0&&covered==requested})).collect::<Vec<_>>())
}
pub fn prepare(s: &LiveState, now: u64) -> Result<Value, Error> {
    if s.stocks.phase != "streaming" || !fresh(Some(s.published_ms), now, 3000) {
        return Err(Error::Stale);
    }
    let tick = s
        .ticks
        .get(&s.symbol)
        .filter(|t| {
            t.symbol == s.symbol
                && fresh(t.trade_ms, now, 15000)
                && t.price.is_some_and(|p| p.is_finite() && p > 0.0)
        })
        .ok_or(Error::Stale)?;
    let mut market = json!({"symbol":s.symbol,"alignment":snake(&s.adjusted_alignment),"engine_build":"RUST_V1"});
    for h in s.option_snapshot_status.horizons.iter().take(3) {
        let key = match h.mode.as_str() {
            "DAY" | "NEXT_SESSION" => "day",
            "WEEK" => "week",
            "NEXT4" => "four_weeks",
            _ => continue,
        };
        market[key] = horizon(s, h, now);
    }
    let off = &s.off_exchange.analytics;
    market["dark_pool_context"] = json!({"symbol":s.symbol,"status":if off.live_fresh{"LIVE"}else{"WAITING"},"live_print_count":off.session.live_count,"print_count":off.session.print_count,"total_notional":off.session.total_notional,"pressure_pct":off.session.pressure_pct,"pressure_coverage_pct":off.session.pressure_coverage_pct,"history_integrity_complete":off.session.history_complete,"reference_date":off.session.date,"error":s.off_exchange.error});
    let day = &market["day"];
    let quick = json!({"symbol":s.symbol,"spot":tick.price,"levels":day["levels"],"call_wall":day["call_wall_detail"],"put_wall":day["put_wall_detail"],"call_pull":day["call_pull"],"put_pull":day["put_pull"],"quick_read_state":day["reading"],"quick_read_side":day["side"],"stale_data":!day.is_object()||!day["error"].is_null()});
    let mut flow = json!({"symbol":s.symbol,"spot":tick.price,"connected":s.options.phase=="streaming","authenticated":s.options.phase=="streaming","subscribed":s.options.phase=="streaming","liquidity":liquidity(&s.option_liquidity),"wall_live_flow":day["wall_live_flow"]});
    for w in &s.flow.windows {
        flow[format!("flow_{}m", w.minutes)] = snake(w);
    }
    let review = &s.decision_review;
    let reviewed = fresh(review["reviewedMs"].as_u64(), now, 3000)
        && review["symbol"].as_str() == Some(&s.symbol);
    let reason = if reviewed {
        review["reason"].clone()
    } else {
        json!("DECISION_REVIEW_STALE")
    };
    let decision = json!({"symbol":s.symbol,"decision":{"decision":review["admission"]["decision"],"primary_side":review["modelSide"],"ready_for_paper_trade":false,"entry_confirmed":reviewed&&review["admission"]["admitted"]==true,"reason":reason},"entry_guard":{"allowed":false,"reason":reason},"session_status":{"new_entries_enabled":reviewed&&review["entriesEnabled"]==true,"reason":reason},"paper_event":{"event":review["position"]["state"],"symbol":s.symbol,"reason":review["position"]["reason"]}});
    let watch:Vec<_>=s.scanner.top.iter().take(5).map(|c|{
        let confirmation=s.scanner.confirmations.get(&c.symbol);let structure=s.scanner.structure_confirmations.get(&c.symbol);
        json!({"symbol":c.symbol,"direction":c.direction,"price":c.price,"volume_2m":c.volume_2m,"rvol":c.rvol,"vwap":c.vwap,"last_bar_end":c.bar_end_ms,"entry_confirmed":false,"confirmation_state":confirmation.map(|r|&r.state),"confirmation_reason":confirmation.map(|r|&r.reason),"live_option_liquidity":s.scanner.option_liquidity.get(&c.symbol).map(liquidity),"position_map_confirmation":structure.map(|r|snake(&r.position)),"wall_pull_confirmation":structure.map(|r|snake(&r.wall))})
    }).collect();
    let scanner = json!({"top_5_phase":s.scanner.selection.phase,"top_5_frozen":s.scanner.selection.locked,"top_5_frozen_symbols":s.scanner.selection.symbols,"top_5_frozen_at":s.scanner.selection.locked_at,"top_5_watch":watch,"auto_paper_enabled":false});
    let mut gex = day.clone();
    gex["mode"] = day["horizon"].clone();
    gex["mode_label"] = day["display_label"].clone();
    let quality = json!({"evidence_timestamp_ms":tick.trade_ms,"stocks_phase":s.stocks.phase,"options_phase":s.options.phase,"snapshot_phase":s.option_snapshot_status.phase,"snapshot_observed_ms":s.option_snapshot_status.observed_ms,"snapshot_reference_source":s.selection.source,"open_interest_semantics":"DAILY_OBSERVATION_NOT_INTRADAY_FLOW","greeks_provider_timestamp":null,"oi_provider_timestamp":null,"oi_previous_observed_date":s.option_snapshot_status.previous_observed_date,"snapshot_storage_error":s.option_snapshot_status.storage_error,"flow_coverage":coverage(s),"wall_pull":snake(&s.wall_pull),"structural_paths":snake(&s.structural_paths),"scanner_confirmations":snake(&s.scanner.confirmations),"scanner_structure_confirmations":snake(&s.scanner.structure_confirmations),"decision_review":review,"off_exchange":snake(&s.off_exchange.analytics),"flow_windows_quality":snake(&s.flow.windows),"synthetic_spx":s.spx_synthetic,"cme":s.cme});
    let data = json!({"symbol":s.symbol,"market_interpretation":market,"quick_read":quick,"live_flow":flow,"gex":gex,"market_scanner":scanner,"decision_runtime":decision,"vwap_2m":{"symbol":s.symbol,"price":tick.price,"vwap":s.metrics.vwap,"source_status":s.metrics.phase},"source_status":{"symbol":s.symbol,"massive":s.stocks.phase,"live_flow":s.options.phase,"scanner":s.scanner.connected},"data_quality":quality});
    let mut output = analyst_compact::system(&data, now)?;
    output["neuro_model_context"] = snake(&s.neuro_context.checked(&s.symbol, now, s.ticks.get(&s.symbol).and_then(|t|t.price.zip(t.trade_ms)), s.stocks.phase=="streaming"));
    if s.greek_support.symbol == s.symbol && fresh(Some(s.greek_support.at), now, 3000) {
        output["intraday_greek_support"] = snake(&s.greek_support);
    }
    snipers_core::analyst::validate(&output, 0)?;
    if output.to_string().len() > snipers_core::analyst::MAX_SNAPSHOT_CHARS {
        return Err(Error::Budget);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn internal_greeks_reach_analyst_only_when_current_and_same_asset() {
        let now = snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap();
        let mut s = state(now);
        s.greek_support = snipers_core::greek_support::Context {
            symbol: "QQQ".into(),
            at: now,
            interpretation: "support".into(),
            ..Default::default()
        };
        assert_eq!(
            prepare(&s, now).unwrap()["intraday_greek_support"]["interpretation"],
            "support"
        );
        s.greek_support.at = now - 3001;
        assert!(prepare(&s, now).unwrap()["intraday_greek_support"].is_null());
        s.greek_support.at = now;
        s.greek_support.symbol = "SPY".into();
        assert!(prepare(&s, now).unwrap()["intraday_greek_support"].is_null());
    }
    #[test]
    fn horizon_adapter_preserves_call_put_routes_and_revokes_expired_scores() {
        let now = snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap();
        let mut s = state(now);
        let mut observation = snipers_core::options::Observation {
            schema: 1,
            symbol: "QQQ".into(),
            observed_ms: now,
            observed_date: "2026-09-24".into(),
            oi_reference_date: None,
            contracts: Default::default(),
        };
        for strike in [490, 500, 510] {
            for (kind, code, delta) in [("call", "C", 0.5), ("put", "P", -0.5)] {
                let ticker = format!("O:QQQ260924{code}{:08}", strike * 1000);
                let mut contract=snipers_core::options::normalize(&json!({"details":{"ticker":ticker,"expiration_date":"2026-09-24","strike_price":strike,"contract_type":kind},"underlying_asset":{"ticker":"QQQ"},"open_interest":1000,"day":{"volume":500},"greeks":{"gamma":0.03,"delta":delta},"implied_volatility":0.2}),"QQQ").unwrap();
                contract.premium_multiplier = Some(100.0);
                observation.contracts.insert(ticker, contract);
            }
        }
        s.option_snapshot_status.horizons =
            snipers_core::horizons::calculate(&observation, 500.0, "VERIFIED_FIXTURE").unwrap();
        s.option_snapshot_status.phase = "ready".into();
        let h = &mut s.option_snapshot_status.horizons[0];
        h.structure
            .routes
            .iter_mut()
            .find(|r| r.direction == "CALL")
            .unwrap()
            .pull_target = Some(520.0);
        let output = prepare(&s, now).unwrap();
        assert_eq!(
            output["market_interpretation"]["day_or_next_session"]["call_pull"]["preferred_target"],
            520.0
        );
        s.option_snapshot_status.horizons[0].observed_ms = now - 900001;
        let output = prepare(&s, now).unwrap();
        let day = &output["market_interpretation"]["day_or_next_session"];
        assert_eq!(day["error"], "SNAPSHOT_STALE_OR_INCOMPLETE");
        assert!(day["call_score"].is_null());
        assert_eq!(day["decision_ready"], false);
    }
    fn state(now: u64) -> LiveState {
        let mut s = crate::live::new_state().lock().unwrap().clone();
        s.published_ms = now;
        s.stocks.phase = "streaming".into();
        s.ticks.insert(
            "QQQ".into(),
            crate::live::Tick {
                symbol: "QQQ".into(),
                price: Some(500.0),
                trade_ms: Some(now),
                bid: None,
                ask: None,
                quote_ms: None,
                trade_sequence: Some(1),
            },
        );
        s
    }
    #[test]
    fn context_is_local_bounded_and_preserves_missing_data_and_session_blocks() {
        let now = snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap();
        let mut s = state(now);
        s.decision_review = json!({"symbol":"QQQ","reviewedMs":now,"reason":"DAILY_LIMIT","entriesEnabled":true,"admission":{"admitted":false,"decision":"WAIT"}});
        let out = prepare(&s, now).unwrap();
        assert_eq!(out["symbol"], "QQQ");
        assert_eq!(out["quick_read"]["stale_data"], true);
        assert_eq!(out["decision_runtime"]["entry_guard"]["allowed"], false);
        assert_eq!(out["decision_runtime"]["decision"]["reason"], "DAILY_LIMIT");
        assert!(out["data_quality"]["greeks_provider_timestamp"].is_null());
        assert_eq!(out["data_quality"]["cme"]["phase"], "pending_license");
        s.published_ms = now - 3001;
        assert_eq!(prepare(&s, now), Err(Error::Stale));
        s.published_ms = now;
        s.ticks.get_mut("QQQ").unwrap().symbol = "SPY".into();
        assert_eq!(prepare(&s, now), Err(Error::Stale));
    }
    #[test]
    fn all_contracts_contribute_to_coverage_and_stale_review_cannot_certify_admission() {
        let now = snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap();
        let mut s = state(now);
        s.decision_review = json!({"symbol":"QQQ","reviewedMs":now-3001,"entriesEnabled":true,"admission":{"admitted":true,"decision":"CALL"}});
        for i in 0..1001 {
            s.flow
                .contract_windows
                .push(snipers_core::flow::ContractWindow {
                    ticker: format!("C{i}"),
                    kind: "call".into(),
                    strike: 500.0,
                    expiration: "2026-09-25".into(),
                    minutes: 1,
                    observed_since_ms: Some(now - 60000),
                    covered: i != 1000,
                    contracts: None,
                    premium: None,
                    trades: None,
                    reason: "FIXTURE".into(),
                });
        }
        let out = prepare(&s, now).unwrap();
        assert_eq!(
            out["data_quality"]["flow_coverage"][0]["requested_contracts"],
            1001
        );
        assert_eq!(out["data_quality"]["flow_coverage"][0]["complete"], false);
        assert_eq!(
            out["decision_runtime"]["decision"]["entry_confirmed"],
            false
        );
        assert_eq!(
            out["decision_runtime"]["session_status"]["new_entries_enabled"],
            false
        );
    }
}
