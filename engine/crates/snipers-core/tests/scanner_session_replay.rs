use snipers_core::{
    calendar,
    scanner_memory::{Evidence, Memory},
    scanner_trigger,
};

fn evaluate(
    memory: &mut Memory,
    symbol: &str,
    bar: u64,
    now: u64,
    verified: bool,
    gates: bool,
    ready: bool,
) -> snipers_core::scanner_memory::Read {
    let trigger = scanner_trigger::Read {
        ready,
        reference_price: Some(102.),
        invalidation: Some(99.),
        ..Default::default()
    };
    memory.evaluate(
        &Evidence {
            symbol,
            bar_ms: bar,
            close: 102.,
            vwap: 100.,
            bullish: true,
            verified,
            all_gates: gates,
            targets: &[105., 110.],
            trigger: &trigger,
        },
        now,
    )
}

#[test]
fn regular_and_early_close_session_replay_requires_new_trigger_after_invalidation() {
    for date in ["2026-09-24", "2026-11-27"] {
        let open = calendar::legacy_eastern_ms(&format!("{date} 09:30:00")).unwrap();
        let calendar::Day::Open(day) = calendar::schedule(calendar::date_at(open).unwrap()) else {
            panic!("open session")
        };
        let members: Vec<String> = ["QQQ", "SPY", "NVDA", "TSLA", "MU"]
            .map(str::to_owned)
            .into();
        let mut memory = Memory::default();
        memory.reconcile(&members, open);
        let first = open + 30 * 60000;
        for symbol in &members {
            assert!(
                evaluate(&mut memory, symbol, first, first + 2000, true, true, true).signal_active
            );
        }
        assert!(
            !evaluate(&mut memory, "QQQ", first, first + 3000, false, true, true).signal_active
        );
        // Replaying the candle that was invalidated must not revive a signal.
        assert!(!evaluate(&mut memory, "QQQ", first, first + 4000, true, true, true).signal_active);
        assert!(evaluate(&mut memory, "SPY", first, first + 4000, true, true, false).signal_active);
        assert!(
            !evaluate(
                &mut memory,
                "QQQ",
                first + 120000,
                first + 122000,
                true,
                true,
                false
            )
            .signal_active
        );
        assert!(
            evaluate(
                &mut memory,
                "QQQ",
                first + 240000,
                first + 242000,
                true,
                true,
                true
            )
            .signal_active
        );
        assert!(
            !evaluate(
                &mut memory,
                "QQQ",
                first + 240000,
                first + 243000,
                true,
                false,
                true
            )
            .signal_active
        );
        assert!(
            !evaluate(
                &mut memory,
                "QQQ",
                first + 240000,
                first + 244000,
                true,
                true,
                true
            )
            .signal_active
        );
        let mut bar = first + 360000;
        while bar < day.close_ms {
            for symbol in &members {
                let read = evaluate(&mut memory, symbol, bar, bar + 2000, true, true, true);
                assert!(!read.entry_confirmed, "scanner must not admit trades");
                // Other members skipped the first two bars and must rebuild after
                // the continuity gap; later regular bars retain their own state.
                if bar > first + 360000 {
                    assert!(read.signal_active, "{symbol} at {bar}");
                }
            }
            bar += 120000;
        }
        for symbol in &members {
            assert!(
                !evaluate(
                    &mut memory,
                    symbol,
                    day.close_ms,
                    day.close_ms,
                    true,
                    true,
                    true
                )
                .signal_active
            );
        }
        let mut restarted = Memory::default();
        restarted.reconcile(&members, day.close_ms);
        assert!(
            !evaluate(
                &mut restarted,
                "QQQ",
                day.close_ms,
                day.close_ms,
                true,
                true,
                true
            )
            .signal_active
        );
    }
}
