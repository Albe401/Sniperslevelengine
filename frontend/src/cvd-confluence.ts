import type {LiveState} from './live';
import type {DirectionScore} from './direction-scanner';
export interface CvdConfluence {status:'confirms'|'contradicts'|'pending';label:string;detail:string}
/** Independent confirmation; no extra volume weight in the daily score. */
export function cvdConfluence(s:LiveState|undefined,d:DirectionScore,symbol:string,active:boolean,now:number):CvdConfluence {
 const pending=(detail:string):CvdConfluence=>({status:'pending',label:'CVD · PENDIENTE',detail});
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const parts=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hourCycle:'h23',hour:'2-digit',minute:'2-digit'}).formatToParts(now);
 const minute=Number(parts.find(p=>p.type==='hour')?.value)*60+Number(parts.find(p=>p.type==='minute')?.value);
 const period=minute<570?'premarket':'regular',phase=period==='premarket'?'Premarket':'Sesión';
 const c=s?.cvd,p=c?.[period];
 if(!active||s?.symbol!==symbol||s.stocks?.phase!=='streaming'||s.metrics?.session!==today||!['Esperando primera vela RTH completa','Sesión RTH'].includes(s.metrics?.phase)||minute<240||minute>=960||!Number.isFinite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000)return pending('Sesión o conexión sin vigencia.');
 if(!c||c.symbol!==symbol||c.date!==today||!c.available||!Number.isFinite(c.at)||c.at>now||now-c.at>3000||!p||p.lastMs==null||!Number.isFinite(p.lastMs)||p.lastMs>now+1000||now-p.lastMs>15000)return pending(`${phase}: esperando operaciones CVD vigentes del activo.`);
 const total=p.buy+p.sell+p.unknown,coverage=100*(p.buy+p.sell)/total;
 if(![p.buy,p.sell,p.unknown,p.delta,total].every(Number.isFinite)||Math.min(p.buy,p.sell,p.unknown)<0||total<=0||Math.abs(p.delta-(p.buy-p.sell))>Math.max(1e-6,Math.abs(p.delta)*1e-9)||!Number.isFinite(p.coveragePct)||Math.abs(coverage-p.coveragePct!)>.01||coverage<70)return pending(`${phase}: cobertura o integridad CVD insuficiente.`);
 if(d.call==null||d.put==null||![d.call,d.put].every(Number.isFinite)||Math.abs(d.call+d.put-100)>.01||d.call<0||d.put<0||!['CALL','PUT'].includes(d.side))return pending('El porcentaje del día todavía no tiene un sesgo direccional confirmado.');
 const direction=p.delta>p.unknown?'CALL':-p.delta>p.unknown?'PUT':null;
 if(!direction)return pending(`${phase}: CVD sin predominio que supere el volumen sin clasificar.`);
 const agrees=direction===d.side;
 return {status:agrees?'confirms':'contradicts',label:`CVD · ${agrees?'CONFIRMA':'CONTRADICE'} ${d.side}`,detail:`${phase}: presión ${direction==='CALL'?'compradora':'vendedora'} acumulada del tramo observado · cobertura ${coverage.toFixed(1)}%${p.gaps?' · tramo reiniciado tras reconexión':''}. Se compara con el sesgo ${d.side}; los porcentajes conservan su cálculo actual. No representa una confirmación de entrada ni el historial completo del día.`};
}
