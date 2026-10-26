# Absorption display repair · 26 October 2026

The saved web chart evaluated absorption against the viewer's current clock,
so a valid observation disappeared three seconds after capture. The chart now
uses its explicit capture clock and labels any resulting mark `OBS. EN CAPTURA`.
Live freshness remains three seconds. Future captures, incomplete observations,
wrong symbols and invalid candidates remain rejected. The renderer also keeps
the final ten pixels before the reserved price axis eligible for a mark.

Pending diagnostics now distinguish an observed thirty-second window from
missing recent trades or insufficient classified volume with visible bid/ask
sizes. The displayed thirty-second classification percentage is explicitly a
different measurement from the detector's ten-second coverage test. Detector
formulas, thresholds, directions and input data are unchanged.

Validation: 64 tests passed, one existing positive-real-replay case skipped;
TypeScript check and production build passed. The usual Vite chunk-size warning
remains. New positive renderer cases are synthetic, not market observations.

`execution-absorption-observed-pending.fixture.json` is a projection of the real
QQQ local receiver snapshot at 2026-10-26 11:39:08 ET, source SHA-256
`6008e3e79a0e4cb43dbc30c9ee5acd353ca2fd3381ae77ef4e2ddf336a491274`.
The original snapshot had absorption pending, zero candidates and thirty-second
classification 36.04311077971365%. Replay preserves that negative result and does
not mutate the input. A successful synthetic drawing is not positive real-market
validation of the detector.

The web publishes saved observations every five minutes on weekdays; this
repair does not supply continuous trades or quotes to other watchlist symbols.
In four inspected QQQ receiver samples, the detector itself reported insufficient
classification with visible-size coverage. Graphics cannot replace that missing
input. A receiver status issue associated with future-admitted event times remains
an engine-stage investigation; this change does not bypass its pending status.
