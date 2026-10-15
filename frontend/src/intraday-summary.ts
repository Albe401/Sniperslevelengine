import type {PriceNow} from './price-now';
import type {PriceLevels} from './price-levels';
import type {flowControl} from './flow-control';
type Flow=Pick<ReturnType<typeof flowControl>,'tone'|'headline'|'change'|'evidence'>;
/** Advantage requires options evidence; a price move alone never elects a winner. */
export function intradaySummary(p:PriceNow|undefined,_l:PriceLevels|undefined,f:Flow|undefined){
 const direction=f?.tone==='buyers'?1:f?.tone==='sellers'?-1:0;
 const side=direction>0?'COMPRADORES':'VENDEDORES';
 const evidence=f?.evidence??'Flujo de opciones pendiente';
 if(!f||f.tone==='waiting'||!p?.ready)return {headline:'VENTAJA SIN DETERMINAR',tone:'waiting',direction:0,evidence:'Falta evidencia vigente para comparar compradores y vendedores.'};
 if(f.headline.includes('PARCIAL'))return {headline:'COMPRADORES Y VENDEDORES · EVIDENCIA PARCIAL',tone:'waiting',direction:0,evidence:direction?`La muestra favorece a ${side.toLowerCase()}, pero no alcanza para atribuir control.`:'La muestra es incompleta; no permite declarar equilibrio ni un ganador.'};
 if(!direction)return {headline:'COMPRADORES Y VENDEDORES · SIN VENTAJA CLARA',tone:'balanced',direction:0,evidence:'Presión equilibrada. Ningún lado tiene ventaja suficiente en el flujo observado.'};
 if(p.direction===-direction)return {headline:'CONTROL EN DISPUTA · SIN GANADOR',tone:'balanced',direction:0,evidence:`El flujo favorece a ${side.toLowerCase()}, pero encuentra oposición. Ventaja sin confirmar.`};
 if(!p.direction)return {headline:'COMPRADORES Y VENDEDORES · CONTROL EN DISPUTA',tone:'balanced',direction:0,evidence:`Presión de ${side.toLowerCase()} sin imponerse todavía. No hay control direccional confirmado.`};
 return {headline:`${side} TIENEN VENTAJA`,tone:f.tone,direction,evidence:`${f.change}. Ventaja inferida del flujo y su confirmación; no implica control permanente.`};
}
