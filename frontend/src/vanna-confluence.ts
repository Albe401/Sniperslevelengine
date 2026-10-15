import type {LiveState} from './live';
import type {DirectionScore} from './direction-scanner';
export interface VannaRead {status:'pending'|'gain'|'loss'|'mixed'|'stable';label:string;detail:string;meanDelta:number|null;count:number}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const date=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const record=(v:unknown):Record<string,unknown>|undefined=>v!=null&&typeof v==='object'?v as Record<string,unknown>:undefined;
/** Sensitivity of sampled long options, never inferred dealer inventory or order flow. */
export function vannaConfluence(s:LiveState|undefined,d:DirectionScore,symbol:string,active:boolean,now:number,scope?:{expirations:string[];strike?:number}):VannaRead {
 const pending=(reason:string):VannaRead=>({status:'pending',label:'VANNA · PENDIENTE',detail:reason+' No altera el porcentaje CALL/PUT.',meanDelta:null,count:0});
 const today=date.format(now),expirations=scope?.expirations??[today],session=s?.chartHistory?.sessions.find(x=>x.date===today),g=s?.greekSupport;
 if(!active||s?.symbol!==symbol||s.stocks?.phase!=='streaming'||s.options?.phase!=='streaming'||!finite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000||!session||!finite(session.openMs)||!finite(session.closeMs)||session.openMs>=session.closeMs||now<session.openMs||now>=session.closeMs||s.metrics.session!==today||s.metrics.phase!=='Sesión RTH')return pending('Requiere sesión regular abierta, calendario y publicación vigentes.');
 if(!expirations.length||expirations.some(x=>!/^\d{4}-\d{2}-\d{2}$/.test(x)||!Number.isFinite(Date.parse(x+'T12:00:00Z'))||new Date(x+'T12:00:00Z').toISOString().slice(0,10)!==x||x<today)||new Set(expirations).size!==expirations.length||scope?.strike!=null&&(!finite(scope.strike)||scope.strike<=0))return pending('Ámbito de vencimientos o strike inválido.');
 if(d.call==null||d.put==null||![d.call,d.put].every(finite)||d.call<0||d.put<0||d.call>100||d.put>100||Math.abs(d.call+d.put-100)>.01||!['CALL','PUT'].includes(d.side))return pending('Sin confluencia direccional verificable.');
 if(!g?.available||g.symbol!==symbol||!finite(g.at)||g.at>now||now-g.at>3000||!Array.isArray(g.contracts))return pending('Apoyo de griegas ausente o caducado.');
 const kind=d.side==='CALL'?'call':'put',sign=kind==='call'?1:-1,seen=new Set<string>(),effects:number[]=[];
 for(const raw of g.contracts){const row=record(raw),c=record(row?.contract);if(!row||!c||c.kind!==kind||typeof c.expiration!=='string'||!expirations.includes(c.expiration)||scope?.strike!=null&&c.strike!==scope.strike)continue;
 if(typeof c.ticker!=='string'||seen.has(c.ticker))return pending('Contratos duplicados o identidad inválida en la muestra.');seen.add(c.ticker);
 const va=row.vannaDeltaIvPoint,iv=row.ivChangePoints,prev=row.previousMs,at=c.observedMs,quote=c.quoteMs;
 if(!finite(va)||!finite(iv)||Math.abs(iv)>5||!finite(prev)||prev<session.openMs||!finite(at)||at<=prev||at>now||now-at>900000||at-prev>900000||date.format(at)!==today||c.recent!==true||!finite(quote)||quote<session.openMs||quote>now||now-quote>60000)continue;
 const effect=va*iv*sign;if(finite(effect)&&Math.abs(effect)<=1)effects.push(effect);
 }
 if(!effects.length)return pending('Esperando dos observaciones comparables de IV y vanna del mismo contrato y vencimiento.');
 const mean=effects.reduce((a,b)=>a+b,0)/effects.length,gain=effects.some(x=>x>1e-4),loss=effects.some(x=>x< -1e-4);
 const status=gain&&loss?'mixed':Math.abs(mean)<=1e-4?'stable':mean>0?'gain':'loss';
 const label=`VANNA · ${d.side} ${status==='gain'?'GANA SENSIBILIDAD':status==='loss'?'PIERDE SENSIBILIDAD':status==='mixed'?'MUESTRA MIXTA':'SIN CAMBIO MATERIAL'}`;
 return {status,label,count:effects.length,meanDelta:mean,detail:`${effects.length} contratos ${d.side} comparables en ${expirations.join(" / ")}${scope?.strike!=null?" · strike "+scope.strike:""}. Cambio local estimado en |delta| por IV: ${mean>=0?'+':''}${mean.toFixed(5)}, media simple por contrato, sin ponderación por OI. Vanna por punto de IV × cambio observado de IV; signo invertido para medir |delta| de PUT. Muestra parcial de opciones largas; no indica dirección futura del activo, flujo ni coberturas reales de dealers. Aproximación BSM europeo (r=4%, q=0,5%); cambios de IV >5 puntos excluidos y banda descriptiva de delta ±0,0001. No modifica porcentajes ni confirma entrada. Fuente conceptual: Options Industry Council / OCC, Options Glossary (Vanna).`};
}
