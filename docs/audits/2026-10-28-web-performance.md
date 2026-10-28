# Snapshot performance — 2026-10-28

The saved viewer was downloading unchanged multi-megabyte frames and repeatedly updating archived chart overlays while the chart was stationary or outside the viewport. QQQ's observed response was 15,837,515 decoded bytes (about 2.05 MB gzip). Public requests returned HTTP 200, without console errors in the initial sampled browser session.

The viewer now revalidates captures with their exact response ETag, retains at most two assets in memory, joins identical pending reads, cancels superseded asset reads, and avoids rebuilding an unchanged capture. Captured chart overlays skip offscreen work and update stationary expiry/status once per second; geometry changes retain the existing interaction cadence. Inventory filters check their existing identity/date/horizon cache before enumerating every expiration. All fields, strikes, timestamps, mathematical readers and refresh schedule are retained.

Validation:

- 152 frontend and 19 server tests passed. Four existing historical-fixture tests were skipped by their own fixture guards; these are not passes. Nine loader tests were rerun after correcting a browser-only unbound-fetch receiver error caught during local verification. TypeScript and the production build passed. The existing large-chunk build warning remains.
- Actual server handler with the public QQQ input: the complete JSON body exactly equals the original, including 7,188 option contracts and 23 expirations. Conditional repetition returns HTTP 304 with zero body bytes; changed data and a different asset do not reuse that response.
- Sequential local browser measurement with the same original QQQ frame: 1,539 DOM mutation records in 8.003 seconds before, six in 8.012 seconds after. Both sampled idle windows had no long tasks. The chart was initially below the viewport; these numbers are not a claim of a 99% CPU or total-page latency improvement.
- The 165 displayed DAY GEX rows, eight level cards and structural reading match exactly. Five-minute chart selection and AAPL → QQQ return worked. The six-month filter rendered 519 strikes from 204.78 to 1,115, including 695 and 725. Final local console error list was empty.
- Repeated inventory reads on the same real 7,188-contract input: 300 calls, p50 0.755 → 0.103 ms; both return 7,188 contracts and 23 expirations. Finite local microbenchmark only.

Original inputs, hashes, commands, logs, failed attempts and detailed receipts are retained in the local project audit. An initial missing-fixture-path test failure was resolved with the test's existing fixture-root option; server dependency resolution was restored using the already installed locked Neon 1.2.0 dependency. No package versions were changed. Local browser setup failures and the pre-publication fetch binding defect are retained as incidents.

HTTP conditional validation follows [RFC 9110, If-None-Match](https://www.rfc-editor.org/rfc/rfc9110.html#name-if-none-match). Visibility uses [Document.hidden](https://developer.mozilla.org/en-US/docs/Web/API/Document/hidden). The finite acceptance checks do not certify market reception, trading outcomes, all browsers, or continuous production throughput. Native Snipers is not rebuilt, closed or restarted for this web-only change. Publication status is recorded separately after deployment verification.
