import {it,expect} from 'vitest';
import {directionLevels,directionLevelsPanel,directionLevelTag} from './direction-levels';
import {combineDirection} from './direction-scanner';
import {dayMap} from './day-map';
import type {LiveState} from './live';
const now=Date.UTC(2026,9,5,15);
const fixture=()=>({symbol:'NVDA',publishedMs:now,stocks:{phase:'streaming'},ticks:{NVDA:{symbol:'NVDA',price:185,tradeMs:now}},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:['2026-10-05'],structure:{complete:true,callWall:{strike:190,score:80},putWall:{strike:180,score:80},selected:{M1:187},routes:[]}}]}} as unknown as LiveState);
const direction=(sign=1)=>({...combineDirection(sign*.8,sign*.8,sign*.5,90),cvd:{status:'confirms' as const,label:'CVD',detail:'test'}});
it('links CALL to lower backing and nearest reference above, PUT mirrors it',()=>{
 const s=fixture(),call=directionLevels(s,direction(),'NVDA',true,now),put=directionLevels(s,direction(-1),'NVDA',true,now);
 expect(call.detail).toContain('PUT WALL 180.00');expect(call.detail).toContain('IMÁN PRINCIPAL 187.00');
 expect(put.detail).toContain('CALL WALL 190.00');expect(put.detail).toContain('PUT WALL 180.00');
 expect(call.rows.find(l=>l.price===187)?.text).toContain('PRÓXIMA REFERENCIA');
});
it('flags contrary CVD and price beyond backing without declaring a break',()=>{
 const s=fixture(),d=direction();d.cvd.status='contradicts' as 'confirms';s.ticks.NVDA.price=179;
 const r=directionLevels(s,d,'NVDA',true,now);expect(r.summary).toContain('CVD contrario');expect(r.detail).toContain('respaldo rebasado');expect(r.detail).toContain('no acredita defensa, ruptura ni absorción');
});
it('keeps the backing role when a magnet shares the wall strike',()=>{
 const s=fixture();s.optionSnapshotStatus.horizons[0].structure.selected.M1=180;
 expect(directionLevels(s,direction(),'NVDA',true,now).rows.find(l=>l.price===180)?.text).toContain('RESPALDO POTENCIAL');
});
it.each(['tick','snapshot','expiry','symbol','neutral','corridor'])('keeps %s pending',kind=>{
 const s=fixture(),d=direction();if(kind==='tick')s.ticks.NVDA.tradeMs=now-16000;if(kind==='snapshot')s.optionSnapshotStatus.observedMs=now-900001;if(kind==='expiry')s.optionSnapshotStatus.horizons[0].expirations=['2026-10-06'];if(kind==='symbol')s.symbol='AAPL';if(kind==='neutral')d.side='NEUTRAL';if(kind==='corridor')s.optionSnapshotStatus.horizons[0].structure.putWall!.strike=195;
 expect(directionLevels(s,d,'NVDA',true,now).rows).toEqual([]);
});
it('marks proximity and provisional references and renders on the existing map',()=>{
 const s=fixture();s.ticks.NVDA.price=187;s.optionSnapshotStatus.horizons[0].structure.provisional={...s.optionSnapshotStatus.horizons[0].structure,coveredContracts:5,totalContracts:10} as never;
 const linked=directionLevels(s,direction(),'NVDA',true,now);expect(linked.rows.find(l=>l.price===187)?.text).toContain('PRECIO CERCA');expect(linked.rows[0].text).toContain('provisional');
 const html=dayMap(s.optionSnapshotStatus.horizons[0],true,[],false,187,'',undefined,true,'',linked);
 expect(html).toContain('Confluencia y CVD en los niveles');expect(html).toContain('direction-level-tag');expect(directionLevelTag(linked,999)).toBe('');expect(directionLevelsPanel(linked)).toContain('Sesgo y recorrido');
});
it('benchmarks repeated level linkage and complete map rendering',()=>{
 const s=fixture(),d=direction(),h=s.optionSnapshotStatus.horizons[0];
 for(let i=0;i<100;i++)directionLevels(s,d,'NVDA',true,now);
 const start=performance.now();
 for(let i=0;i<3000;i++){const linked=directionLevels(s,d,'NVDA',true,now);dayMap(h,true,[],false,185,'',undefined,true,'',linked);}
 const elapsed=performance.now()-start;
 console.info(`CVD + niveles + HTML: 3000 actualizaciones en ${elapsed.toFixed(1)} ms; ${(elapsed/3000).toFixed(3)} ms/actualización`);
 expect(elapsed).toBeLessThan(15000);
},20000);
