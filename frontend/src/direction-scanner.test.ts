import {expect,it} from 'vitest';
import {combineDirection,readDirectionScore,directionBadge} from './direction-scanner';
import type {ControlReading} from './intraday-control';
import type {LiveState} from './live';
const now=Date.UTC(2026,9,5,15);
const reading=()=>({symbol:'QQQ',priceNow:{ready:true},components:[{name:'PRECIO 2M + VWAP',signal:.8,weight:20}]} as unknown as ControlReading);
const fixture=()=>({symbol:'QQQ',publishedMs:now,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{QQQ:{symbol:'QQQ',price:740,tradeMs:now}},metrics:{session:'2026-10-05',phase:'Sesión RTH'},flow:{intraday_windows:[{minutes:5,event_count:20,premium_complete:true,buckets:{'CALL BUY':{premium:800},'PUT SELL':{premium:100},'PUT BUY':{premium:50},'CALL SELL':{premium:50},'UNKNOWN':{premium:0}}}]},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:['2026-10-05'],structure:{complete:true,callWall:{strike:745,score:60},putWall:{strike:730,score:90},selected:{M1:743},routes:[]}}]}} as unknown as LiveState);
it('keeps percentages complementary without claiming probability',()=>{
 for(const p of [-1,-.4,0,.4,1])for(const f of [-1,0,1]){const r=combineDirection(p,f,.2,80);expect(r.call!+r.put!).toBe(100);expect(r.call).toBeGreaterThanOrEqual(0);expect(r.call).toBeLessThanOrEqual(100);}
 expect(directionBadge(combineDirection(1,1,1,80))).toContain('no probabilidad de éxito');
});
it('attenuates weak, missing and provisional support without reversing direction',()=>{
 const strong=combineDirection(.8,.8,.5,90),weak=combineDirection(.8,.8,.5,45),missing=combineDirection(.8,.8,.5,null),partial=combineDirection(.8,.8,.5,90,true);
 expect(weak.call!).toBeLessThan(strong.call!);expect(missing.call!).toBeLessThan(weak.call!);expect(partial.call!).toBeLessThan(strong.call!);expect(weak.call!).toBeGreaterThan(50);
});
it('mirrors CALL and PUT evidence and accepts observed zero strength',()=>{
 const a=combineDirection(.8,.6,.4,0),b=combineDirection(-.8,-.6,-.4,0);expect(a.call).toBe(b.put);expect(a.put).toBe(b.call);
});
it('marks conflicting price and premium as mixed instead of activation',()=>{
 expect(combineDirection(1,-.1,1,100).state).toBe('Lectura mixta');expect(combineDirection(0,0,0,100).side).toBe('NEUTRAL');
});
it('rejects nonfinite signals and out of range support',()=>{
 for(const n of [NaN,Infinity,2])expect(combineDirection(n,0,0,80).side).toBe('PENDIENTE');
 for(const n of [-1,101,NaN])expect(combineDirection(1,1,1,n).call).toBeNull();
});
it('does not populate a closed/disconnected symbol from prior readings',()=>{
 const r={symbol:'QQQ',components:[]} as unknown as ControlReading;
 expect(readDirectionScore(undefined,r,false).call).toBeNull();expect(directionBadge(readDirectionScore(undefined,r,false))).toContain('CALL <strong>—');
});
it('combines live symbol price, classified premium and actual DAY walls',()=>{
 const result=readDirectionScore(fixture(),reading(),true,now);expect(result.side).toBe('CALL');expect(result.call!+result.put!).toBe(100);expect(result.components).toHaveLength(3);
});
it('attaches CVD confirmation to the actual live percentage without reweighting it',()=>{
 const s=fixture(),base=readDirectionScore(s,reading(),true,now);
 const p={buy:800,sell:100,unknown:100,delta:700,coveragePct:90,sinceMs:now-60000,lastMs:now,changeOne:100,changeFive:700,gaps:0};
 s.cvd={symbol:'QQQ',date:'2026-10-05',at:now,available:true,premarket:p,regular:p,rejected:0};
 const confirmed=readDirectionScore(s,reading(),true,now);
 expect(confirmed.cvd?.status).toBe('confirms');expect(confirmed.call).toBe(base.call);
 expect(directionBadge(confirmed)).toContain('CVD · CONFIRMA CALL');
 s.cvd.regular={...p,buy:100,sell:800,delta:-700};
 const opposed=readDirectionScore(s,reading(),true,now);expect(opposed.cvd?.status).toBe('contradicts');expect(opposed.call).toBe(base.call);
 expect(directionBadge(opposed)).toContain('CVD · CONTRADICE CALL');
});
it.each(['foreign','publication','tick','flow','expiry','snapshot','closed','missing-price'])('rejects mismatched/stale sources: %s',kind=>{
 const s=fixture(),r=reading();if(kind==='foreign')s.symbol='SPY';if(kind==='publication')s.publishedMs=now-4000;if(kind==='tick')s.ticks.QQQ.tradeMs=now-16000;if(kind==='flow')s.options.lastEventMs=now-16000;if(kind==='expiry')s.optionSnapshotStatus.horizons[0].expirations=['2026-10-06'];if(kind==='snapshot')s.optionSnapshotStatus.observedMs=now-900001;if(kind==='closed')s.metrics.phase='Cierre RTH';if(kind==='missing-price')r.priceNow!.ready=false;
 expect(readDirectionScore(s,r,true,now).call).toBeNull();
});
it('unknown premium attenuates rather than redistributing known money',()=>{
 const s=fixture(),base=readDirectionScore(s,reading(),true,now);s.flow.intraday_windows![0].buckets!.UNKNOWN.premium=200;expect(readDirectionScore(s,reading(),true,now).call!).toBeLessThan(base.call!);
 s.flow.intraday_windows![0].buckets!.UNKNOWN.premium=500;expect(readDirectionScore(s,reading(),true,now).call).toBeNull();
});
it('keeps the badge next to the existing price inside the map header',async()=>{
 const {dayMap}=await import('./day-map');const html=dayMap(undefined,false,[],false,null,'PRICE',undefined,true,directionBadge(readDirectionScore(undefined,reading(),false,now)));
 expect(html.indexOf('PRICE')).toBeLessThan(html.indexOf('direction-badge'));expect(html.indexOf('direction-badge')).toBeLessThan(html.indexOf('day-map-track'));
});
