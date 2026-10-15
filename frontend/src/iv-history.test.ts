import {it,expect,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>false}));
import {readIvHistory} from './iv-history';
const now=Date.parse('2026-10-05T18:00:00Z');
function history(){const start=Date.parse('2025-10-06T18:00:00Z');return {symbol:'NVDA',status:'complete',start:'2025-10-06',end:'2026-10-05',expected:251,observations:Array.from({length:251},(_,i)=>{const at=start+i*(now-start)/250;return {date:new Date(at).toISOString().slice(0,10),at,iv:20+i/10};})};}
it('compares the latest reference IV with the same asset series',()=>expect(readIvHistory(history(),'NVDA',now).rank).toBe(100));
it('rejects another asset, insufficient coverage and old histories',()=>{expect(readIvHistory(history(),'QQQ',now).rank).toBeNull();const h=history();h.observations=h.observations.slice(0,200);expect(readIvHistory(h,'NVDA',now).rank).toBeNull();expect(readIvHistory(history(),'NVDA',now+8*86400000).rank).toBeNull();});
it('excludes future data and duplicate dates from coverage',()=>{const h=history();h.observations=h.observations.slice(0,200);h.observations.push(...Array.from({length:51},()=>h.observations[0]));expect(readIvHistory(h,'NVDA',now).rank).toBeNull();const future=history();future.observations.forEach(o=>o.at+=365*86400000);expect(readIvHistory(future,'NVDA',now).rank).toBeNull();});
it('permits isolated missing quotes with at least 98% coverage',()=>{const h=history();h.observations.splice(40,1);expect(readIvHistory(h,'NVDA',now).rank).toBe(100);});
it('does not mix old and improved construction in a rank',()=>{const h={...history(),modelVersion:2};expect(readIvHistory(h,'NVDA',now).rank).toBeNull();});
it('reports spread uncertainty around the same-series point estimate',()=>{const base=history();base.observations.at(-1)!.iv=30;const h={...base,modelVersion:2,observations:base.observations.map(o=>({...o,modelVersion:2,ivLow:o.iv-.5,ivHigh:o.iv+.5}))};const result=readIvHistory(h,'NVDA',now);expect(result.rank).not.toBeNull();expect(result.rankBand![0]).toBeLessThanOrEqual(result.rank!);expect(result.rankBand![1]).toBeGreaterThanOrEqual(result.rank!);expect(result.ivBand).toEqual([29.5,30.5]);});
