import {qqqConfirmation} from './qqq-confirmation';
import type {LiveState} from './live';
import type {ControlReading} from './intraday-control';
import type {DirectionScore} from './direction-scanner';
import type {DirectionLevels} from './direction-levels';
import {structuralCards,structuralAlignment} from './structural-map-preview';
import {absorptionReading} from './absorption-reading';
export interface DealerQuickLink {vanna?:import('./vanna-engine').VannaPair;symbol:string;at:number;call:number|null;side:DirectionScore['side'];headline:string;detail:string;rows:string[]}
/** Reuses Snipers readings; never blends overlapping horizons into a probability. */
export function dealerQuickLink(s:LiveState|undefined,r:ControlReading,d:DirectionScore,levels:DirectionLevels,active:boolean,now:number):DealerQuickLink {
 const own=!!(active&&s?.symbol===r.symbol&&Number.isFinite(s.publishedMs)&&s.publishedMs<=now&&now-s.publishedMs<=3000);
 const valid=own&&d.call!=null&&d.put!=null&&Number.isFinite(d.call)&&Number.isFinite(d.put)&&d.call>=0&&d.call<=100&&d.put>=0&&d.put<=100&&Math.abs(d.call+d.put-100)<.01;
 const price=own&&r.priceNow?.ready?r.priceNow:undefined;
 const sign=d.side==='CALL'?1:d.side==='PUT'?-1:0;
 const contrary=!!(valid&&price&&sign&&price.direction!=null&&price.direction*sign<0);
 const cvdContrary=valid&&d.cvd?.status==='contradicts';
 const basket=qqqConfirmation(s,r.symbol,active,d.side,now);
 const headline=!valid?'CONFLUENCIA PENDIENTE':contrary||cvdContrary||basket?.status==='contradicts'?'DIVERGENCIA · '+d.side:d.side+' · '+d.state;
 const cards=structuralCards(s,r.symbol,own,now,r.atr),alignment=structuralAlignment(cards);
 const absorption=absorptionReading(r,now);
 return {vanna:r.vannaEngine?.dealers,symbol:r.symbol,at:now,call:valid?d.call:null,side:valid?d.side:'PENDIENTE',headline,detail:!own?'Esperando publicación vigente de Snipers.':`${price?.headline??'Precio comparable pendiente'} · ${d.cvd?.label??'CVD pendiente'}${basket?' · '+basket.label:''}`,
 rows:own?[...(basket?[basket.detail]:[]),`Mapa de niveles: ${levels.summary}. ${levels.detail}`,`Estructura: ${alignment.label}. ${alignment.detail}`,...cards.map(c=>`${c.name}: ${c.side}${c.call==null?'':` · CALL ${c.call}% / PUT ${c.put}%`}. ${c.detail}`),`Absorción: ${absorption.headline}. ${absorption.detail}`,...(r.strikeReading?.rows??[]).map(l=>`${l.name} ${l.strike}: Δ CALL ${l.call??'pendiente'} / Δ PUT ${l.put??'pendiente'}; fuerza ${l.strength??'pendiente'}; ${l.pressure}.`),r.atr?.context?.detail??'ATR pendiente.',contrary?'El precio va contra la confluencia; el porcentaje es intensidad del modelo, no confirmación de CALL/PUT.':cvdContrary?'El CVD contradice la confluencia; confirmación pendiente.':'Confirmar con reacción persistente, flujo y vigencia.']:[]};
}
