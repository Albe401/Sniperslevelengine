import type {Candle} from './model';
import type {LevelRole,RouteStructure} from './route-level-context';
import {ROUTE_BAR_MS,routeSequenceDeadlineMs,routeSequenceBarExpired} from './route-sequence-policy';
export interface RouteLevel {name:string;price:number;observed:number;asof?:number;expiration:string;provisional:boolean;role?:LevelRole;strength?:number|null;change?:number|null;dynamicStrength?:number|null;source?:string;evidence?:import('./level-evidence').LevelEvidence}
export interface RouteSequenceSnapshot {
 identity:string;key:string;direction:number;phase:'recovery'|'test'|'sustained';
 impulse:{startMs:number;endMs:number;close:number};continuation:{startMs:number;endMs:number;close:number}|null;
 anchor:{name:string;price:number;expiration:string;firstObservedMs:number;asofMs:number|null;source:string|null};
 destination:{name:string;price:number;expiration:string;firstObservedMs:number;asofMs:number|null;source:string|null;selectedMs:number|null}|null;
}
export interface RouteEvidence {identity:string;now:number;levels:RouteLevel[];options:string;delta:string;blocks:string;structure?:RouteStructure}
export interface RouteResult {state:'waiting'|'range'|'recovery'|'test'|'sustained'|'invalidated'|'reached';direction:number;level:number|null;target:number|null;stop:number|null;reason:string;at:number;key:string;deadlineMs?:number;terminalCause?:'expired'}
const empty=(reason='Esperando velas cerradas y niveles vigentes'):RouteResult=>({state:'waiting',direction:0,level:null,target:null,stop:null,reason,at:0,key:''});
const median=(n:number[])=>{const s=n.slice().sort((a,b)=>a-b);return s.length%2?s[(s.length-1)/2]:(s[s.length/2-1]+s[s.length/2])/2;};
/** Experimental observation engine. No admission, broker or probability output. */
export class RouteEngine {
 identity='';bars:Candle[]=[];result=empty();
 candidate?:{anchor:RouteLevel;direction:number;impulse:Candle;test?:Candle;follow?:Candle;continuation?:Candle;targetRef?:RouteLevel;targetSelectedMs?:number;phase:'recovery'|'test'|'sustained'};
 /** Detached source metadata; no feed, reset, price reconstruction or decision change. */
 closedBarSnapshot(){const b=this.bars.at(-1);return b?Object.freeze({startMs:b.timestampMs,endMs:b.timestampMs+ROUTE_BAR_MS,close:b.close}):null;}
 sequenceSnapshot():Readonly<RouteSequenceSnapshot>|undefined {
  const k=this.candidate;if(!k)return undefined;
  const bar=(b:Candle)=>Object.freeze({startMs:b.timestampMs,endMs:b.timestampMs+ROUTE_BAR_MS,close:b.close});
  const level=(l:RouteLevel)=>({name:l.name,price:l.price,expiration:l.expiration,firstObservedMs:l.observed,asofMs:l.asof??null,source:l.source??null});
  return Object.freeze({identity:this.identity,key:this.result.key,direction:k.direction,phase:k.phase,impulse:bar(k.impulse),continuation:k.continuation?bar(k.continuation):null,anchor:Object.freeze(level(k.anchor)),destination:k.targetRef?Object.freeze({...level(k.targetRef),selectedMs:k.targetSelectedMs??null}):null});
 }
 reset(reason:string){this.bars=[];this.candidate=undefined;this.result=empty(reason);return this.result;}
 warm(bars:Candle[],identity:string){this.reset('Acumulando una nueva prueba en vivo');this.identity=identity;this.bars=bars.slice(-20);}
 feed(b:Candle,c:RouteEvidence):RouteResult {
  if(this.identity!==c.identity){this.reset('Activo, sesión o vencimiento nuevos');this.identity=c.identity;}
  if(![c.now,b.timestampMs,b.open,b.high,b.low,b.close,b.volume].every(Number.isFinite)||b.low<=0||b.volume<0||b.high<Math.max(b.open,b.close)||b.low>Math.min(b.open,b.close))return this.reset('Vela inválida');
  if(b.timestampMs+ROUTE_BAR_MS>c.now||c.now-b.timestampMs-ROUTE_BAR_MS>90000)return this.reset('Vela abierta o caducada');
  const last=this.bars.at(-1);
  if(last&&b.timestampMs<=last.timestampMs){if(JSON.stringify(b)!==JSON.stringify(last))return this.reset('Vela corregida: reconstruir contexto');return this.result;}
  if(last&&b.timestampMs-last.timestampMs!==ROUTE_BAR_MS)this.reset('Hueco en las velas: prueba reiniciada');
  const prior=this.bars.slice(-20);this.bars=[...prior,b];
  const levels=c.levels.filter(l=>Number.isFinite(l.price)&&l.price>0&&l.observed<=b.timestampMs&&(l.asof??l.observed)<=c.now&&c.now-(l.asof??l.observed)<=900000);
  const k=this.candidate;
  if(k){
   const tol=k.anchor.price*.0003;
   const deadlineMs=routeSequenceDeadlineMs(k.impulse.timestampMs+ROUTE_BAR_MS);
   const finish=(state:'invalidated'|'reached',reason:string,terminalCause?:'expired')=>{this.candidate=undefined;this.result={...this.result,state,direction:0,reason,at:c.now,deadlineMs,...(terminalCause?{terminalCause}:{}),...(state==='invalidated'?{target:null,stop:null}:{})};return this.result;};
   // Existing close at deadline is allowed; later closes cannot resolve by price.
   if(routeSequenceBarExpired(b.timestampMs+ROUTE_BAR_MS,deadlineMs))return finish('invalidated','La secuencia excedió diez minutos desde el cierre del impulso','expired');
   if(!levels.some(l=>l.name===k.anchor.name&&l.price===k.anchor.price&&l.expiration===k.anchor.expiration))return finish('invalidated','La referencia cambió o caducó');
   if((b.close-k.anchor.price)*k.direction < -tol)return finish('invalidated','El cierre perdió el nivel decisivo; giro contrario pendiente');
   if(k.phase==='sustained'&&this.result.target!=null&&(b.close-this.result.target)*k.direction>=0)return finish('reached','Cierre alcanzó la referencia; esperar una nueva secuencia');
   if(k.phase==='sustained'){
    if(!levels.some(l=>l.price===this.result.target))return finish('invalidated','Objetivo cambió o caducó');
    this.result={...this.result,at:c.now};
    return this.result;
   }
   const touch=b.low<=k.anchor.price+tol&&b.high>=k.anchor.price-tol;
   const extreme=k.direction===1?b.low>k.impulse.low:b.high<k.impulse.high;
   if(k.phase==='recovery'&&touch&&extreme&&(b.close-k.anchor.price)*k.direction>0&&b.volume<k.impulse.volume&&b.high-b.low<=k.impulse.high-k.impulse.low){
    k.test=b;k.phase='test';this.result={...this.result,state:'test',reason:'Prueba posterior: menor volumen y amplitud; falta continuación',at:c.now};return this.result;
   }
   if(k.phase==='test'&&k.test&&(b.close-k.test.close)*k.direction>tol&&b.volume>=k.test.volume){
    if(c.structure?.requiresAdditionalTest&&(!k.follow||(b.close-k.follow.close)*k.direction < -tol)){
     k.follow=b;this.result={...this.result,reason:'Continuación adicional pendiente; otra vela no verifica el respaldo del nivel',at:c.now};return this.result;
    }
    const target=levels.filter(l=>(l.price-b.close)*k.direction>tol).sort((a,z)=>Math.abs(a.price-b.close)-Math.abs(z.price-b.close))[0];
    if(!target){this.result={...this.result,reason:'Secuencia de precio presente; siguiente referencia pendiente',at:c.now};return this.result;}
    k.continuation=b;k.targetRef={...target};k.targetSelectedMs=c.now;k.phase='sustained';this.result={...this.result,state:'sustained',target:target.price,reason:'Recuperación, prueba y continuación presentes; recorrido en observación',at:c.now};return this.result;
   }
   if(k.phase==='test')k.follow=undefined;
   return this.result;
  }
  if(prior.length<8){this.result=empty('Faltan ocho velas comparables');return this.result;}
  const vol=median(prior.map(x=>x.volume)),spread=median(prior.map(x=>x.high-x.low));
  if(vol<=0||spread<=0||!levels.length){this.result=empty('Baseline o referencia anterior a la vela pendientes');return this.result;}
  for(const l of levels.slice().sort((a,z)=>Math.abs(a.price-b.close)-Math.abs(z.price-b.close))){
   if(l.role==='vacuum-edge'||l.role==='pull')continue;
   const tol=l.price*.0003,location=b.high>b.low?(b.close-b.low)/(b.high-b.low):.5;
   for(const direction of [1,-1]){
    const crossed=direction===1?b.low<l.price-tol&&b.close>l.price+tol:b.high>l.price+tol&&b.close<l.price-tol;
    if(crossed&&(direction===1?location>=.7:location<=.3)&&b.volume>=vol*1.2&&b.high-b.low>=spread){
     this.candidate={anchor:l,direction,impulse:b,phase:'recovery'};
     this.result={state:'recovery',direction,level:l.price,target:null,stop:l.price-direction*tol,reason:'Recuperación/rechazo candidato; todavía falta la prueba posterior',at:c.now,deadlineMs:routeSequenceDeadlineMs(b.timestampMs+ROUTE_BAR_MS),key:`${c.identity}:${l.name}:${l.price}:${b.timestampMs}`};return this.result;
    }
   }
  }
  this.result={...empty('Sin secuencia verificable'),state:'range',at:c.now};return this.result;
 }
}
