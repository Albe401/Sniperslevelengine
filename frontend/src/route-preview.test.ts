import {it,expect} from 'vitest';
import {routeData,routePreview} from './route-preview';
import type {LiveState} from './live';
import type {ControlReading} from './intraday-control';
const now=Date.UTC(2026,9,2,15);
const state=()=>({symbol:'QQQ',publishedMs:now,options:{phase:'streaming',lastEventMs:now},flow:{intraday_windows:[{minutes:5,event_count:3,contracts:12,premium_complete:false,buckets:{'CALL BUY':{premium:1000},'PUT BUY':{premium:0},UNKNOWN:{premium:600}}}]}} as unknown as LiveState);
const reading=()=>({symbol:'QQQ',priceNow:{ready:true,spot:742,headline:'PRECIO SUBIENDO'},strikeReading:{rows:[]},priceLevels:{important:'CALL WALL 745',route:'Primera referencia 745',reaction:'Defensa pendiente',condition:'Sostener 742'}} as unknown as ControlReading);
it('connects observed premiums and structural references without claiming confirmation',()=>{
 const d=routeData(state(),reading(),true,now);expect(d.flow).toContain('$1,000');expect(d.flow).toContain('PUT BUY $0');expect(d.flowDetail).toContain('$600');expect(d.flowDetail).toContain('parcial');expect(d.target).toContain('745');expect(d.conclusion).toBe('Esperando datos vigentes');expect(d.compact.direction).toBe('Sin activación de precio');
});
it('adds ATR distance to the actual next map reference without selecting another target',()=>{
 const s=state(),r=reading();s.stocks={phase:'streaming'} as LiveState['stocks'];s.ticks={QQQ:{symbol:'QQQ',price:742,tradeMs:now}} as unknown as LiveState['ticks'];
 s.optionSnapshotStatus={phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:['2026-10-02'],structure:{complete:true,callWall:{strike:745,score:80},putWall:{strike:730,score:80},selected:{M1:743},routes:[]}}]} as unknown as LiveState['optionSnapshotStatus'];
 r.priceNow!.direction=1;
 const a={value:2,percent:.27,lastRangeRatio:1,endMs:now,reason:'ATR14 test'};
 const d=routeData(s,r,true,now,a);expect(d.atrNext).toBe('0.50 ATR · 5 min');expect(d.compact.next).toBe('$743.00');
 expect(routePreview(s,r,true,now,a)).toContain('0.50 ATR · 5 min');
});
it.each(['inactive','wrong-symbol','publication','future','expired','disconnected'])('rejects invalid flow context: %s',kind=>{
 const s=state();if(kind==='wrong-symbol')s.symbol='QCOM';if(kind==='publication')s.publishedMs=now-4000;if(kind==='future')s.options.lastEventMs=now+1;if(kind==='expired')s.options.lastEventMs=now-15001;if(kind==='disconnected')s.options.phase='disconnected';
 expect(routeData(s,reading(),kind!=='inactive',now).flow).toBe('CALL BUY — · PUT BUY —');
});
it('zero received executions are missing activity, not zero buying',()=>{const s=state();s.flow.intraday_windows![0].event_count=0;expect(routeData(s,reading(),true,now).flow).toContain('CALL BUY —');});
it('does not fabricate a target from missing price',()=>{const r=reading();r.priceNow!.ready=false;expect(routeData(state(),r,true,now).target).toBe('Esperando precio vigente');});
it('escapes provider content',()=>{const r=reading();r.priceLevels!.important='<img onerror=alert(1)>';expect(routePreview(state(),r,true,now)).toContain('&lt;img');});
it.each([null,NaN,-1])('rejects malformed current price %s',spot=>{const r=reading();r.priceNow!.spot=spot as number;expect(routeData(state(),r,true,now).target).toBe('Esperando precio vigente');});
it('does not associate 0DTE zone premiums with the next-session delta',()=>{
 const s=state(),r=reading();r.strikeReading={rows:[{name:'CALL WALL',strike:745,expiration:'2026-10-05',call:.5,put:-.5,at:now,change:null,strength:null,pressure:'Pendiente',partial:true}]};
 s.levelDynamics={symbol:'QQQ',at:now,available:true,levels:[{name:'CALL WALL',strike:745,premium:9999}]} as LiveState['levelDynamics'];
 expect(routeData(s,r,true,now).rows[0].zone).toBe('Prima por zona pendiente');
});
it('renders three parallel cards and keeps premium delta and absorption behind details',()=>{const html=routePreview(state(),reading(),true,now);const collapsed=html.replace(/<details\b[\s\S]*?<\/details>/g,'');expect(collapsed).toContain('Escenario CALL');expect(collapsed).toContain('Escenario PUT');expect(collapsed).toContain('Proyección histórica');expect(collapsed.match(/class="route-preview-station"/g)).toHaveLength(3);expect(collapsed).not.toContain('CALL BUY');expect(collapsed).not.toContain('Δ C');expect(collapsed).not.toContain('SIN INDICIOS');expect(collapsed).not.toContain('fuerza');expect(html).toContain('CALL BUY');expect(html).toContain('Camino del precio');expect(collapsed).toContain('Proyección pendiente');expect(collapsed).not.toContain('$758.90');});
