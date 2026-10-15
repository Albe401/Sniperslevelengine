import type {MarketEvidence} from './market-evidence';
import type {AtrRead} from './intraday-atr';
/** Prospective references, available before a confirmed route. Never selects an entry or invents a probability. */
export function routeLevelDestinations(m:MarketEvidence,now:number,a?:AtrRead){
 const spot=m.price?.spot;
 if(!Number.isFinite(now)||!m.price?.ready||spot==null||!Number.isFinite(spot)||spot<=0||!Number.isFinite(m.at)||m.at>now||now-m.at>3000)return [];
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const levels=m.levels.filter(l=>Number.isFinite(l.price)&&l.price>0&&l.role!=='vacuum-edge'&&l.expiration===today&&Number.isFinite(l.observed)&&l.observed<=now&&Number.isFinite(l.asof??l.observed)&&(l.asof??l.observed)<=now&&now-(l.asof??l.observed)<=300000);
 const atr=a?.value!=null&&Number.isFinite(a.value)&&a.value>0&&a.endMs!=null&&a.endMs<=now&&now-a.endMs<300000?a.value:null;
 return [1,-1].flatMap(direction=>{
  const target=levels.filter(l=>(l.price-spot)*direction>0).sort((a,b)=>Math.abs(a.price-spot)-Math.abs(b.price-spot))[0];
  if(!target)return [];
  const distance=Math.abs(target.price-spot),strength=target.strength!=null&&Number.isFinite(target.strength)&&target.strength>=0&&target.strength<=100?target.strength:null;
  return [{direction,origin:spot,target:target.price,name:target.name,distance,atr:atr==null?null:distance/atr,strength,provisional:target.provisional,at:now,
   label:`${direction>0?'CALL ↑':'PUT ↓'} $${target.price.toFixed(2)} · ${target.name}`,
   condition:`Si el precio avanza ${direction>0?'por encima':'por debajo'} de $${spot.toFixed(2)} hacia esta referencia; falta confirmar prueba y continuación. No es una entrada.`,
   invalidation:'Se suspende al cambiar el nivel, perder vigencia los datos o alcanzar la referencia.'}];
 });
}
