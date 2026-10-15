import type {Block} from './investment-research';
export interface SavedBlocks {minimum:number;report:{date:string;throughMs:number;rows:{ticker:string;block:Block}[];coverage:string[]}}
export interface ClosingBar {symbol:string;date:string;open:number;high:number;low:number;close:number;volume:number}
export interface Holding {symbol:string;weight:number}
export interface Holdings {asOf:string;source:string;rows:Holding[]}
export type Side='BUY'|'SELL'|'NEUTRAL'|'PENDING';
export const etDate=(at:number)=>new Date(at).toLocaleDateString('sv-SE',{timeZone:'America/New_York'});
export const etHour=(at:number)=>Number(new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hour:'2-digit',hourCycle:'h23'}).format(at));
export const etMinute=(at:number)=>Number(new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',minute:'2-digit'}).format(at));
export const percent=(n:number|null)=>n==null?'—':`${n.toFixed(1)}%`;
export const money=(n:number)=>new Intl.NumberFormat('en-US',{style:'currency',currency:'USD',notation:'compact',maximumFractionDigits:2}).format(n);
