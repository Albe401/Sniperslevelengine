import {it,expect} from 'vitest';
import {cvdConfluence} from './cvd-confluence';
import {combineDirection,readDirectionScore,directionBadge} from './direction-scanner';
import type {LiveState} from './live';
import type {ControlReading} from './intraday-control';
const now=Date.UTC(2026,9,5,15),day='2026-10-05';
const period={buy:800,sell:100,unknown:100,delta:700,coveragePct:90,lastMs:now,sinceMs:now-60000,changeOne:100,changeFive:700,gaps:0};
const state=()=>({symbol:'NVDA',publishedMs:now,stocks:{phase:'streaming'},ticks:{},options:{phase:'streaming',lastEventMs:now},metrics:{session:day,phase:'Sesión RTH'},cvd:{symbol:'NVDA',date:day,at:now,available:true,premarket:{...period},regular:{...period},rejected:0}} as unknown as LiveState);
const call=()=>combineDirection(.8,.8,.5,90);
it('confirms or contradicts both directions without changing percentages',()=>{
 for(const sign of [1,-1]) {
 const d=combineDirection(sign*.8,sign*.8,sign*.5,90),s=state(),before=[d.call,d.put];
 expect(cvdConfluence(s,d,'NVDA',true,now).status).toBe(sign===1?'confirms':'contradicts');
 s.cvd!.regular={...period,buy:100,sell:800,delta:-700};
 expect(cvdConfluence(s,d,'NVDA',true,now).status).toBe(sign===-1?'confirms':'contradicts');
 expect([d.call,d.put]).toEqual(before);
 }
});
it.each(['foreign','date','stale','disconnected','closed','coverage','invalid','neutral','unknown'])('keeps %s evidence pending',kind=>{
 const s=state(),d=call();
 if(kind==='foreign')s.cvd!.symbol='AAPL';if(kind==='date')s.cvd!.date='2026-10-02';if(kind==='stale')s.cvd!.regular.lastMs=now-16000;if(kind==='disconnected')s.stocks.phase='connecting';if(kind==='closed')s.metrics.phase='Cierre RTH';if(kind==='coverage')s.cvd!.regular={...period,buy:100,sell:100,unknown:800,delta:0,coveragePct:20};if(kind==='invalid')s.cvd!.regular.delta=Infinity;if(kind==='neutral')d.side='NEUTRAL';if(kind==='unknown')s.cvd!.regular={...period,buy:450,sell:350,unknown:200,delta:100,coveragePct:80};
 expect(cvdConfluence(s,d,'NVDA',true,now).status).toBe('pending');
});
it('selects premarket before open and regular segment after open',()=>{
 const pre=Date.UTC(2026,9,5,12),s=state();s.publishedMs=pre;s.metrics.phase='Esperando primera vela RTH completa';s.cvd!.at=pre;s.cvd!.premarket={...period,lastMs:pre,buy:100,sell:800,delta:-700};
 expect(cvdConfluence(s,call(),'NVDA',true,pre).status).toBe('contradicts');
 s.publishedMs=now;s.cvd!.at=now;s.metrics.phase='Sesión RTH';expect(cvdConfluence(s,call(),'NVDA',true,now).status).toBe('confirms');
});
it('does not manufacture a percentage when the base direction is pending',()=>{
 const s=state();const d=readDirectionScore(s,{symbol:'NVDA',components:[]} as unknown as ControlReading,true,now);
 expect(d.call).toBeNull();expect(d.cvd?.status).toBe('pending');expect(directionBadge(d)).toContain('CVD · PENDIENTE');
});
