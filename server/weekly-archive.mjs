/** Assemble a historical input; calculations and quote validation stay in the reader. */
export function weeklySession(frame,at){
 const sessions=frame?.chartHistory?.sessions??[];
 return sessions.filter(s=>Number.isFinite(s.openMs)&&Number.isFinite(s.closeMs)&&s.openMs<s.closeMs&&s.closeMs<=at&&at-s.closeMs<=7*86400000).sort((a,b)=>b.closeMs-a.closeMs)[0];
}
export function assembleWeeklyArchive(frame,archived,capturedMs){
 const session=weeklySession(frame,capturedMs);
 if(!session)return undefined;
 const bar=frame.chartHistory?.minutes?.find(b=>b.timestampMs+60000===session.closeMs);
 const observation=archived?.contractChain?.observation;
 const report=observation?.value;
 if(!bar||!Number.isFinite(bar.close)||bar.close<=0||report?.symbol!==frame.symbol||!Number.isFinite(report.at)||report.at>session.closeMs+120000||report.at<session.closeMs-15*60000||observation.observedMs>capturedMs)return undefined;
 return {...report,days:7,spot:bar.close,priceAt:session.closeMs,at:capturedMs,historicalSession:session,originalSnapshotAt:report.at,originalPriceAt:report.priceAt,assembledFrom:'saved-rth-close-and-archived-options'};
}
export async function loadWeeklyArchive(sql,frame,capturedMs,decode){
 const session=weeklySession(frame,capturedMs);
 if(!session)return undefined;
 // During regular hours the existing live-compatible inventory remains the input.
 if((frame.chartHistory?.sessions??[]).some(s=>s.openMs<=capturedMs&&capturedMs<s.closeMs))return undefined;
 const rows=await sql.query(`SELECT CASE WHEN a.payload_compressed IS NULL THEN a.payload ELSE jsonb_build_object('compressedBase64',encode(a.payload_compressed,'base64'),'sha256',a.payload_sha256) END AS frame FROM snipers_snapshots.batches b JOIN snipers_snapshots.assets a ON a.batch_id=b.id WHERE a.symbol=$1 AND b.captured_ms BETWEEN $2 AND $3 ORDER BY b.captured_ms DESC LIMIT 1`,[frame.symbol,session.closeMs-15*60000,Math.min(capturedMs,session.closeMs+120000)]);
 return rows[0]?assembleWeeklyArchive(frame,decode(rows[0].frame),capturedMs):undefined;
}
