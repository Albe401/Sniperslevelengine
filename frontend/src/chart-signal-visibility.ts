/** Stage1/2 observations continue in the background; legacy CALL/PUT overlays stay off. */
export const CHART_SIGNAL_VISIBILITY_POLICY=Object.freeze({
 version:'chart-signal-visibility-v1',legacyCallPutOverlays:false,engineObservationEnabled:true,
 executionEnabled:false,reason:'research-validation-pending'
} as const);
