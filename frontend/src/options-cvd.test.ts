import {it,expect} from 'vitest';
import {optionsCvd} from './options-cvd';
import {flowFour} from './flow-four';
import type {LiveState} from './live';
const now=Date.UTC(2026,9,5,15);
const fixture=()=>({symbol:'NVDA',publishedMs:now,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},metrics:{session:'2026-10-05',phase:'Sesión RTH'},cvd:{symbol:'NVDA',date:'2026-10-05',at:now,available:true,recentFive:{buy:800,sell:100,unknown:100,delta:700,startMs:now-300000,endMs:now,complete:true}},flow:{asof_ms:now,intraday_windows:[{minutes:5,event_count:10,premium_complete:true,buckets:{'CALL BUY':{premium:800},'PUT BUY':{premium:100},'CALL SELL':{premium:0},'PUT SELL':{premium:0},UNKNOWN:{premium:100}}}],windows:[]}} as unknown as LiveState);
it('aligns and diverges symmetrically for buy-side calls and puts',()=>{
 const s=fixture();expect(optionsCvd(s,'NVDA',true,now).status).toBe('aligned');
 s.cvd!.recentFive={...s.cvd!.recentFive!,buy:100,sell:800,delta:-700};expect(optionsCvd(s,'NVDA',true,now).status).toBe('divergent');
 const b=s.flow.intraday_windows![0].buckets!;b['CALL BUY'].premium=100;b['PUT BUY'].premium=800;
 expect(optionsCvd(s,'NVDA',true,now).status).toBe('aligned');
});
it.each(['foreign','stale','window','gap','coverage','mixed','premium','expiry','closed','snapshot'])('keeps %s pending',kind=>{
 const s=fixture();if(kind==='foreign')s.cvd!.symbol='AAPL';if(kind==='stale')s.options.lastEventMs=now-16000;if(kind==='window')s.cvd!.recentFive!.startMs--;if(kind==='gap')s.cvd!.recentFive!.complete=false;if(kind==='coverage')s.cvd!.recentFive!.unknown=1000;if(kind==='mixed')s.flow.intraday_windows![0].buckets!['PUT BUY'].premium=800;if(kind==='premium')s.flow.intraday_windows![0].premium_complete=false;if(kind==='expiry')s.flow.intraday_windows=[];if(kind==='closed')s.metrics.phase='Cierre RTH';if(kind==='snapshot')s.cvd!.at--;
 expect(optionsCvd(s,'NVDA',true,now).status).toBe('pending');
});
it('shows the comparison below cards using the same expiry-scoped window',()=>{
 const html=flowFour(fixture(),'NVDA',true,now);expect(html).toContain('ALINEADOS');expect(html).toContain('$800');expect(html.indexOf('options-cvd')).toBeGreaterThan(html.indexOf('PUT BUY'));
});
it('benchmarks 3000 card updates with alignment',()=>{
 const s=fixture(),start=performance.now();for(let i=0;i<3000;i++)flowFour(s,'NVDA',true,now);
 const elapsed=performance.now()-start;console.info(`Opciones + CVD: 3000 actualizaciones ${elapsed.toFixed(1)} ms; ${(elapsed/3000).toFixed(3)} ms/actualización`);expect(elapsed).toBeLessThan(15000);
});
