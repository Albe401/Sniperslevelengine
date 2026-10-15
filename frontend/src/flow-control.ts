import type {LiveState} from './live';
import {interpretationFresh} from './confirmations';
import {classifiedPremiumPercent} from './flow-classification';
import type {PriceNow} from './price-now';
export function flowControl(s:LiveState|undefined,symbol:string,active:boolean,p:PriceNow,now=Date.now()) {
 const pending={quality:'waiting' as 'waiting'|'partial'|'ready',sourceKey:'',headline:'CONTROL SIN CONFIRMACIÓN',tone:'waiting',change:'Esperando flujo clasificado y cobertura',evidence:'Falta evidencia para atribuir ventaja a compradores o vendedores'};
 const t=s?.options.lastEventMs;
 if(!active||s?.symbol!==symbol||!interpretationFresh(s.publishedMs,now)||s.options.phase!=='streaming'||t==null||t>now+1000||now-t>15000)return pending;
 const windows=s.flow.intraday_windows??[];
 const valid=(minutes:number)=>{
  const w=windows.find(w=>w.minutes===minutes);if(!w||!Number.isInteger(w.event_count)||w.event_count<=0)return;
  const amounts=['CALL BUY','PUT SELL','PUT BUY','CALL SELL','UNKNOWN'].map(k=>w.buckets?.[k]?.premium);
  if(amounts.some(n=>n==null||!Number.isFinite(n)||n<0))return;
  const [cb,ps,pb,cs,unknown]=amounts as number[];
  const bull=cb+ps,bear=pb+cs,total=bull+bear+unknown;
  if(!Number.isFinite(total)||total<=0||bull+bear<=0)return;
  const coverage=w.premium_complete?classifiedPremiumPercent(w,true):(bull+bear)/total*100;
  if(coverage==null||coverage<50)return;
  return {net:(bull-bear)/(bull+bear)*100,coverage,w,partial:!w.premium_complete||(Math.abs((bull-bear)/(bull+bear))>=.15?Math.abs(bull-bear)<=unknown:unknown/total>.15),bull:bull/(bull+bear)*100,bear:bear/(bull+bear)*100};
 };
 const one=valid(1),five=valid(5),current=one??five;
 if(!current)return pending;
 const dir=current.net>=15?1:current.net<=-15?-1:0;
 const who=dir>0?'COMPRADORES':'VENDEDORES';
 const delta=one&&five?one.net-five.net:null;
 const change=!dir?'PRESIÓN MIXTA':delta==null?'Sin comparación 1m / 5m':delta*dir>=10?`${who} GANAN FUERZA`:delta*dir<=-10?`${who} PIERDEN FUERZA`:`VENTAJA ${dir>0?'COMPRADORA':'VENDEDORA'} SOSTENIDA`;
 return {quality:(current.partial?'partial':'ready') as 'waiting'|'partial'|'ready',sourceKey:JSON.stringify([current.w.minutes,current.w.event_count,current.w.buckets]),headline:current.partial?`MUESTRA ${dir>0?'COMPRADORA':dir<0?'VENDEDORA':'MIXTA'} · PARCIAL`:dir?`VENTAJA ${dir>0?'COMPRADORA':'VENDEDORA'} · INFERIDA`:'PRESIÓN MIXTA',tone:dir>0?'buyers':dir<0?'sellers':'balanced',change:change+(current.partial||one?.partial||five?.partial?' · MUESTRA PARCIAL':''),evidence:`Flujo ${current.w.minutes}m: alcista ${current.bull.toFixed(0)}% / bajista ${current.bear.toFixed(0)}% · prima calculable clasificada ${current.coverage.toFixed(0)}%. ${!p.ready?'Precio pendiente':!dir?'Sin ventaja clara':p.direction===dir?'Precio acompaña':p.direction===-dir?'Precio contradice':'Precio lateral'}.${current.partial?' Muestra incompleta o lado desconocido material; no confirma dominio.':''}`};
}
