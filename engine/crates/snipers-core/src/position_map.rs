//! Price geometry only. A valid channel never constitutes permission to trade.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub side: String,
    pub price: Option<f64>,
    pub put_wall: Option<f64>,
    pub call_wall: Option<f64>,
    pub pull_target: Option<f64>,
    pub main_magnet: Option<f64>,
    pub liquidity_magnet: Option<f64>,
    pub vacuum_low: Option<f64>,
    pub vacuum_high: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Map {
    pub ready: bool,
    pub status: String,
    pub side: String,
    pub reason: String,
    pub price: Option<f64>,
    pub put_wall: Option<f64>,
    pub call_wall: Option<f64>,
    pub pull_target: Option<f64>,
    pub walls_ordered: bool,
    pub inside_walls: bool,
    pub support_position_ok: bool,
    pub target_wall_position_ok: bool,
    pub pull_position_ok: bool,
    pub main_options_magnet: Option<f64>,
    pub main_magnet_relation: String,
    pub liquidity_magnet: Option<f64>,
    pub vacuum_low: Option<f64>,
    pub vacuum_high: Option<f64>,
    pub vacuum_relation: String,
    pub entry_confirmed: bool,
}
fn positive(v: Option<f64>) -> Option<f64> {
    v.filter(|n| n.is_finite() && *n > 0.0)
}
pub fn calculate(i: &Input) -> Map {
    let side = i.side.trim().to_ascii_uppercase();
    let mut r = Map {
        status: "POSITION MAP WAIT".into(),
        side: if side.is_empty() {
            "NEUTRAL".into()
        } else {
            side.clone()
        },
        price: positive(i.price),
        put_wall: positive(i.put_wall),
        call_wall: positive(i.call_wall),
        pull_target: positive(i.pull_target),
        main_options_magnet: positive(i.main_magnet),
        liquidity_magnet: positive(i.liquidity_magnet),
        main_magnet_relation: "SIN DATOS".into(),
        vacuum_relation: "SIN DATOS".into(),
        ..Default::default()
    };
    if !matches!(side.as_str(), "CALL" | "PUT") {
        r.reason = "DIRECCION CALL/PUT NO VERIFICADA".into();
        return r;
    }
    let call = side == "CALL";
    let missing: Vec<_> = [
        (r.price, "PRECIO"),
        (r.put_wall, "PUT WALL"),
        (r.call_wall, "CALL WALL"),
        (r.pull_target, if call { "CALL PULL" } else { "PUT PULL" }),
    ]
    .into_iter()
    .filter_map(|(v, s)| v.is_none().then_some(s))
    .collect();
    if !missing.is_empty() {
        r.reason = format!("FALTAN: {}", missing.join(", "));
        return r;
    }
    let (p, put, wall, target) = (
        r.price.unwrap(),
        r.put_wall.unwrap(),
        r.call_wall.unwrap(),
        r.pull_target.unwrap(),
    );
    r.walls_ordered = put <= wall;
    r.inside_walls = put <= p && p <= wall;
    r.support_position_ok = if call { p >= put } else { p <= wall };
    r.target_wall_position_ok = if call { p <= wall } else { p >= put };
    r.pull_position_ok = if call { target > p } else { target < p };
    if let Some(m) = r.main_options_magnet {
        r.main_magnet_relation = if (m - p).abs() <= (p.abs() * 0.0005).max(0.01) {
            "EN PRECIO"
        } else if (call && m > p) || (!call && m < p) {
            "DELANTE"
        } else {
            "DETRAS"
        }
        .into();
    }
    if let Some((a, b)) = positive(i.vacuum_low).zip(positive(i.vacuum_high)) {
        let (low, high) = (a.min(b), a.max(b));
        r.vacuum_low = Some(low);
        r.vacuum_high = Some(high);
        let name = if call {
            "VACIO SUPERIOR"
        } else {
            "VACIO INFERIOR"
        };
        r.vacuum_relation = if low <= p && p <= high {
            format!("PRECIO DENTRO DEL {name}")
        } else if (call && low > p) || (!call && high < p) {
            format!("{name} DELANTE")
        } else {
            format!("{name} DETRAS")
        };
    }
    r.ready = r.walls_ordered
        && r.inside_walls
        && r.support_position_ok
        && r.target_wall_position_ok
        && r.pull_position_ok;
    r.reason = if !r.walls_ordered {
        "CALL/PUT WALL INCOMPLETOS O INVERTIDOS".into()
    } else if !r.inside_walls {
        if call && p < put {
            "CALL: PRECIO DEBAJO DEL PUT WALL"
        } else if !call && p > wall {
            "PUT: PRECIO ENCIMA DEL CALL WALL"
        } else {
            "PRECIO FUERA DEL CANAL PUT WALL / CALL WALL"
        }
        .into()
    } else if !r.pull_position_ok {
        if call {
            "CALL: CALL PULL NO ESTA DELANTE DEL PRECIO"
        } else {
            "PUT: PUT PULL NO ESTA DELANTE DEL PRECIO"
        }
        .into()
    } else {
        format!("POSICION ESTRUCTURAL CORRECTA · PUT WALL {put:.2} · PRECIO {p:.2} · CALL WALL {wall:.2} · {side} PULL {target:.2}")
    };
    if r.ready {
        r.status = "POSITION MAP READY".into();
    }
    r
}
/// Reception time must never refresh a stale structural snapshot or a stale price.
pub fn guarded(i: &Input, complete: bool, observed: u64, price_ms: Option<u64>, now: u64) -> Map {
    let mut r = calculate(i);
    let fresh = |t: u64, age: u64| {
        t <= now
            && now - t <= age
            && crate::calendar::date_at(t).is_some()
            && crate::calendar::date_at(t) == crate::calendar::date_at(now)
    };
    if !complete || !fresh(observed, 900_000) || !price_ms.is_some_and(|t| fresh(t, 15_000)) {
        r.ready = false;
        r.status = "POSITION MAP WAIT".into();
        r.reason = "Cobertura incompleta o referencia sin vigencia".into();
    }
    r
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_geometry_and_relations_match() {
        let f: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-position-map.json"))
                .unwrap();
        for c in f["cases"].as_array().unwrap() {
            let i: Input = serde_json::from_value(c["input"].clone()).unwrap();
            let r = calculate(&i);
            let e = &c["expected"];
            assert_eq!(r.ready, e["ready"]);
            assert_eq!(r.status, e["status"]);
            assert_eq!(r.reason, e["reason"]);
            assert!(!r.entry_confirmed);
            for (a, k) in [
                (r.walls_ordered, "walls_ordered"),
                (r.inside_walls, "inside_walls"),
                (r.pull_position_ok, "pull_position_ok"),
            ] {
                if let Some(b) = e[k].as_bool() {
                    assert_eq!(a, b);
                }
            }
            for (a, k) in [
                (&r.main_magnet_relation, "main_magnet_relation"),
                (&r.vacuum_relation, "vacuum_relation"),
            ] {
                if let Some(b) = e[k].as_str() {
                    assert_eq!(a, b);
                }
            }
        }
    }
    #[test]
    fn invalid_and_stale_data_never_authorize_geometry() {
        let now = crate::calendar::day_bounds("2026-09-24").unwrap().0 + 36_000_000;
        let i = Input {
            side: "CALL".into(),
            price: Some(100.0),
            put_wall: Some(90.0),
            call_wall: Some(110.0),
            pull_target: Some(105.0),
            ..Default::default()
        };
        assert!(guarded(&i, true, now, Some(now), now).ready);
        for (complete, obs, price) in [
            (false, now, Some(now)),
            (true, now + 1, Some(now)),
            (true, now - 900_001, Some(now)),
            (true, now, Some(now - 15_001)),
            (true, now, Some(now + 1)),
            (true, now, None),
        ] {
            assert!(!guarded(&i, complete, obs, price, now).ready);
        }
        for price in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert!(
                !calculate(&Input {
                    price: Some(price),
                    ..i.clone()
                })
                .ready
            );
        }
        assert!(
            !calculate(&Input {
                call_wall: Some(f64::NAN),
                ..i
            })
            .ready
        );
    }
}
