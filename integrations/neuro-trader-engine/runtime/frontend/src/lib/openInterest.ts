import type {ProfileStrike} from './types';
const count=(n:number|undefined)=>n!=null&&Number.isFinite(n)&&n>=0?n:null;
export function strikeOpenInterest(s:ProfileStrike,includeZero:boolean){
 const side=(all:number|undefined,zero:number|undefined)=>{
  const a=count(all);if(a==null)return null;if(includeZero)return a;
  const z=count(zero);return z==null||z>a?null:a-z;
 };
 return {call:side(s.oi_c,s.oi_c0),put:side(s.oi_p,s.oi_p0)};
}
export function totalOpenInterest(rows:ProfileStrike[],includeZero:boolean){
 const values=rows.map(s=>strikeOpenInterest(s,includeZero));
 const sum=(side:'call'|'put')=>values.length&&values.every(v=>v[side]!=null)?values.reduce((a,v)=>a+v[side]!,0):null;
 return {call:sum('call'),put:sum('put')};
}
export function oiNumber(n:number|null,compact=false){return n==null?'—':new Intl.NumberFormat('es',{notation:compact?'compact':'standard',maximumFractionDigits:compact?1:0}).format(n);}
