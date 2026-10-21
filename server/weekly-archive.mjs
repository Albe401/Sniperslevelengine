/** Assemble a historical input; calculations and quote validation stay in the reader. */
export function weeklySession(frame,at){
 const sessions=frame?.chartHistory?.sessions??[];
 return sessions.filter(s=>Number.isFinite(s.openMs)&&Number.isFinite(s.closeMs)&&s.openMs<s.closeMs&&s.closeMs<=at&&at-s.closeMs<=7*86400000).sort((a,b)=>b.closeMs-a.closeMs)[0];
}
function weeklyInput(report,session){
 // The weekly reader needs quote/IV evidence, not another copy of a six-month
 // heatmap and its exposures. Bound transmission to the reference's seven days.
 const end=new Date(session.closeMs+7*86400000).toISOString().slice(0,10);
 const details=(report.enrichment?.details??[]).filter(c=>c.expiration>=session.date&&c.expiration<=end);
 return {symbol:report.symbol,days:7,at:report.at,spot:report.spot,priceAt:report.priceAt,expirationCloses:report.expirationCloses,enrichment:{details}};
}
export function assembleWeeklyArchive(frame,archived,capturedMs){
 const session=weeklySession(frame,capturedMs);
 if(!session)return undefined;
 const bar=frame.chartHistory?.minutes?.find(b=>b.timestampMs+60000===session.closeMs);
 const observation=archived?.contractChain?.observation;
 const report=observation?.value;
 if(!bar||!Number.isFinite(bar.close)||bar.close<=0||report?.symbol!==frame.symbol||!Number.isFinite(report.at)||report.at>session.closeMs+120000||report.at<session.closeMs-15*60000||observation.observedMs>capturedMs)return undefined;
 return {...weeklyInput(report,session),spot:bar.close,priceAt:session.closeMs,at:capturedMs,historicalSession:session,originalSnapshotAt:report.at,originalPriceAt:report.priceAt,assembledFrom:'saved-rth-close-and-archived-options'};
}
/** Current inventory can retain real last-session quotes before the open.
 * Pair it with the recorded regular-session close, as the desktop reader does.
 * Quote times, IV, ATM pair and spreads still pass the unchanged reader checks.
 */
export function closedSessionReference(frame,capturedMs){
 if((frame.chartHistory?.sessions??[]).some(s=>s.openMs<=capturedMs&&capturedMs<s.closeMs))return undefined;
 const session=weeklySession(frame,capturedMs),observation=frame.contractChain?.observation,report=observation?.value;
 const bar=frame.chartHistory?.minutes?.find(b=>session&&b.timestampMs+60000===session.closeMs);
 if(!session||!bar||!Number.isFinite(bar.close)||bar.close<=0||report?.symbol!==frame.symbol||!Number.isFinite(report.at)||report.at>capturedMs||!Number.isFinite(observation.observedMs)||observation.observedMs>capturedMs)return undefined;
 return {...weeklyInput(report,session),spot:bar.close,priceAt:session.closeMs,historicalSession:session,originalSnapshotAt:report.at,originalPriceAt:report.priceAt,assembledFrom:'saved-rth-close-and-current-inventory'};
}
export async function loadWeeklyArchives(sql,frame,capturedMs,decode,hasBatchHistory=true){
 const session=weeklySession(frame,capturedMs);
 if(!session)return [];
 // During regular hours the existing live-compatible inventory remains the input.
 if((frame.chartHistory?.sessions??[]).some(s=>s.openMs<=capturedMs&&capturedMs<s.closeMs))return [];
 const candidates=[],current=closedSessionReference(frame,capturedMs);
 if(current)candidates.push(current);
 if(!hasBatchHistory)return candidates;
 // A missing chain at the last close must not hide a still-applicable weekly
 // reference. Retain exact closing dates; the reader validates every candidate.
 const sessions=(frame.chartHistory?.sessions??[]).filter(s=>s.openMs<s.closeMs&&s.closeMs<=capturedMs&&capturedMs-s.closeMs<=7*86400000).sort((a,b)=>b.closeMs-a.closeMs).slice(0,5);
 for(const closed of sessions){
  const rows=await sql.query(`SELECT CASE WHEN a.payload_compressed IS NULL THEN a.payload ELSE jsonb_build_object('compressedBase64',encode(a.payload_compressed,'base64'),'sha256',a.payload_sha256) END AS frame FROM snipers_snapshots.batches b JOIN snipers_snapshots.assets a ON a.batch_id=b.id WHERE a.symbol=$1 AND b.captured_ms BETWEEN $2 AND $3 ORDER BY b.captured_ms DESC LIMIT 1`,[frame.symbol,closed.closeMs-15*60000,Math.min(capturedMs,closed.closeMs+120000)]);
  const scoped={...frame,chartHistory:{...frame.chartHistory,sessions:[closed]}};
  const candidate=rows[0]?assembleWeeklyArchive(scoped,decode(rows[0].frame),capturedMs):undefined;
  if(candidate)candidates.push(candidate);
 }
 return candidates;
}
export async function loadWeeklyArchive(...args){return (await loadWeeklyArchives(...args))[0];}
