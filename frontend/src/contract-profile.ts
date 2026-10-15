import type {EtfReport,EtfSide} from './etf-dealers';
import type {LiveState,Horizon} from './live';
const sum=(v:(number|null)[])=>v.length&&v.every(n=>n!=null&&Number.isFinite(n)&&n>=0)?v.reduce<number>((a,b)=>a+b!,0):null;
const dateFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
/** Same contracts, dates and unknown handling for ETF scanner and chart profile. */
export function scopeContractReport(r:EtfReport,dates:string[]):EtfReport {
 const details=r.enrichment?.details.filter(c=>dates.includes(c.expiration))??[];
 const grouped=new Map<number,typeof details>();for(const c of details){const row=grouped.get(c.strike)??[];row.push(c);grouped.set(c.strike,row);}
 const rows=[...grouped].sort(([a],[b])=>a-b).map(([strike,contracts])=>{
  const side=(kind:string):EtfSide=>{const ds=contracts.filter(c=>c.kind===kind);return {oi:ds.length?sum(ds.map(c=>c.oi)):0,volume:ds.length?sum(ds.map(c=>c.volume)):0,delta:ds.length?sum(ds.map(c=>c.dex)):0,gamma:ds.length?sum(ds.map(c=>c.gex)):0};};return {strike,call:side('call'),put:side('put')};
 });
 return {...r,rows,expirations:dates,contracts:details.length,gammaComplete:details.length>0&&details.every(c=>c.gex!=null),deltaComplete:details.length>0&&details.every(c=>c.dex!=null),enrichment:r.enrichment?{...r.enrichment,details,oiKnown:details.filter(c=>c.oi!=null).length,gammaKnown:details.filter(c=>c.gamma!=null).length,deltaKnown:details.filter(c=>c.delta!=null).length,quotesFresh:details.filter(c=>c.fresh).length}:undefined};
}
/** A next-session profile is a next-session profile, never disguised as 0DTE. */
export function chartContractReport(s:LiveState|undefined,symbol:string,active:boolean,h:Horizon|undefined,now:number):EtfReport|undefined {
 const snap=s?.optionSnapshotStatus,r=snap?.structuralEnrichment,dates=h?.expirations;
 if(!active||s?.symbol!==symbol||snap?.phase!=='ready'||!r?.enrichment||!h||r.symbol!==symbol||!Number.isFinite(r.at)||r.at!==snap.observedMs||r.at!==h.observedMs||r.at>now||now-r.at>300000||!dates?.length||new Set(dates).size!==dates.length||dates.some(d=>!r.expirations.includes(d)))return;
 const today=dateFormatter.format(now);if(dateFormatter.format(r.at)!==today||dates.some(d=>!/^\d{4}-\d{2}-\d{2}$/.test(d)||d<today))return;
 const ds=r.enrichment.details.filter(c=>dates.includes(c.expiration));
 if(!ds.length||new Set(ds.map(c=>c.ticker)).size!==ds.length||ds.some(c=>!Number.isFinite(c.strike)||c.strike<=0||!['call','put'].includes(c.kind)||[c.oi,c.volume,c.dex,c.gex].some(v=>v!=null&&(!Number.isFinite(v)||v<0))))return;
 return scopeContractReport(r,dates);
}
