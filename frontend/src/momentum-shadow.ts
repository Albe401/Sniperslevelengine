import {alphaPath,momentumAlpha,type AlphaRead} from './momentum-alpha';
import type {LiveState} from './live';
import {buildIntradayControl} from './intraday-control';
import {readDirectionScore} from './direction-scanner';
import {currentSessionCandles} from './candle-series';
import {marketOpen} from './continuous-quick-audit';
import {routeChartReading} from './route-live';
import {intradayAtr} from './intraday-atr';
import {MomentumEngine,type MomentumInput,type MomentumRead} from './momentum-engine';
import {escapeHtml as esc} from './model';
const key='snipers.momentum-shadow.v1';
interface RecordRow {alpha?:AlphaRead;rvol?:number|null;input:MomentumInput;read:MomentumRead;after5:number|null;outcome:'pending'|'observed'|'missing'}
export class MomentumShadow {
 enabled=false;waiting='Pausado';error='';records:RecordRow[]=[];current?:RecordRow;
 private engine=new MomentumEngine();private identity='';private lastMinute=0;private lastPoll=0;
 constructor(){try{const v=JSON.parse(localStorage.getItem(key)??'null');if(v?.version===1){this.enabled=v.enabled===true;this.waiting=this.enabled?'Armado · esperando apertura y datos vigentes':'Pausado';this.records=Array.isArray(v.records)?v.records.filter((r:RecordRow)=>r?.input&&r?.read&&Number.isFinite(r.input.at)&&Number.isFinite(r.input.price)&&typeof r.input.symbol==='string'&&['pending','observed','missing'].includes(r.outcome)).slice(-500):[];}}catch{this.error='Historial local no recuperado';}}
 save(){try{localStorage.setItem(key,JSON.stringify({version:1,enabled:this.enabled,records:this.records}));this.error='';}catch{this.error='No se pudo guardar; registros conservados en memoria';}}
 toggle(){this.enabled=!this.enabled;this.engine.reset();this.lastMinute=0;this.current=undefined;this.waiting=this.enabled?'Armado · esperando apertura y datos vigentes':'Pausado';this.save();}
 observe(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()){
  if(!this.enabled)return;
  if(!marketOpen(s,now)){this.waiting='Armado · esperando sesión regular (09:30 ET) y calendario';this.engine.reset();return;}
  if(!active||s?.symbol!==symbol||s.publishedMs>now||now-s.publishedMs>3000){this.waiting='Esperando conexión y publicación vigente';this.engine.reset();return;}
  if(now-this.lastPoll<1000)return;this.lastPoll=now;
  const session=s.metrics.session,id=symbol+':'+session;
  if(this.identity!==id){this.identity=id;this.engine.reset();this.lastMinute=0;this.current=undefined;}
  const bars=currentSessionCandles(s.chartHistory,1,now).filter(b=>b.complete&&b.sessionDate===session),bar=bars.at(-1);
  if(!bar||now-bar.timestampMs-60000>90000){this.waiting='Esperando vela cerrada vigente';this.engine.reset();return;}
  // Sample once per actual wall-clock minute; never backfill historical option flow.
  const minute=Math.floor(now/60000);if(minute===this.lastMinute)return;this.lastMinute=minute;const at=now;
  const r=buildIntradayControl(s,symbol,active,now),d=readDirectionScore(s,r,active,now),route=routeChartReading(symbol,now)?.result;
  const f=d.components.find(c=>c.name==='Prima clasificada 5m')?.signal??null;
  const p=d.components.find(c=>c.name==='Precio y VWAP')?.signal??null;
  const direction=d.side==='CALL'?1:d.side==='PUT'?-1:0;
  const recent=bars.slice(-6),contiguous=recent.length===6&&recent.every((b,i)=>!i||b.timestampMs-recent[i-1].timestampMs===60000);
  const avg=contiguous?recent.slice(0,-1).reduce((n,b)=>n+b.volume,0)/5:0;
  const price=s.ticks[symbol]?.price;
  const input:MomentumInput={symbol,session,at,price:price??bar.close,baseline:`${d.side} · ${d.call??'—'}/${d.put??'—'} · ruta ${route?.state??'pendiente'}`,direction,flow:f,volumeRatio:avg>0?bar.volume/avg:null,priceAligned:p!=null&&p*direction>0,routeConfirmed:route?.state==='sustained'&&route.direction===direction,target:route?.direction===direction?route.target:null,stop:route?.direction===direction?route.stop:null,atr:intradayAtr(s,symbol,active,now).value,ready:d.call!=null&&f!=null&&contiguous&&price!=null,reason:'Precio, prima clasificada, estructura o minutos comparables pendientes'};
  const read=this.engine.feed(input);
  const previous=this.records.filter(v=>v.input.symbol===symbol&&v.input.session===session&&at-v.input.at>=300000&&at-v.input.at<=360000).at(-1);
  const rvol=s.metrics.baselineSessions>=5&&s.metrics.asofMs!=null&&s.metrics.asofMs<=now&&now-s.metrics.asofMs<=180000?s.metrics.rvol:null;
  const alpha=momentumAlpha(read,alphaPath(bars,at,input.price,direction,input.target,input.stop,input.atr),rvol,previous?.rvol??null,input.volumeRatio,previous?.input.volumeRatio??null);
  for(const row of this.records.filter(v=>v.input.symbol===symbol&&v.outcome==='pending')){
   const elapsed=at-row.input.at;
   const trail=this.records.filter(v=>v.input.symbol===symbol&&v.input.at>=row.input.at);
   const observedContinuously=trail.every((v,i)=>v.input.ready&&(!i||v.input.at-trail[i-1].input.at<=90000))&&at-(trail.at(-1)?.input.at??0)<=90000;
   if(elapsed>=300000&&elapsed<=360000&&row.input.session===session&&contiguous&&input.ready&&observedContinuously){row.after5=(input.price/row.input.price-1)*10000;row.outcome='observed';}
   else if(elapsed>=300000||row.input.session!==session)row.outcome='missing';
  }
  this.current={input,read,alpha,rvol,after5:null,outcome:'pending'};this.records.push(this.current);this.records=this.records.slice(-500);this.waiting='Comparando · activo seleccionado · una muestra por minuto';this.save();
 }
}
export const momentumShadow=new MomentumShadow();
export function momentumPanel():string {
 const a=momentumShadow,c=a.current;
 const time=(n:number)=>new Date(n).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false});
 return `<section class="learn-panel" aria-label="Momentum experimental"><div class="learn-section-head"><h2>Alfa + Momentum · comparación experimental</h2><button class="learn-button" data-momentum-toggle aria-pressed="${a.enabled}">${a.enabled?'Pausar motor':'Armar motor'}</button></div><p role="status">${esc(a.waiting)}</p><div class="learn-main-grid"><article><small>LECTURA ACTUAL</small><h3>${esc(c?.input.baseline??'Esperando datos')}</h3></article><article><small>MOMENTUM ALPHA v2 · EVIDENCIA, NO PROBABILIDAD</small><h3>${esc(c?`${c.read.side} · ${c.alpha?.state??c.read.state} · ${c.alpha?.score??'—'}/100`:'Pendiente')}</h3><p>${esc(c?.alpha?.reason??c?.read.reason??'Puedes armarlo a cualquier hora. Comienza con sesión regular y conexión vigente.')}</p></article></div><p>${c?`Aceleración de presión: ${c.read.acceleration?.toFixed(2)??'—'} · recorrido / invalidación: ${c.read.space?.toFixed(2)??'—'} · volumen / media local 5 min: ${c.input.volumeRatio?.toFixed(2)??'—'}`:'Antes de abrir no genera señales. Necesita acumular ventanas comparables.'}</p>${c?.alpha?`<p>Camino: ${esc(c.alpha.path.reason)} · primer obstáculo ${c.alpha.path.obstacle?.toFixed(2)??'—'} · Δ RVOL ${c.alpha.rvolChange?.toFixed(2)??'pendiente'} · Δ volumen local ${c.alpha.volumeChange?.toFixed(2)??'pendiente'}</p>`:''}${a.error?`<p class="learn-error">${esc(a.error)}</p>`:''}<details><summary>Comparaciones guardadas · ${a.records.length}</summary><div class="learn-table-scroll"><table><thead><tr><th>Activo / ET</th><th>Actual</th><th>Momentum</th><th>Subyacente +5 min</th></tr></thead><tbody>${a.records.slice(-20).reverse().map(x=>`<tr><td>${esc(x.input.symbol)} ${time(x.input.at)}</td><td>${esc(x.input.baseline)}</td><td>${esc(x.read.side+' · '+x.read.state)} ${x.read.score??'—'}/100</td><td>${x.outcome==='observed'&&x.after5!=null?(x.after5/100).toFixed(3)+'%':x.outcome==='missing'?'Sin observación':'En seguimiento'}</td></tr>`).join('')||'<tr><td colspan="4">Sin muestras todavía.</td></tr>'}</tbody></table></div></details><p class="learn-note">Versión de observación: precio/VWAP, cambio de prima entre ventanas separadas 5 min, volumen local y recorrido de la ruta. No agrega de nuevo CVD ni bloques del mismo flujo. Historial local, últimas 500 muestras; exige aplicación abierta y Mercado conectado. No cambia señales ni bot. No demuestra alpha; faltan validación fuera de muestra, costes e internos del ETF.</p></section>`;
}
