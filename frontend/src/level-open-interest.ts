import type {EtfReport} from './etf-dealers';
import {sameStrike} from './strike-key';
/** OI is a snapshot stock of contracts, never an execution-side or dealer inventory. */
export function levelOpenInterest(report:EtfReport|undefined,strike:number,expirations?:string[]){
 if(expirations&&(!expirations.length||expirations.some(e=>!report?.expirations.includes(e))))return null;
 const rows=report?.enrichment?.details.filter(c=>sameStrike(c.strike,strike)&&(!expirations||expirations.includes(c.expiration)))??[];
 if(expirations&&expirations.some(e=>!['call','put'].every(kind=>rows.some(c=>c.expiration===e&&c.kind===kind))))return null;
 if(!rows.length||new Set(rows.map(c=>c.ticker)).size!==rows.length||!rows.some(c=>c.kind==='call')||!rows.some(c=>c.kind==='put')||rows.some(c=>c.oi==null||!Number.isFinite(c.oi)||c.oi<0))return null;
 const call=rows.filter(c=>c.kind==='call').reduce((n,c)=>n+c.oi!,0),put=rows.filter(c=>c.kind==='put').reduce((n,c)=>n+c.oi!,0),total=call+put;
 if(total<=0||!Number.isFinite(total))return null;
 const callPct=Math.round(100*call/total);
 return {call,put,total,callPct,putPct:100-callPct,contracts:rows.length,expirations:expirations?[...new Set(expirations)]:report!.expirations};
}
