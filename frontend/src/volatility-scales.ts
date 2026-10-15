import type {ControlReading} from './intraday-control';
export function volatilityScales(r:ControlReading,now:number):{label:string;detail:string} {
 const c=r.expectedMoveContext,move=r.expectedMove,d=r.atr?.daily;
 const positive=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v)&&v>0;
 const date=(at:number)=>new Date(at).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const valid=!!c&&Number.isFinite(now)&&Number.isFinite(c.at)&&c.at<=now&&now-c.at<=900000&&date(c.at)===date(now)&&positive(move)&&positive(c.spot)&&!!c.expiration&&c.expiration>=date(now)&&['ATM_IV_1D','ATM_STRADDLE_DAILY_EQUIVALENT_FALLBACK'].includes(c.method??'');
 const label=valid?(c!.method==='ATM_IV_1D'?'Mov. IV 1 día':'Mov. prima 1 día')+' · ±$'+move!.toFixed(2):'Mov. opciones · —';
 const estimate=valid?`${c!.method==='ATM_IV_1D'?'IV ATM anualizada: precio × IV / √252':'Alternativa de prima ATM: call + put del mismo strike / √sesiones hasta vencimiento; aproximación, no una desviación estándar validada'}. Distancia ±$${move!.toFixed(2)} alrededor de referencia $${c!.spot!.toFixed(2)}; ancho total de banda $${(2*move!).toFixed(2)}. Consulta ${new Date(c!.at).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false})} ET · vencimiento ${c!.expiration} · calidad ${c!.quality}. La consulta reciente no acredita la edad de cada IV o precio del proveedor.`:'Movimiento de opciones pendiente: requiere método, referencia y consulta vigente del mismo día.';
 const historical=d?.symbol===r.symbol&&positive(d.value)?`ATR14 diario histórico $${d.value.toFixed(2)}, hasta ${d.through}: media simple de 14 true ranges, calculados con 15 sesiones cerradas y precios ajustados por splits. True range = máximo de alto−bajo, |alto−cierre previo| y |bajo−cierre previo|; incluye gaps.`:'ATR diario histórico pendiente.';
 return {label,detail:estimate+' '+historical+' Son medidas distintas: movimiento estimado por lado frente a rango histórico completo. No deben igualarse, sumarse ni promediarse; su cociente no demuestra una prima de volatilidad. Ninguna es rango restante, límite del día, dirección o agotamiento confirmado.'};
}
