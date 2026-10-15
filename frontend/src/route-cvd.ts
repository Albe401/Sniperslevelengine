import type {LiveState} from './live';
import type {RouteResult} from './route-engine';
export interface RouteCvdSupport {status:'pending'|'observing'|'aligned'|'opposed';label:string;detail:string;at:number|null;coverage:number|null;observations:number}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const date=(t:number)=>new Date(t).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
/** Context only: never writes RouteResult, elects a side, sets a target or admits an order. */
export class RouteCvdTracker {
 private identity='';private lastBar=0;private sign=0;private count=0;
 reset(){this.identity='';this.lastBar=0;this.sign=0;this.count=0;}
 read(s:LiveState|undefined,r:RouteResult,symbol:string,active:boolean,barMs:number|undefined,now:number):RouteCvdSupport {
  const pending=(detail:string):RouteCvdSupport=>{this.reset();return {status:'pending',label:'Pendiente',detail,at:null,coverage:null,observations:0};};
  if(!active||s?.symbol!==symbol||s.stocks?.phase!=='streaming'||!finite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000||s.metrics?.session!==date(now)||s.metrics?.phase!=='Sesión RTH')return pending('Conexión o sesión regular sin vigencia.');
  if(!['test','sustained'].includes(r.state)||![1,-1].includes(r.direction)||!r.key||!finite(barMs)||barMs+60000>now||now-barMs-60000>90000||date(barMs)!==date(now))return pending('Esperando prueba de precio y cierres comparables.');
  const c=s.cvd,p=c?.regular,w=c?.recentFive;
  if(!c||c.symbol!==symbol||c.date!==date(now)||!c.available||!finite(c.at)||c.at>now||now-c.at>3000||!p||!finite(p.lastMs)||p.lastMs>now||now-p.lastMs>15000||!finite(p.sinceMs)||!Number.isInteger(p.gaps)||p.gaps<0||!w||!w.complete)return pending('CVD reciente incompleto o sin operaciones vigentes.');
  if(![w.startMs,w.endMs].every(finite)||w.endMs>now||now-w.endMs>3000||w.endMs-w.startMs!==300000||p.sinceMs>w.startMs||date(w.startMs)!==c.date)return pending('Ventana CVD no comparable o reiniciada.');
  const parts=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hourCycle:'h23',hour:'2-digit',minute:'2-digit'}).formatToParts(w.startMs);
  const minute=Number(parts.find(x=>x.type==='hour')?.value)*60+Number(parts.find(x=>x.type==='minute')?.value);
  const endParts=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hourCycle:'h23',hour:'2-digit',minute:'2-digit'}).formatToParts(now);
  const endMinute=Number(endParts.find(x=>x.type==='hour')?.value)*60+Number(endParts.find(x=>x.type==='minute')?.value);
  if(minute<570||endMinute>=960)return pending('No mezclar premarket y sesión regular.');
  const total=w.buy+w.sell+w.unknown,coverage=100*(w.buy+w.sell)/total;
  if(![w.buy,w.sell,w.unknown,w.delta,total,coverage].every(finite)||Math.min(w.buy,w.sell,w.unknown)<0||total<=0||coverage<70||coverage>100||Math.abs(w.delta-w.buy+w.sell)>Math.max(1e-6,Math.abs(w.delta)*1e-9))return pending('Integridad o cobertura CVD reciente insuficiente.');
  const sign=w.delta>w.unknown?1:-w.delta>w.unknown?-1:0;
  if(!sign)return pending('La presión reciente no supera el volumen desconocido.');
  const identity=[symbol,c.date,r.key,r.direction,p.sinceMs,p.gaps].join(':');
  if(identity!==this.identity){this.reset();this.identity=identity;}
  if(this.lastBar&&barMs<this.lastBar)return pending('Cierre anterior: no usar evidencia posterior.');
  if(barMs===this.lastBar){if(sign!==this.sign){this.sign=sign;this.count=0;}}
  else{this.count=sign===this.sign&&barMs-this.lastBar===60000?this.count+1:1;this.lastBar=barMs;this.sign=sign;}
  const status=this.count<2?'observing':sign===r.direction?'aligned':'opposed';
  return {status,label:status==='observing'?'Observando':status==='aligned'?'Acompaña':'En contra',detail:'CVD reciente 5m de acciones · '+(status==='observing'?'falta otro cierre distinto':status==='aligned'?'presión acompaña la ruta del precio':'presión contraria; observar resistencia del precio, sin confirmar absorción ni elegir un giro')+' · contexto experimental, sin modificar dirección, objetivos o ejecución.',at:w.endMs,coverage,observations:this.count};
 }
}
