import type {AtrRead} from './intraday-atr';
export interface RouteAtrContext {value:number|null;distance:number|null;targetAtr:number|null;stopAtr:number|null;endMs:number|null;detail:string}
/** Magnitude context only: ATR does not select direction, target, or arrival time. */
export function routeAtrContext(a:AtrRead|undefined,spot:number,target:number|null,stop:number|null,now:number):RouteAtrContext {
 const pending:RouteAtrContext={value:null,distance:null,targetAtr:null,stopAtr:null,endMs:null,detail:'ATR 5 min pendiente: requiere 75 minutos RTH completos y continuos; no confirma el alcance del objetivo.'};
 if(!a||!Number.isFinite(now)||a.value==null||!Number.isFinite(a.value)||a.value<=0||a.endMs==null||!Number.isFinite(a.endMs)||a.endMs>now||now-a.endMs>=300000||!Number.isFinite(spot)||spot<=0)return pending;
 const distance=target!=null&&Number.isFinite(target)&&target>0?Math.abs(target-spot):null;
 const targetAtr=distance==null?null:distance/a.value;
 const stopAtr=stop!=null&&Number.isFinite(stop)&&stop>0?Math.abs(spot-stop)/a.value:null;
 if([targetAtr,stopAtr].some(v=>v!=null&&!Number.isFinite(v)))return pending;
 return {value:a.value,distance,targetAtr,stopAtr,endMs:a.endMs,detail:`ATR14 · 5 min $${a.value.toFixed(2)}${distance==null?'':` · hasta llegada $${distance.toFixed(2)} = ${targetAtr!.toFixed(2)} ATR`}${stopAtr==null?'':` · hasta invalidación ${stopAtr.toFixed(2)} ATR`}. Magnitud reciente por vela de 5 min; no es límite de recorrido, probabilidad ni tiempo de llegada.`};
}
