import {validStructuralIndicators,type StructuralIndicators} from './structural-indicators';

/** Only the saved, already connected card is needed; no live preparation pipeline. */
interface IntradayDirectionInput {
 symbol:string;publishedMs:number|null;asOfMs:number;
 card?:Readonly<{interval:number;side:string;endMs:number|null;bars:number;indicators?:StructuralIndicators}>;
}
export interface IntradayDirectionReading {
 version:'intraday-direction-v1';interval:15;symbol:string;
 availability:'reference'|'pending';side:'CALL'|'PUT'|'MIXTA'|'PENDIENTE';
 structure:string|null;closedAtMs:number|null;asOfMs:number|null;
 relation:'aligned'|'opposed'|'mixed'|'pending';livePriceReady:false;reason:string;
 triggerConfirmed:false;probability:null;executionEnabled:false;
}
const clock=(value:unknown):value is number=>typeof value==='number'&&Number.isSafeInteger(value)&&value>0&&value<=8640000000000000;

/** Reuse the frozen 15m technical reading. A snapshot never becomes a current signal. */
export function buildIntradayDirection(input:Readonly<IntradayDirectionInput>):IntradayDirectionReading {
 const c=input.card;
 const out:IntradayDirectionReading={version:'intraday-direction-v1',interval:15,symbol:input.symbol,availability:'pending',side:'PENDIENTE',structure:c?.side??null,closedAtMs:null,asOfMs:clock(input.asOfMs)?input.asOfMs:null,relation:'pending',livePriceReady:false,reason:'Esperando una dirección técnica verificable en la captura guardada de 15m.',triggerConfirmed:false,probability:null,executionEnabled:false};
 if(!c||c.interval!==15||!['CALL','PUT','MIXTA'].includes(c.side)||!clock(input.asOfMs)||!clock(input.publishedMs)||input.publishedMs>input.asOfMs||!clock(c.endMs)||c.endMs>input.publishedMs||!validStructuralIndicators(c.indicators,c.endMs,c.bars)||!c.indicators!.ready)return out;
 out.availability='reference';out.side=c.indicators!.trend as 'CALL'|'PUT'|'MIXTA';out.closedAtMs=c.endMs;
 out.relation=c.side==='MIXTA'||out.side==='MIXTA'?'mixed':c.side===out.side?'aligned':'opposed';
 out.reason='Referencia técnica histórica del cierre guardado de 15m. Estructura '+c.side+'. Lectura exclusiva del cierre guardado; no acredita un precio actual. No confirma una entrada.';
 return out;
}

export function intradayDirectionLabel(reading:Readonly<IntradayDirectionReading>|undefined):string {
 return 'REF. '+(reading?.side??'PENDIENTE')+' · 15m';
}
