import type {EtfReport} from './etf-dealers';
import {sameStrike} from './strike-key';
import {reportOiHeatmap} from './option-oi-heatmap-report';
import {strikeMillis,type Heatmap} from './option-oi-heatmap';
const verifiedByEvidence=new WeakMap<object,Map<string,Heatmap>>();
function verifiedMap(report:EtfReport,dates:string[]):Heatmap {
 const evidence=report.heatmap!;
 let scoped=verifiedByEvidence.get(evidence);
 if(!scoped){scoped=new Map();verifiedByEvidence.set(evidence,scoped);}
 const key=JSON.stringify([report.symbol,report.at,[...dates].sort()]);
 let map=scoped.get(key);
 if(!map){map=reportOiHeatmap({...report,expirations:dates});scoped.set(key,map);}
 return map;
}
/** OI is a snapshot stock of contracts, never an execution-side or dealer inventory. */
export function levelOpenInterest(report:EtfReport|undefined,strike:number,expirations?:string[]){
 if(!report||expirations&&(!expirations.length||expirations.some(e=>!report.expirations.includes(e))))return null;
 const dates=expirations??report.expirations;
 if(!dates.length||new Set(dates).size!==dates.length)return null;
 const finish=(call:number,put:number,contracts:number,verification:'reference'|'snapshot',scopeComplete:boolean,oiDate:string|null)=>{
  const total=call+put;
  if(![call,put,total].every(n=>Number.isSafeInteger(n)&&n>=0)||total===0)return null;
  const callPct=Math.round(100*call/total);
  return {call,put,total,callPct,putPct:100-callPct,contracts,expirations:dates,verification,scopeComplete,oiDate};
 };
 // The full-chain inventory already carries the independent reference used by the OI map.
 // Reuse its verified cells when present; do not silently fall back on a conflicting snapshot.
 if(report.heatmap){
  try{
   const key=strikeMillis(strike),map=verifiedMap(report,dates);
   let call=0,put=0,contracts=0;
   for(const date of dates)for(const side of ['call','put'] as const){
    const cell=map.cells.find(c=>c.strikeMillis===key&&c.expiration===date&&c.side===side);
    if(!cell||!['known','no_contract','excluded'].includes(cell.state))return null;
    const value=cell.state==='known'?cell.oi:0;
    if(value==null)return null;
    if(side==='call')call+=value;else put+=value;
    contracts+=cell.knownContracts;
   }
   return finish(call,put,contracts,'reference',map.universeComplete,map.oiDate);
  }catch{return null;}
 }
 // The lightweight live chain lacks independent reference rows. Preserve a local sum,
 // explicitly marked as snapshot-only, until the verified inventory is available.
 const rows=report.enrichment?.details.filter(c=>sameStrike(c.strike,strike)&&dates.includes(c.expiration))??[];
 if(dates.some(e=>!['call','put'].every(kind=>rows.some(c=>c.expiration===e&&c.kind===kind))))return null;
 if(!rows.length||new Set(rows.map(c=>c.ticker)).size!==rows.length||rows.some(c=>c.oi==null||!Number.isSafeInteger(c.oi)||c.oi<0))return null;
 const call=rows.filter(c=>c.kind==='call').reduce((n,c)=>n+c.oi!,0),put=rows.filter(c=>c.kind==='put').reduce((n,c)=>n+c.oi!,0);
 return finish(call,put,rows.length,'snapshot',false,report.enrichment?.oiReferenceDate??null);
}
