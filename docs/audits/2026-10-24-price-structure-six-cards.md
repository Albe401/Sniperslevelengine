# Frozen five-timeframe price structure

The structural map now has 15-minute, hourly, four-hour, daily and weekly cards, followed by a joint reading. The three-column layout retains its original width, card dimensions and lower edge; the new row grows upward. Existing options exposure remains available in the detail panel, under its original expiration horizons.

The numerical kernel is shared with the installed native implementation from 24 October 2026: strict pivots with two closed candles on either side, alternating extrema, a 100-candle pivot window, Wilder ATR14 and at least 100 consecutive closed candles per timeframe. Removing the first 14 candles must change ATR by no more than 0.5%. The minimum history, sensitivity check and pivot parameters are project conventions, not calibrated predictors. CALL requires rising confirmed highs and lows; PUT requires both falling; conflicting extrema or a subsequent close through structural support produce MIXTA. Missing or invalid evidence produces SIN DATOS.

U=(H2-H1)/ATR; V=(L2-L1)/ATR. ATR scales the magnitude; the zero sign threshold determines the descriptive direction. All five CALL or all five PUT give full alignment. A majority does not create a signal. Timeframes overlap and are not independent votes or probabilities. The upper daily ATR engine is unchanged.

Intraday aggregation uses every minute of the official regular session, anchored at its opening. The final block ends at the official close, including shortened hourly and four-hour blocks. Daily bars use the provider's split-adjusted aggregates; weekly bars require every scheduled daily close of a completed exchange week. Missing minutes/days reset the continuous sequence; no gap filling or future pivot confirmation is permitted.

The desktop publisher computes a small `priceStructureReading` with the same TypeScript kernel and stores it inside the immutable captured frame. Its history comes from a local, per-asset REST cache containing 60 intraday sessions and daily aggregates from January 2024. At most two cache workers run concurrently. Refreshes occur on ET date rollover, the exchange close and after closing consolidation. Only history acquired before the capture is used. The frontend verifies version, asset, frozen publication/capture times, numerical evidence, pivots, normalized changes and classification. The summary is recomputed from the five stored cards. The browser makes no market-data requests and receives no provider credentials. Older captures without long history explicitly show missing scales.

Validation: shared math and long-history tests, frozen identity/clock/number rejection tests, Python publisher tests, API preservation checks, production TypeScript/Vite build, 60 acceptance checks on actual QQQ/SPY/NVDA histories, and headless Edge interaction/geometry comparisons at widths 1920 and 1482. The native kernel separately passed 2,505 independent real-data checks. Actual frozen QQQ input gives PUT/PUT/PUT/PUT/CALL; its summary is divergence, not a majority PUT signal.

A broad run exposed seven existing failing suites in the unchanged parent commit too: missing external fixtures/audit inputs and native watchlist-study assumptions in this web source copy. These failures are preserved in the local audit rather than silently reported as passing. The changed structural/API/publisher tests and production build pass independently. Vite reports the existing large-bundle advisory.

Historical validation is descriptive. Live-session acceptance and predictive performance are not established by these checks. The original company database and two-year SPY/QQQ minute archives are preserved; this operational cache is not presented as a ten-year backtest.

Sources defining inputs and ATR, rather than certifying the project's directional rule:

- [Fidelity ATR guide](https://www.fidelity.com/learning-center/trading-investing/technical-analysis/technical-indicator-guide/atr)
- [Massive custom bars](https://massive.com/docs/rest/stocks/aggregates/custom-bars)
- [Massive aggregate construction](https://massive.com/knowledge-base/article/how-does-massive-create-aggregate-bars)
- [NYSE sessions and holidays](https://www.nyse.com/markets/hours-calendars)
- [TradingView candle intervals](https://www.tradingview.com/support/solutions/43000747934-time-intervals-how-much-market-activity-each-candle-shows/)

Installation: build `desktop-writer/price-structure-capture.mjs` with `node desktop-writer/build-price-structure.mjs`. Install that kernel plus `snapshot_price_structure.py` into the native writer's scripts directory, compile the included Rust helper as `massive-feed/src/bin/snipers-structure-history.rs`, and provide local `data/price-structure-snapshot/runtime.json` with `node`, `kernel`, `historyExe` absolute paths. Primary staging and supplementary watchlist delivery both call `enrich_frames` before archiving. This directory is excluded from the public Vercel function bundle. No desktop application restart is needed for publisher updates.
