import {it,expect} from 'vitest';
import {RouteEngine,type RouteEvidence} from './route-engine';
import type {Candle} from './model';
import {assessRouteStructure} from './route-level-context';
const bar=(i:number,o=100.2,h=100.3,l=100.1,c=100.2,v=100):Candle=>({timestampMs:i*60000,open:o,high:h,low:l,close:c,volume:v});
const context=(b:Candle):RouteEvidence=>({identity:'TEST:2026-10-05:2026-10-05',now:b.timestampMs+60000,levels:[{name:'wall',price:100,observed:0,expiration:'2026-10-05',provisional:true},{name:'target',price:102,observed:0,expiration:'2026-10-05',provisional:true}],options:'partial',delta:'snapshot',blocks:'context'});
const seq=[bar(8,100,100.5,99.8,100.4,200),bar(9,100.2,100.3,99.99,100.15,80),bar(10,100.15,100.7,100.1,100.6,150)];
function ready(){const e=new RouteEngine();for(let i=0;i<8;i++){const b=bar(i);e.feed(b,context(b));}return e;}
function sustained(){const e=ready();for(const b of seq)e.feed(b,context(b));return e;}
it('requires a later test and continuation',()=>{const e=ready();expect(e.feed(seq[0],context(seq[0])).state).toBe('recovery');expect(e.feed(seq[1],context(seq[1])).state).toBe('test');expect(e.feed(seq[2],context(seq[2])).state).toBe('sustained');expect(e.result.target).toBe(102);});
it('never offers execution eligibility or a probability',()=>{expect(sustained().result).not.toHaveProperty('full_signal');expect(sustained().result).not.toHaveProperty('probability');});
it('is symmetric for put direction',()=>{const e=new RouteEngine();for(const raw of [...Array.from({length:8},(_,i)=>bar(i)),...seq]){const b={...raw,open:200-raw.open,high:200-raw.low,low:200-raw.high,close:200-raw.close};const c=context(b);c.levels[1].price=98;e.feed(b,c);}expect(e.result.direction).toBe(-1);expect(e.result.target).toBe(98);});
it('does not flip after one adverse candle',()=>{const e=sustained(),b=bar(11,100.6,100.65,100.15,100.25,80);expect(e.feed(b,context(b)).direction).toBe(1);});
it('a lost level invalidates but does not elect put',()=>{const e=sustained(),b=bar(11,100.5,100.7,99.7,99.8,200);expect(e.feed(b,context(b)).state).toBe('invalidated');expect(e.result.direction).toBe(0);});
it('ends episode at the target close',()=>{const e=sustained(),b=bar(11,100.6,102.2,100.4,102.1,200);expect(e.feed(b,context(b)).state).toBe('reached');expect(e.candidate).toBeUndefined();});
it('cannot confirm without target',()=>{const e=ready();for(const b of seq){const c=context(b);c.levels=c.levels.slice(0,1);e.feed(b,c);}expect(e.result.state).toBe('test');expect(e.result.reason).toContain('pendiente');});
it('missing target invalidates an existing episode',()=>{const e=sustained(),b=bar(11);const c=context(b);c.levels=c.levels.slice(0,1);expect(e.feed(b,c).state).toBe('invalidated');});
it('a duplicate cannot advance a test',()=>{const e=ready();const r=e.feed(seq[0],context(seq[0]));for(let i=0;i<20;i++)expect(e.feed(seq[0],context(seq[0]))).toEqual(r);expect(e.bars).toHaveLength(9);});
it('gap resets persistence',()=>{const e=sustained(),b=bar(12);expect(e.feed(b,context(b)).state).toBe('waiting');});
it('identity resets persistence',()=>{const e=sustained(),b=bar(11);const c=context(b);c.identity='OTHER';expect(e.feed(b,c).state).toBe('waiting');});
it('future level cannot enter an earlier bar',()=>{const e=ready(),b=seq[0],c=context(b);c.levels.forEach(l=>l.observed=b.timestampMs+1);expect(e.feed(b,c).state).toBe('waiting');});
it('a fresh unchanged snapshot preserves first-known time',()=>{const e=ready(),b=seq[0],c=context(b);c.levels.forEach(l=>l.asof=c.now);expect(e.feed(b,c).state).toBe('recovery');});
it.each(['open','stale','invalid','corrected'])('rejects %s bars',kind=>{const e=sustained();let b=bar(11),c=context(b);if(kind==='open')c.now=b.timestampMs+59999;if(kind==='stale')c.now=b.timestampMs+150001;if(kind==='invalid')b.high=99;if(kind==='corrected'){b={...seq[2],close:100.61};c=context(b);}expect(e.feed(b,c).state).toBe('waiting');});
it('high volume alone does not bypass the test',()=>{const e=ready();e.feed(seq[0],context(seq[0]));const b=bar(9,100.3,100.5,100.05,100.15,1000);expect(e.feed(b,context(b)).state).toBe('recovery');});
it('future append cannot mutate an earlier result',()=>{const e=sustained(),saved={...e.result};const b=bar(11,100.6,102.2,99.8,102.1,1e9);e.feed(b,context(b));expect(saved.state).toBe('sustained');expect(saved.target).toBe(102);});
it('limited wall backing requires two separate continuation bars',()=>{
 const e=ready();
 const ctx=(b:Candle)=>{const c=context(b);c.levels[0]={...c.levels[0],role:'call-wall',strength:80};c.levels.push({name:'PUT WALL',price:98,observed:0,expiration:'2026-10-05',provisional:false,role:'put-wall',strength:45});c.structure=assessRouteStructure(c.levels,b.close,1,100,c.now);return c;};
 for(const b of seq)e.feed(b,ctx(b));expect(e.result.state).toBe('test');expect(e.result.direction).toBe(1);
 for(let i=0;i<10;i++)e.feed(seq[2],ctx(seq[2]));expect(e.result.state).toBe('test');
 const next=bar(11,100.6,100.9,100.5,100.8,150);expect(e.feed(next,ctx(next)).state).toBe('sustained');
});
it('pull and vacuum boundaries do not start a support defense',()=>{for(const role of ['pull','vacuum-edge'] as const){const e=ready(),b=seq[0],c=context(b);c.levels[0].role=role;expect(e.feed(b,c).state).toBe('range');}});
