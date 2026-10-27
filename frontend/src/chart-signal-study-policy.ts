/** Preregistered research protocol. Date windows and gates are project conventions, not profit guarantees. */
export const CHART_SIGNAL_STUDY_POLICY=Object.freeze({
 version:'chart-signal-study-v1',registryId:'chart-signal-study-20261027-v1',registeredAtMs:Date.parse('2026-10-27T17:04:33.269Z'),registeredDateET:'2026-10-27',
 trainFrom:'2026-10-28',validationFrom:'2026-11-04',holdoutFrom:'2026-11-11',holdoutUntil:'2026-11-18',minimumCompletedDatesPerPartition:5,
 gates:['baseline','structure-15m-aligned','structure-60m-aligned','technical-15m-aligned','options-premium-5m-aligned','gex-context-covered','patterns-15m-covered','absorption-direction-adapter'] as const,
 technicalAdxMinimum:20,technicalRvolMinimum:1,technicalRequiredReferences:20,optionClassifiedMinimumPct:70,
 costScenariosRoundTripBps:[0,1,5] as const,wilsonZ:1.959963984540054,
 maxRecentEnrollments:4,maxTrialBytes:16384,maxStudyRows:4096,maxInputBytes:16777216,maxEpisodes:256,maxReportBytes:262144,
 primitiveDedup:'symbol/date/side/impulse/trigger/reference/target/invalidation;earliest-known-first',overlapRule:'same-symbol-frozen-known-through-deadline;purge-later-including-touching-endpoint',
 optionPnl:'unavailable-without-contract-quotes-executions-and-costs',promotionEnabled:false,markersEnabled:false,executionEnabled:false
} as const);
export type StudyGate=typeof CHART_SIGNAL_STUDY_POLICY.gates[number];
export const CHART_SIGNAL_STUDY_REGISTRY_SIGNATURE=JSON.stringify(CHART_SIGNAL_STUDY_POLICY);
