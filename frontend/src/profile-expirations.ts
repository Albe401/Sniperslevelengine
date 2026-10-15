import type {EtfReport} from './etf-dealers';
export type ProfileHorizon='DAY'|'WEEK'|'NEXT4';
const date=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
/** Weekend snapshots contain the next session's chain, explicitly labelled by the UI. */
export function historicalProfileReference(report:EtfReport):number{
 const today=date.format(report.at),day=new Date(today+'T12:00:00Z').getUTCDay();
 if(day!==0&&day!==6)return report.at;
 const first=profileExpirations(report,'NEXT4',report.at)[0];
 return first?Date.parse(first+'T12:00:00Z'):report.at;
}
/** Expiry dates, not four calendar weeks; no fallback from missing 0DTE. */
export function profileExpirations(report:EtfReport|null|undefined,mode:ProfileHorizon,now:number):string[]{
 const today=date.format(now),available=[...new Set(report?.enrichment?.details.map(c=>c.expiration).filter(d=>/^\d{4}-\d{2}-\d{2}$/.test(d)&&d>=today)??[])].sort();
 if(mode==='DAY')return available.filter(d=>d===today);
 if(mode==='NEXT4')return available.slice(0,4);
 const end=new Date(today+'T12:00:00Z');end.setUTCDate(end.getUTCDate()+(7-end.getUTCDay())%7);
 return available.filter(d=>d<=end.toISOString().slice(0,10));
}
