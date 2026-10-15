import {it,expect} from 'vitest';
import {readStrikeAbsorption,strikeAbsorptionRows,strikeAbsorptionDetails} from './strike-absorption';
import type {LiveState} from './live';
const now=Date.UTC(2026,9,1,15);
const fixture=()=>({symbol:'QQQ',publishedMs:now,optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:['2026-10-01'],structure:{complete:true,callWall:{strike:745},putWall:{strike:730},selected:{M1:742},routes:[]}}]},levelDeltas:[{ticker:'C',strike:745,kind:'call',expiration:'2026-10-01',delta:.52,observedMs:now},{ticker:'P',strike:745,kind:'put',expiration:'2026-10-01',delta:-.48,observedMs:now}]} as unknown as LiveState);
const dynamic=()=>({symbol:'QQQ',at:now,available:true,levels:[{name:'CALL WALL',strike:745,provisional:false,change:8,strength:75,flowComplete:true,classifiedPct:85,callBuy:200,callSell:20,putBuy:10,putSell:30}]} as unknown as NonNullable<LiveState['levelDynamics']>);
it('explains genuine insufficient coverage without manufacturing a score',()=>{const s=fixture();s.levelDynamics=dynamic();s.levelDynamics.levels[0].strength=null;s.levelDynamics.levels[0].coverage=55;const reading=readStrikeAbsorption(s,'QQQ',true,now);expect(reading.rows[0].forceReason).toContain('55/100; mínimo 60');const html=strikeAbsorptionRows({strikeReading:reading} as never);expect(html).toContain('title="Cobertura 55');expect(html).toContain('Fuerza pendiente');});
it('benchmarks compact delta/strength rendering',()=>{const s=fixture();s.levelDynamics=dynamic();const start=performance.now();for(let i=0;i<1000;i++)strikeAbsorptionRows({strikeReading:readStrikeAbsorption(s,'QQQ',true,now)} as never);const ms=(performance.now()-start)/1000;console.info(`Delta por strike: ${ms.toFixed(3)} ms/update`);expect(ms).toBeLessThan(10);});
it('never joins delta from an older snapshot to newer structural strikes',()=>{
 const s=fixture();s.levelDeltas![0].observedMs--;expect(readStrikeAbsorption(s,'QQQ',true,now).rows[0].call).toBeNull();
 s.optionSnapshotStatus.horizons[0].observedMs--;expect(readStrikeAbsorption(s,'QQQ',true,now).rows).toEqual([]);
});
it.each(['negative','overcoverage','strength','next-expiry','old','future','foreign','partial'])('withholds unsupported live strength or pressure: %s',kind=>{
 const s=fixture();s.levelDynamics=dynamic();const d=s.levelDynamics.levels[0];
 if(kind==='negative')d.callBuy=-200;
 if(kind==='overcoverage')d.classifiedPct=101;
 if(kind==='strength')d.strength=101;
 if(kind==='next-expiry'){s.optionSnapshotStatus.horizons[0].expirations=['2026-10-02'];s.levelDeltas!.forEach(c=>c.expiration='2026-10-02');}
 if(kind==='old')s.levelDynamics.at=now-3001;
 if(kind==='future')s.levelDynamics.at=now+1;
 if(kind==='foreign')s.levelDynamics.symbol='SPY';
 if(kind==='partial')d.flowComplete=false;
 const row=readStrikeAbsorption(s,'QQQ',true,now).rows[0];
 if(kind==='strength')expect(row.strength).toBeNull();else expect(row.pressure).toContain('pendiente');
});
it('uses directional option premiums only with sufficient classification',()=>{const s=fixture();s.levelDynamics=dynamic();expect(readStrikeAbsorption(s,'QQQ',true,now).rows[0].pressure).toContain('alcista');s.levelDynamics.levels[0].putBuy=500;expect(readStrikeAbsorption(s,'QQQ',true,now).rows[0].pressure).toContain('bajista');s.levelDynamics.levels[0].classifiedPct=69.9;expect(readStrikeAbsorption(s,'QQQ',true,now).rows[0].pressure).toContain('pendiente');});
it('does not interpret increasing delta as increasing importance',()=>{const s=fixture();s.levelDeltas![0].delta=.95;expect(readStrikeAbsorption(s,'QQQ',true,now).rows[0].change).toBeNull();s.levelDynamics=dynamic();s.levelDynamics.levels[0].change=-8;expect(readStrikeAbsorption(s,'QQQ',true,now).rows[0].change).toBe(-8);});
it.each([NaN,Infinity,1.01,-1.01])('rejects invalid delta %s',delta=>{const s=fixture();s.levelDeltas![0].delta=delta;expect(readStrikeAbsorption(s,'QQQ',true,now).rows[0].call).toBeNull();});
it('accepts observed zero delta and zero strength without filling missing fields',()=>{const s=fixture();s.levelDeltas![0].delta=0;s.levelDynamics=dynamic();s.levelDynamics.levels[0].strength=0;const row=readStrikeAbsorption(s,'QQQ',true,now).rows[0];expect(row.call).toBe(0);expect(row.strength).toBe(0);});
it('expires after crossing the New York session date',()=>{const s=fixture();const midnight=Date.UTC(2026,9,2,4,1);s.publishedMs=midnight;s.optionSnapshotStatus.observedMs=midnight;s.optionSnapshotStatus.horizons[0].observedMs=midnight;s.levelDeltas![0].observedMs=midnight-120000;expect(readStrikeAbsorption(s,'QQQ',true,midnight).rows[0].call).toBeNull();});
it('pairs exact strike and expiry without replacing missing deltas',()=>{const r=readStrikeAbsorption(fixture(),'QQQ',true,now);expect(r.rows[0].call).toBe(.52);expect(r.rows[0].put).toBe(-.48);expect(r.rows[1].call).toBeNull();expect(r.rows[0].change).toBeNull();});
it.each(['expiry','future','stale','sign','duplicate'])('withholds invalid or ambiguous delta: %s',kind=>{const s=fixture();const c=s.levelDeltas![0];if(kind==='expiry')c.expiration='2026-10-02';if(kind==='future')c.observedMs=now+1;if(kind==='stale')c.observedMs=now-900001;if(kind==='sign')c.delta=-.5;if(kind==='duplicate')s.levelDeltas!.push({...c,ticker:'C2'});expect(readStrikeAbsorption(s,'QQQ',true,now).rows[0].call).toBeNull();});
it('rejects disconnected foreign and stale publications',()=>{const s=fixture();expect(readStrikeAbsorption(s,'QQQ',false,now).rows).toEqual([]);expect(readStrikeAbsorption(s,'SPY',true,now).rows).toEqual([]);s.publishedMs=now-60000;expect(readStrikeAbsorption(s,'QQQ',true,now).rows).toEqual([]);});
it('escapes level labels and renders absent evidence as pending',()=>{const html=strikeAbsorptionRows({strikeReading:{rows:[{name:'<script>',strike:745,expiration:'2026-10-01',call:null,put:null,at:null,strength:null,change:null,pressure:'Pendiente',partial:true}]}} as never);expect(html).toContain('&lt;script&gt;');expect(html).toContain('Fuerza pendiente');expect(html).not.toContain('Cambio pendiente');expect(strikeAbsorptionDetails({strikeReading:{rows:[{name:'<script>',strike:745,expiration:'2026-10-01',call:null,put:null,at:null,strength:null,change:null,pressure:'Pendiente',partial:true}]}} as never)).toContain('Cambio pendiente');expect(html).toContain('Δ CALL <b>—');});
