import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
/** Interpretation only. Pressure does not rewrite the observed price sequence or elect a target. */
export function assessRoute(r:RouteResult,market:MarketEvidence,now:number){
 const fresh=Number.isFinite(now)&&Number.isFinite(market.at)&&market.at<=now&&now-market.at<=3000&&market.price?.ready;
 const live=fresh&&Number.isFinite(r.at)&&r.at<=now&&now-r.at<=90000;
 const side=r.direction>0?'alcista ↑':'bajista ↓';
 if(!live||r.state==='waiting')return {quality:'pending' as const,label:'Esperando datos vigentes',detail:'Sin escenario activo con evidencia actual.'};
 if(r.state==='invalidated')return {quality:'pending' as const,label:'Ruta invalidada · esperar nueva prueba',detail:r.reason};
 if(r.state==='reached')return {quality:'pending' as const,label:'Referencia alcanzada · esperar nueva prueba',detail:r.reason};
 if(r.state==='range'||![1,-1].includes(r.direction))return {quality:'pending' as const,label:'Sin activación · esperar prueba del nivel',detail:'El sesgo CALL/PUT no sustituye una secuencia de precio.'};
 const comparable=(at:number|null)=>at!=null&&Number.isFinite(at)&&at<=now&&now-at<=3000;
 const flow=comparable(market.flow?.at??null)?market.flow.signal:null,cvd=comparable(market.cvd?.at??null)?market.cvd.signal:null;
 const opposed=[flow,cvd].some(v=>v!=null&&Number.isFinite(v)&&v*r.direction<0)||market.direction*r.direction<0;
 const aligned=!opposed&&market.direction===r.direction&&flow!=null&&cvd!=null&&flow*r.direction>0&&cvd*r.direction>0;
 const quality=opposed?'opposed' as const:aligned?'aligned' as const:'partial' as const;
 const phase=r.state==='sustained'?`Continuación ${side} observada`:r.state==='test'?`Prueba ${side} · falta continuación`:`Candidato ${side} · falta prueba`;
 const support=opposed?'presión en contra':aligned?'flujo y CVD acompañan':'apoyo incompleto';
 return {quality,label:`${phase} · ${support}`,detail:opposed?'El precio avanzó, pero la presión discrepa. No confirma absorción ni un giro; esperar nueva evidencia.':aligned?'Hay coincidencia de lecturas observadas; no es probabilidad de llegada ni predicción de la próxima vela.':'La secuencia de precio es observable, pero falta apoyo direccional comparable de flujo/CVD.'};
}
