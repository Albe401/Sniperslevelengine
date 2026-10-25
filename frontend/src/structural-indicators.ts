import {cachedTechnical} from './structural-technical-cache';
import type {PriceBar,PriceInterval} from './price-structure';

/** Documented recurrences; periods, slope horizon and convergence tolerances are project conventions. */
export const STRUCTURAL_INDICATOR_VERSION='structural-indicators-v1';
export interface StructuralIndicators {
 version:typeof STRUCTURAL_INDICATOR_VERSION;at:number;bars:number;ready:boolean;reason:string;
 ema9:number|null;ema20:number|null;ema21:number|null;distanceAtr:number|null;slope3Atr:number|null;
 plusDi:number|null;minusDi:number|null;adx:number|null;
 emaSeedAtr:number|null;adxSeedPoints:number|null;
 trend:'CALL'|'PUT'|'MIXTA'|'PENDIENTE';strength:'FUERTE'|'TRANSICIÓN'|'DÉBIL'|'PENDIENTE';
 rvol:{value:number|null;references:number;required:20;mean:number|null;method:string};
}
const finite=(x:unknown):x is number=>typeof x==='number'&&Number.isFinite(x);
const timeET=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hourCycle:'h23'});
export function emaSeries(bars:readonly PriceBar[],period:number):number[] {
 const result:number[]=Array(bars.length).fill(NaN);if(!Number.isInteger(period)||period<1||bars.length<period)return result;
 let e=bars.slice(0,period).reduce((n,b)=>n+b.close,0)/period;result[period-1]=e;
 const alpha=2/(period+1);for(let i=period;i<bars.length;i++){e+=alpha*(bars[i].close-e);result[i]=e;}return result;
}
export function dmi14(bars:readonly PriceBar[]):{plusDi:number|null;minusDi:number|null;adx:number|null} {
 let tr=0,plus=0,minus=0,adx:number|null=null;const dxs:number[]=[];let plusDi:number|null=null,minusDi:number|null=null;
 for(let i=1;i<bars.length;i++){
  const b=bars[i],p=bars[i-1],u=b.high-p.high,d=p.low-b.low;
  const t=Math.max(b.high-b.low,Math.abs(b.high-p.close),Math.abs(b.low-p.close));
  const up=u>d&&u>0?u:0,down=d>u&&d>0?d:0;
  if(i<=14){tr+=t;plus+=up;minus+=down;}else{tr=tr-tr/14+t;plus=plus-plus/14+up;minus=minus-minus/14+down;}
  if(i<14)continue;
  plusDi=tr>0?100*plus/tr:0;minusDi=tr>0?100*minus/tr:0;
  const dx=plusDi+minusDi>0?100*Math.abs(plusDi-minusDi)/(plusDi+minusDi):0;
  if(dxs.length<14){dxs.push(dx);if(dxs.length===14)adx=dxs.reduce((n,v)=>n+v,0)/14;}
  else adx=(adx!*13+dx)/14;
 }
 return {plusDi,minusDi,adx};
}
/** Strict comparable closed bars; no fallback to an earlier offset or extrapolation of open candles. */
export function comparableRvol(bars:readonly PriceBar[],interval:PriceInterval):StructuralIndicators['rvol'] {
 const last=bars.at(-1),method=interval<1440?'20 sesiones previas, misma hora ET y duración RTH':interval===1440?'20 sesiones previas de igual duración RTH':'20 semanas previas de igual duración bursátil';
 const result:StructuralIndicators['rvol']={value:null,references:0,required:20,mean:null,method};if(!last)return result;
 const duration=(b:PriceBar)=>b.tradingMinutes??(b.endMs-b.timestampMs)/60000;
 const matches:PriceBar[]=[];const offset=interval<1440?timeET.format(last.timestampMs):'';for(let j=bars.length-2;j>=0&&matches.length<20;j--){const b=bars[j];if(duration(b)===duration(last)&&(interval>=1440||b.sessionDate!==last.sessionDate&&timeET.format(b.timestampMs)===offset))matches.push(b);}
 result.references=matches.length;if(matches.length!==20)return result;
 result.mean=matches.reduce((n,b)=>n+b.volume,0)/20;if(result.mean>0&&finite(last.volume)&&last.volume>=0)result.value=last.volume/result.mean;return result;
}
export function structuralIndicators(bars:readonly PriceBar[],interval:PriceInterval,atr:number|null,cacheKey?:string):StructuralIndicators {
 const primary=cacheKey?cachedTechnical(bars,cacheKey):undefined;
 const last=bars.at(-1),e21=primary?[]:emaSeries(bars,21),e9=primary?[]:emaSeries(bars,9),e20=primary?[]:emaSeries(bars,20),di=primary?{plusDi:primary.plusDi,minusDi:primary.minusDi,adx:primary.adx}:dmi14(bars);
 const number=(x:number|undefined)=>finite(x)?x:null,ema21=primary?primary.ema21:number(e21.at(-1));
 const alt=bars.slice(14),alternative=cacheKey?cachedTechnical(alt,cacheKey+':seed14'):undefined,altE=alternative?alternative.ema21:emaSeries(alt,21).at(-1),altD=alternative?{adx:alternative.adx}:dmi14(alt);
 const emaSeedAtr=atr!=null&&atr>0&&ema21!=null&&finite(altE)?Math.abs(altE-ema21)/atr:null;
 const adxSeedPoints=di.adx!=null&&altD.adx!=null?Math.abs(di.adx-altD.adx):null;
 const ready=bars.length>=100&&atr!=null&&atr>0&&emaSeedAtr!=null&&emaSeedAtr<=.01&&adxSeedPoints!=null&&adxSeedPoints<=.5;
 const distanceAtr=last&&ema21!=null&&atr!=null&&atr>0?(last.close-ema21)/atr:null;
 const prior=primary?primary.ema21ThreeAgo:e21.at(-4),slope3Atr=ema21!=null&&finite(prior)&&atr!=null&&atr>0?(ema21-prior)/atr:null;
 const trend=!ready?'PENDIENTE':distanceAtr!>0&&slope3Atr!>0&&di.plusDi!>di.minusDi!?'CALL':distanceAtr!<0&&slope3Atr!<0&&di.plusDi!<di.minusDi!?'PUT':'MIXTA';
 return {version:STRUCTURAL_INDICATOR_VERSION,at:last?.endMs??0,bars:bars.length,ready,reason:ready?'Indicadores de las mismas velas cerradas.':'Esperando 100 velas y estabilidad de EMA/ADX.',ema9:primary?primary.ema9:number(e9.at(-1)),ema20:primary?primary.ema20:number(e20.at(-1)),ema21,distanceAtr,slope3Atr,...di,emaSeedAtr,adxSeedPoints,trend,strength:!ready?'PENDIENTE':di.adx!>=25?'FUERTE':di.adx!>=20?'TRANSICIÓN':'DÉBIL',rvol:comparableRvol(bars,interval)};
}
/** Frozen snapshot indicators must carry their own version, closing clock and bounded numerical evidence. */
export function validStructuralIndicators(x:StructuralIndicators|undefined,at:number|null,bars:number):boolean {
 if(!x||x.version!==STRUCTURAL_INDICATOR_VERSION||x.at!==at||x.bars!==bars||typeof x.ready!=='boolean'||typeof x.reason!=='string')return false;
 if([x.ema9,x.ema20,x.ema21,x.distanceAtr,x.slope3Atr,x.plusDi,x.minusDi,x.adx,x.emaSeedAtr,x.adxSeedPoints].some(v=>v!==null&&!finite(v)))return false;
 if([x.emaSeedAtr,x.adxSeedPoints].some(v=>v!==null&&v<0))return false;
 if([x.ema9,x.ema20,x.ema21].some(v=>v!==null&&v<=0)||[x.plusDi,x.minusDi,x.adx].some(v=>v!==null&&(v<0||v>100+1e-9)))return false;
 if(!['CALL','PUT','MIXTA','PENDIENTE'].includes(x.trend)||!['FUERTE','TRANSICIÓN','DÉBIL','PENDIENTE'].includes(x.strength)||!x.rvol||!Number.isInteger(x.rvol.references)||x.rvol.references<0||x.rvol.references>20||x.rvol.required!==20||typeof x.rvol.method!=='string')return false;
 if([x.rvol.mean,x.rvol.value].some(v=>v!==null&&(!finite(v)||v<0))||x.rvol.value!==null&&(x.rvol.references!==20||!(x.rvol.mean!>0)))return false;
 if(x.ready&&(bars<100||[x.ema21,x.distanceAtr,x.slope3Atr,x.plusDi,x.minusDi,x.adx,x.emaSeedAtr,x.adxSeedPoints].some(v=>v===null)||x.emaSeedAtr!>.01||x.adxSeedPoints!>.5))return false;
 const trend=!x.ready?'PENDIENTE':x.distanceAtr!>0&&x.slope3Atr!>0&&x.plusDi!>x.minusDi!?'CALL':x.distanceAtr!<0&&x.slope3Atr!<0&&x.plusDi!<x.minusDi!?'PUT':'MIXTA';
 return x.trend===trend&&x.strength===(!x.ready?'PENDIENTE':x.adx!>=25?'FUERTE':x.adx!>=20?'TRANSICIÓN':'DÉBIL');
}
