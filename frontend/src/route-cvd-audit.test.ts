// Isolated audit only. This file is never imported by the application.
import {it,expect} from 'vitest';
import {RouteEngine,type RouteEvidence,type RouteResult} from './route-engine';
import {cvdConfluence} from './cvd-confluence';
import type {LiveState} from './live';
import type {DirectionScore} from './direction-scanner';
import type {Candle} from './model';
const start=Date.UTC(2026,9,5,15),date='2026-10-05';
const bar=(i:number,o=100.2,h=100.3,l=100.1,c=100.2,v=100):Candle=>({timestampMs:start+i*60000,open:o,high:h,low:l,close:c,volume:v});
const sequence=[bar(8,100,100.5,99.8,100.4,200),bar(9,100.2,100.3,99.99,100.15,80),bar(10,100.15,100.7,100.1,100.6,150)];
function context(b:Candle,dir:number):RouteEvidence{return {identity:'QQQ:'+date,now:b.timestampMs+60000,levels:[{name:'wall',price:100,observed:start-60000,asof:b.timestampMs+60000,expiration:date,provisional:false},{name:'target',price:dir===1?102:98,observed:start-60000,asof:b.timestampMs+60000,expiration:date,provisional:false}],options:'audit',delta:'audit',blocks:'audit'};}
function stock(now:number,sign:number){const p={buy:sign===1?800:100,sell:sign===1?100:800,unknown:100,delta:700*sign,coveragePct:90,lastMs:now,sinceMs:start,changeOne:100*sign,changeFive:700*sign,gaps:0};return {symbol:'QQQ',publishedMs:now,stocks:{phase:'streaming'},metrics:{session:date,phase:'Sesión RTH'},cvd:{symbol:'QQQ',date,at:now,available:true,premarket:{...p},regular:{...p},rejected:0}} as unknown as LiveState;}
function direction(r:RouteResult):DirectionScore{return {call:r.direction===1?80:r.direction===-1?20:null,put:r.direction===1?20:r.direction===-1?80:null,side:r.direction===1?'CALL':r.direction===-1?'PUT':'PENDIENTE',state:r.state,reason:r.reason,components:[]};}
function mirror(b:Candle,dir:number){return dir===1?b:{...b,open:200-b.open,high:200-b.low,low:200-b.high,close:200-b.close};}
function ready(dir:number){const e=new RouteEngine();for(const raw of [...Array.from({length:8},(_,i)=>bar(i)),...sequence]){const b=mirror(raw,dir);e.feed(b,context(b,dir));}return e;}
for(const dir of [1,-1]){
 it(`aligned CVD cannot distinguish a successful and failed route (${dir})`,()=>{
  for(const success of [true,false]){const e=ready(dir);expect(e.result.state).toBe('sustained');const saved=structuredClone(e.result),now=start+11*60000;
   expect(cvdConfluence(stock(now,dir),direction(e.result),'QQQ',true,now).status).toBe('confirms');expect(e.result).toEqual(saved);
   const b=mirror(success?bar(11,100.6,102.2,100.4,102.1,200):bar(11,100.5,100.7,99.7,99.8,200),dir);
   expect(e.feed(b,context(b,dir)).state).toBe(success?'reached':'invalidated');
  }
 });
 it(`opposed CVD can coexist with a valid price route (${dir})`,()=>{const e=ready(dir),now=start+11*60000;expect(cvdConfluence(stock(now,-dir),direction(e.result),'QQQ',true,now).status).toBe('contradicts');const b=mirror(bar(11,100.6,102.2,100.4,102.1,200),dir);expect(e.feed(b,context(b,dir)).state).toBe('reached');});
 it(`one pullback and opposed CVD do not elect a reverse route (${dir})`,()=>{const e=ready(dir),b=mirror(bar(11,100.6,100.65,100.15,100.25,80),dir);const r=e.feed(b,context(b,dir));const before=structuredClone(r);expect(cvdConfluence(stock(context(b,dir).now,-dir),direction(r),'QQQ',true,context(b,dir).now).status).toBe('contradicts');expect(e.result).toEqual(before);expect(r.direction).toBe(dir);expect(r.state).toBe('sustained');});
 it(`CVD alone cannot invent a route or target (${dir})`,()=>{const e=new RouteEngine();for(let i=0;i<11;i++){const b=mirror(bar(i),dir);e.feed(b,context(b,dir));}const now=start+11*60000;expect(e.result.direction).toBe(0);expect(cvdConfluence(stock(now,dir),direction(e.result),'QQQ',true,now).status).toBe('pending');expect(e.result.target).toBeNull();});
 it(`CVD cannot supply a missing structural target (${dir})`,()=>{const e=new RouteEngine();for(const raw of [...Array.from({length:8},(_,i)=>bar(i)),...sequence]){const b=mirror(raw,dir),c=context(b,dir);c.levels=c.levels.slice(0,1);e.feed(b,c);}const now=start+11*60000;expect(cvdConfluence(stock(now,dir),direction(e.result),'QQQ',true,now).status).toBe('confirms');expect(e.result.state).toBe('test');expect(e.result.target).toBeNull();});
}
it.each(['stale','foreign','coverage','unknown','corrupt','disconnect','premarket','date'])('inadequate %s CVD adds no evidence',kind=>{const e=ready(1),now=start+11*60000,s=stock(now,1);if(kind==='stale')s.cvd!.at=now-3001;if(kind==='foreign')s.cvd!.symbol='SPY';if(kind==='coverage')Object.assign(s.cvd!.regular,{buy:100,sell:100,unknown:800,delta:0,coveragePct:20});if(kind==='unknown')Object.assign(s.cvd!.regular,{buy:450,sell:350,unknown:200,delta:100,coveragePct:80});if(kind==='corrupt')s.cvd!.regular.delta=999;if(kind==='disconnect')s.stocks.phase='connecting';if(kind==='premarket')s.cvd!.regular.lastMs=null;if(kind==='date')s.cvd!.date='2026-10-02';expect(cvdConfluence(s,direction(e.result),'QQQ',true,now).status).toBe('pending');expect(e.result.state).toBe('sustained');});
it('a strong accumulated CVD can oppose the most recent five-minute window',()=>{const now=start+11*60000,s=stock(now,1);s.cvd!.recentFive={buy:100,sell:800,unknown:100,delta:-700,startMs:now-300000,endMs:now,complete:true};expect(cvdConfluence(s,direction(ready(1).result),'QQQ',true,now).status).toBe('confirms');expect(s.cvd!.recentFive.delta).toBeLessThan(0);});
it('duplicate observations cannot provide new price confirmation',()=>{const e=ready(1),before=structuredClone(e.result),now=start+11*60000;for(let i=0;i<20;i++)cvdConfluence(stock(now,1),direction(e.result),'QQQ',true,now);expect(e.result).toEqual(before);expect(e.bars).toHaveLength(11);});
