import type {LiveState} from './live';
import {currentSessionCandles,sessionCandles} from './candle-series';
import {premarketDirection} from './premarket-direction';
import {interpretationFresh} from './confirmations';
import {escapeHtml as esc,price} from './model';

export interface PriceNow {premarket?:boolean;spot?:number;direction?:number;ready:boolean;headline:string;tone:string;pace:string;structure:string;vwap:string;change:string;levels:string;at:string;flow:string}
const positive=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>0;
const etDateFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const dateET=(n:number)=>etDateFormatter.format(n);
// Provider events may lead the local clock slightly. Bound this tolerance to 1 s;
// this is display-only and does not change broker/strategy freshness gates.
const recent=(t:number|undefined|null,now:number,age:number)=>t!=null&&Number.isFinite(t)&&t<=now+1000&&now-t<=age&&dateET(t)===dateET(now);
/** Describes stock prices independently of the options model. Never authorizes an entry. */
export function readPriceNow(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):PriceNow {
 const out:PriceNow={ready:false,headline:'ESPERANDO DATOS DE PRECIO',tone:'waiting',pace:'Esperando velas recientes',structure:'Estructura pendiente',vwap:'VWAP pendiente',change:'Sin comparación reciente',levels:'Sin referencias recientes',at:'Sin precio vigente',flow:'Opciones: confirmación incompleta'};
 const tick=s?.ticks?.[symbol];
 if(!active||s?.symbol!==symbol||!interpretationFresh(s.publishedMs,now)||s.stocks.phase!=='streaming'||tick?.symbol!==symbol||!positive(tick.price)||!recent(tick.tradeMs,now,15000))return out;
 const today=dateET(now);
 const spot=tick.price;out.spot=spot;
 out.at=`${new Date(tick.tradeMs!).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET · $${price(spot)}${tick.tradeMs!>now?' · reloj proveedor <1 s adelantado':''}`;
 const metrics=s.metrics;
 if(metrics.session===today&&metrics.phase==='Sesión RTH'&&recent(metrics.asofMs,now,180000)&&positive(metrics.vwap))out.vwap=`${spot<metrics.vwap?'BAJO':spot>metrics.vwap?'SOBRE':'EN'} VWAP $${price(metrics.vwap)} · ${spot-metrics.vwap>=0?'+':''}${price(spot-metrics.vwap)} puntos`;
 // Canonical closed one-minute bars only; gaps break the comparison window.
 let bars=currentSessionCandles(s.chartHistory,1,now).filter(b=>b.complete&&b.sessionDate===today);
 if(premarketDirection(s,symbol,active,now)){
  const session=s.chartHistory!.sessions.find(v=>v.date===today)!;
  const end=Math.floor(now/60000)*60000-60000;
  const minutes=(s.minutes?Object.values(s.minutes):s.chartHistory!.minutes).filter(b=>b.timestampMs>=end-300000&&b.timestampMs<=end);
  bars=sessionCandles({sessions:[{...session,openMs:session.openMs-19800000,closeMs:session.openMs}],minutes},[],1,now);
  out.premarket=true;out.vwap='Premarket · VWAP RTH todavía no disponible';out.flow='Premarket · flujo de opciones todavía no disponible';
 }
 if(!recent(bars.at(-1)?.timestampMs,now,150000)){out.headline='PRECIO VIGENTE · HISTORIAL PENDIENTE';return out;}
 bars=bars.slice(-6);
 for(let i=bars.length-1;i>0;i--)if(bars[i].timestampMs-bars[i-1].timestampMs!==60000){bars=bars.slice(i);break;}
 if(bars.length<4){out.headline='PRECIO VIGENTE · ACUMULANDO VELAS';return out;}
 const last=bars.at(-1)!,prev=bars.at(-2)!,old=bars[0];
 const tolerance=spot*.00015; // 0.015% noise band, disclosed in evidence.
 const delta=spot-old.close,short=spot-last.close;
 const direction=delta>tolerance?1:delta<-tolerance?-1:0;
 const counter=direction!==0&&short*direction<-tolerance;
 out.ready=true;out.direction=counter?-direction:direction;
 out.headline=counter?(direction<0?'REBOTE DURANTE LA CAÍDA':'RETROCESO DURANTE LA SUBIDA'):direction<0?'PRECIO CAYENDO':direction>0?'PRECIO SUBIENDO':'PRECIO LATERAL EN LA VENTANA';
 out.tone=counter?(direction<0?'buyers':'sellers'):direction<0?'sellers':direction>0?'buyers':'balanced';
 // Compare equal two-minute windows of closed candles, independent of refresh frequency.
 const a=bars.at(-4)!,b=bars.at(-3)!;
 const older=(b.close-a.open)/2,newer=(last.close-prev.open)/2;
 out.pace=counter?'MOVIMIENTO CONTRARIO RECIENTE · GIRO NO CONFIRMADO':direction===0?'SIN DESPLAZAMIENTO NETO CLARO':newer*direction<=0?'PRECIO ACTUAL Y VELAS CERRADAS DIVERGEN':
   (newer-older)*direction>tolerance?'ACELERANDO EN VELAS CERRADAS':(newer-older)*direction<-tolerance?'PERDIENDO VELOCIDAD EN VELAS CERRADAS':direction<0?'CAÍDA SOSTENIDA':'SUBIDA SOSTENIDA';
 const trio=bars.slice(-3);
 out.structure=trio.slice(1).every((v,i)=>v.high<trio[i].high&&v.low<trio[i].low)?'MÁXIMOS Y MÍNIMOS MÁS BAJOS':trio.slice(1).every((v,i)=>v.high>trio[i].high&&v.low>trio[i].low)?'MÁXIMOS Y MÍNIMOS MÁS ALTOS':'ESTRUCTURA RECIENTE MIXTA';
 const low=Math.min(...bars.map(b=>b.low)),high=Math.max(...bars.map(b=>b.high));
 out.levels=spot<low?`BAJO MÍNIMO RECIENTE $${price(low)} · siguiente soporte pendiente`:spot>high?`SOBRE MÁXIMO RECIENTE $${price(high)} · siguiente resistencia pendiente`:`Rango reciente $${price(low)} — $${price(high)}`;
 const minutes=(tick.tradeMs!-old.timestampMs-60000)/60000;
 out.change=`${delta>=0?'+':''}${price(delta)} puntos (${delta>=0?'+':''}${(delta/old.close*100).toFixed(2)}%) en ${minutes.toFixed(1)} min`;
 const w=s.flow?.intraday_windows?.find(w=>w.minutes===1);
 if(s.options.phase==='streaming'&&recent(s.options.lastEventMs,now,15000)&&w&&w.event_count>0&&w.premium_complete&&typeof w.bullish_pct==='number'&&typeof w.bearish_pct==='number'&&Number.isFinite(w.bullish_pct)&&Number.isFinite(w.bearish_pct)&&w.bullish_pct>=0&&w.bearish_pct>=0&&w.bullish_pct+w.bearish_pct<=100.01){
  const net=w.bullish_pct-w.bearish_pct;
  const current=counter?-direction:direction;
  out.flow=Math.abs(net)<15?'Opciones 1m: flujo mixto':current===0?'Opciones 1m: sesgo '+(net>0?'alcista':'bajista'):net*current>0?'Opciones 1m: acompañan el movimiento observado':'Opciones 1m: contradicen el movimiento observado';
 }
 return out;
}
export function priceNowPanel(r:PriceNow):string {
 return `<section class="panel intraday-control control-${r.tone}" aria-label="Precio en este momento"><header><span class="control-kicker">PRECIO EN ESTE MOMENTO</span></header><h2>${esc(r.headline)}</h2><p>${esc(r.at)} · ${esc(r.change)}</p><p>${esc(r.pace)} · ${esc(r.structure)}</p><p>${esc(r.vwap)}</p><p>${esc(r.levels)}</p><p>${esc(r.flow)}</p><small>Último trade frente a velas cerradas de 1 min. El movimiento actual puede cambiar antes del cierre; no confirma una entrada.</small></section>`;
}
