/**
 * Project heuristic shared by route observations and opportunity admission.
 * The ten-minute window starts at the CLOSED impulse's end, never at a refresh.
 * A one-minute close at the deadline can finish an existing observation; a new
 * opportunity requires time strictly before that deadline. These are different
 * clocks and boundary operators, not evidence of a fill or a probability.
 */
export const ROUTE_BAR_MS=60000;
export const ROUTE_SEQUENCE_WINDOW_MS=600000;

/** Input is the impulse CLOSE time, not its candle start. */
export const routeSequenceDeadlineMs=(impulseCloseMs:number):number=>impulseCloseMs+ROUTE_SEQUENCE_WINDOW_MS;

/** Compatibility with frozen-route-opportunity-v1's bar-start representation. */
export const routeSequenceLastAllowedBarStartMs=(impulseStartMs:number):number=>
 routeSequenceDeadlineMs(impulseStartMs+ROUTE_BAR_MS)-ROUTE_BAR_MS;

/** Existing price observations admit the last close exactly at the deadline. */
export const routeSequenceBarExpired=(barCloseMs:number,deadlineMs:number):boolean=>barCloseMs>deadlineMs;

/** A fresh opportunity cannot be admitted at or after its original deadline. */
export const routeSequenceAdmissionExpired=(availableMs:number,deadlineMs:number):boolean=>availableMs>=deadlineMs;
