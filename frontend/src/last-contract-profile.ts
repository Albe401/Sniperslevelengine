import type {EtfReport} from './etf-dealers';
import type {LiveState} from './live';
import {scopeContractReport} from './contract-profile';
import {profileExpirations,historicalProfileReference,type ProfileHorizon} from './profile-expirations';
const formatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'});
const memory=new Map<string,EtfReport>();
/** Display-only retained chain. Never supplied to live signals or order logic. */
export function retainedContractProfile(s:LiveState|undefined,symbol:string,now=Date.now(),mode?:ProfileHorizon):EtfReport|undefined {
 const key=`snipers.contract-profile.v1.${symbol}`,candidate=s?.symbol===symbol?s.optionSnapshotStatus?.structuralEnrichment:undefined;
 const valid=(r:EtfReport|null|undefined):r is EtfReport=>!!r&&r.symbol===symbol&&Number.isFinite(r.at)&&r.at<=now&&now-r.at<=7*86400000&&!!r.enrichment?.details.length&&r.enrichment.details.length<=10000&&new Set(r.enrichment.details.map(c=>c.ticker)).size===r.enrichment.details.length&&r.enrichment.details.every(c=>Number.isFinite(c.strike)&&c.strike>0&&/^\d{4}-\d{2}-\d{2}$/.test(c.expiration)&&['call','put'].includes(c.kind)&&[c.oi,c.volume,c.dex,c.gex].every(v=>v==null||Number.isFinite(v)&&v>=0));
 let r=memory.get(symbol);
 if(!r){try{const raw=localStorage.getItem(key);if(raw&&raw.length<4000000){const saved=JSON.parse(raw);if(valid(saved))r=saved;}}catch{/* storage unavailable */}}
 if(valid(candidate)&&(!r||candidate.at>r.at)){
  if(candidate!==r){try{const raw=JSON.stringify(candidate);if(raw.length<4000000)localStorage.setItem(key,raw);}catch{/* retain in memory */}}
  r=candidate;memory.set(symbol,r);
 }
 if(!valid(r))return;
 const today=formatter.format(now),session=s?.chartHistory?.sessions.find(x=>x.date===today);
 const parts=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',weekday:'short',hour:'2-digit',minute:'2-digit',hourCycle:'h23'}).formatToParts(now);
 const part=(type:string)=>parts.find(p=>p.type===type)?.value??'';
 const weekday=!['Sat','Sun'].includes(part('weekday')),minutes=Number(part('hour'))*60+Number(part('minute'));
 const opening=session?.openMs;
 if(!mode&&!session&&weekday&&minutes>=570&&formatter.format(r.at)!==today)return;
 // A known new session replaces the prior chain at its real market opening.
 if(!mode&&opening!=null&&now>=opening&&r.at<opening&&formatter.format(r.at)!==today)return;
 if(!mode&&s?.symbol===symbol&&s.metrics?.phase==='Sesión RTH'&&formatter.format(r.at)!==today)return;
 // Historical filters refer to the saved observation date, never relabel old OI as live.
 const reference=mode?formatter.format(r.at):today;
 const dates=[...new Set(r.enrichment!.details.map(c=>c.expiration).filter(d=>d>=reference))].sort();
 if(!dates.length)return;
 return scopeContractReport(r,mode?profileExpirations(r,mode,historicalProfileReference(r)):[dates[0]]);
}
