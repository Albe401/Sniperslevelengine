import {it,expect} from 'vitest';
import {volatilityReading,volatilityReadingPanel,intradayPressure} from './volatility-reading';
import type {ControlReading} from './intraday-control';
const now=Date.parse('2026-09-30T15:00:00Z');
it('moves proportionally with price and flow independently of IV confirmation',()=>{
 const r={pricePressure:.25,components:[{name:'FLOW 1M',signal:.5,weight:24}],flowControl:{quality:'ready'}} as ControlReading;
 expect(intradayPressure(r).value).toBeCloseTo(.4);
 expect(volatilityReadingPanel(r,now)).toContain('left:70%');
 r.pricePressure=-.5;r.components[0].signal=-.5;
 expect(intradayPressure(r).value).toBeCloseTo(-.5);
 expect(volatilityReadingPanel(r,now)).toContain('left:25%');
 r.pricePressure=undefined;r.components=[];
 expect(intradayPressure(r)).toMatchObject({available:false,value:0});
});
it('reports IV trend while price candles are pending without selecting CALL or PUT',()=>{
 const r=fixture();r.priceNow!.ready=false;
 expect(volatilityReading(r,now)).toMatchObject({tone:'neutral',headline:'VOLATILIDAD SUBIENDO · DIRECCIÓN SIN CONFIRMAR'});
 r.iv!.trend='CONTRAYENDO';expect(volatilityReading(r,now).headline).toContain('CONTRAYENDO');
 r.iv!.trend='ESTABLE';expect(volatilityReading(r,now).headline).toContain('ESTABLE');
 r.iv!.delta=null;expect(volatilityReading(r,now).detail).toContain('segunda observación');
 expect(volatilityReadingPanel(r,now)).toContain('title=');
});
function fixture(direction=1):ControlReading{return {pricePressure:direction,ivObservation:{key:'QQQ',at:now,percent:25},iv:{percent:25,delta:1,trend:'EXPANDIENDO',tone:'yellow',context:''},priceNow:{ready:true,direction,spot:500,structure:direction===1?'MÁXIMOS Y MÍNIMOS MÁS ALTOS':'MÁXIMOS Y MÍNIMOS MÁS BAJOS',vwap:direction===1?'SOBRE VWAP':'BAJO VWAP',pace:'ACELERANDO EN VELAS CERRADAS'},flowControl:{quality:'ready',tone:direction===1?'buyers':'sellers',headline:'VENTAJA'}} as ControlReading;}
it('requires expanding IV and aligned price structure, VWAP and classified flow',()=>{
 expect(volatilityReading(fixture(),now).headline).toContain('FAVORECE CALL');expect(volatilityReading(fixture(-1),now).headline).toContain('FAVORECE PUT');
 for(const change of [{tone:'sellers'},{quality:'partial'}]){const r=fixture();Object.assign(r.flowControl!,change);expect(volatilityReading(r,now).headline).toContain('SIN CONFIRMAR');}
 for(const change of [{vwap:'BAJO VWAP'},{pace:'PERDIENDO VELOCIDAD EN VELAS CERRADAS'},{structure:'ESTRUCTURA RECIENTE MIXTA'},{direction:0}]){const r=fixture();Object.assign(r.priceNow!,change);expect(volatilityReading(r,now).headline).toContain('SIN CONFIRMAR');}
});
it('does not preserve directional readings with stale or absent evidence',()=>{
 for(const at of [now-900001,now+1,now-86400000]){const r=fixture();r.ivObservation!.at=at;expect(volatilityReading(r,now).tone).toBe('waiting');}
 const r=fixture();r.iv!.delta=null;expect(volatilityReading(r,now).tone).toBe('waiting');r.iv=undefined;expect(volatilityReading(r,now).tone).toBe('waiting');
});
it('keeps a comparable IV reading between scheduled five-minute snapshot refreshes',()=>{
 const r=fixture();for(const age of [180001,300000,360000,900000]){r.ivObservation!.at=now-age;expect(volatilityReading(r,now).tone).toBe('call');}
});
it('distinguishes stable and contracting IV without selecting a direction',()=>{const r=fixture();for(const trend of ['ESTABLE','CONTRAYENDO']){r.iv!.trend=trend;expect(volatilityReading(r,now).headline).toContain('ESPERAR');}});
it('renders a directional bar with visible IV, change and status',()=>{
 for(const [direction,value,tone] of [[1,1,'call'],[-1,-1,'put']] as const){const html=volatilityReadingPanel(fixture(direction),now);expect(html).toContain(`volatility-${tone}`);expect(html).toContain(`aria-valuenow="${value}"`);expect(html).toContain('IV ATM 25.00 %');expect(html).toContain('cambio +1.00 puntos de IV');expect(html).toContain('FAVORECE');}
 const r=fixture();r.iv!.trend='ESTABLE';expect(volatilityReadingPanel(r,now)).toContain('aria-valuenow="1"');r.iv=undefined;expect(volatilityReadingPanel(r,now)).toContain('volatility-call');
});
