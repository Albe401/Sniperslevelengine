import {afterEach,expect,it,vi} from 'vitest';
import type {LiveState} from './live';
const native=vi.hoisted(()=>({invoke:vi.fn(),isTauri:vi.fn(()=>true)}));
vi.mock('@tauri-apps/api/core',()=>native);
afterEach(()=>{vi.useRealTimers();vi.resetModules();vi.clearAllMocks();vi.unstubAllGlobals();});
const start=Date.parse('2026-10-12T09:30:00-04:00');
function frame(now:number,symbol='QQQ'):LiveState {
 return {symbol,publishedMs:now,startedMs:start,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{[symbol]:{symbol,price:100.2,tradeMs:now}},metrics:{session:'2026-10-12',phase:'Sesión RTH'},flow:{intraday_windows:[]},
 chartHistory:{sessions:[{date:'2026-10-12',openMs:start,closeMs:start+390*60000}],minutes:Array.from({length:30},(_,i)=>({timestampMs:start+i*60000,open:100,high:100.3,low:99.8,close:100.2,volume:100}))},optionSnapshotStatus:{phase:'ready',horizons:[]}} as unknown as LiveState;
}
it('archives own QQQ control independently while the selected route is SPY',async()=>{
 vi.useFakeTimers();const now=start+30*60000+1000;vi.setSystemTime(now);
 const qqq=frame(now);native.invoke.mockImplementation(async(name:string)=>name==='get_option_control_frame'?{connected:true,frame:qqq}:name==='get_option_research_status'?{status:'OBSERVING'}:undefined);
 const selected=await import('./route-live');vi.stubGlobal('window',{});selected.readRouteLive(frame(now,'SPY'),'SPY',true,now);
 const before=selected.routeChartReading('SPY',now);
 const background=await import('./option-background-control');await background.pollBackgroundOptionControl();
 const calls=native.invoke.mock.calls.filter(([name])=>name==='record_option_formula_baseline');
 expect(calls).toHaveLength(1);const packet=calls[0][1].payload;
 expect(packet.symbol).toBe('QQQ');expect(packet.connection).toBe(start);expect(packet.publishedMs).toBe(now);
 expect(packet.quant.probabilities).toBeNull();expect(selected.routeChartReading('SPY',now)).toEqual(before);
 expect(selected.routeChartReading('QQQ',now)).toBeUndefined();
 expect(JSON.parse(selected.routeJournal()).formulaBaseline.rows.at(-1).symbol).toBe('QQQ');
});
it('foreign frames and disconnects cannot manufacture a QQQ diary entry',async()=>{
 vi.useFakeTimers();const now=start+30*60000+1000;vi.setSystemTime(now);
 native.invoke.mockResolvedValue({connected:true,frame:frame(now,'SPY')});
 const background=await import('./option-background-control');vi.stubGlobal('window',{});await background.pollBackgroundOptionControl();
 native.invoke.mockResolvedValue({connected:false});await background.pollBackgroundOptionControl();
 expect(native.invoke.mock.calls.filter(([name])=>name==='record_option_formula_baseline')).toHaveLength(0);
});
it('does not overlap background IPC requests',async()=>{
 let resolve!:(v:unknown)=>void;native.invoke.mockImplementation(()=>new Promise(r=>{resolve=r;}));
 const background=await import('./option-background-control');const first=background.pollBackgroundOptionControl();
 await background.pollBackgroundOptionControl();expect(native.invoke).toHaveBeenCalledTimes(1);resolve({connected:false});await first;
});
