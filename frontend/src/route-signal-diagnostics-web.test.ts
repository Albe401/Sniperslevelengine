import {expect,it} from 'vitest';
import fs from 'node:fs';
import {createRouteReader} from './route-live';
import type {LiveState} from './live';
import {validChartSignalDiagnostics} from './chart-signal-diagnostics';

const capture=Date.parse('2026-10-26T10:00:00-04:00'),open=Date.parse('2026-10-26T09:30:00-04:00');
function frame():LiveState {
 const minutes=Array.from({length:20},(_,i)=>({timestampMs:capture-(20-i)*60000,open:100,high:100.1,low:99.9,close:100,volume:10,vwap:100}));
 return {symbol:'QQQ',startedMs:open,publishedMs:capture,stocks:{phase:'streaming',events:20,lastEventMs:capture,lastReceivedMs:capture,malformed:0,reconnects:0},options:{phase:'waiting',events:0,lastEventMs:null,lastReceivedMs:null,malformed:0,reconnects:0},ticks:{QQQ:{symbol:'QQQ',price:100,tradeMs:capture}},metrics:{session:'2026-10-26',phase:'Sesión RTH'},flow:{intraday_windows:[],contract_windows:[],windows:[],recent:[],capacity_drops:0,rejected_trades:0},optionSnapshotStatus:{phase:'waiting',horizons:[]},chartHistory:{sessions:[{date:'2026-10-26',openMs:open,closeMs:open+390*60000}],minutes}} as unknown as LiveState;
}

it('exports the last captured diagnostic with explicit non-live semantics and no native ledger',()=>{
 const reader=createRouteReader(),s=frame(),panel=reader.readRouteLive(s,'QQQ',true,capture);
 const report=reader.routeSignalDiagnosticsSnapshot('QQQ',capture)!;
 expect(report).not.toBeNull();expect(report.coverage.rows).toHaveLength(8);expect(validChartSignalDiagnostics(report)).toBe(true);
 expect(panel.intraday.state).toBe('waiting');expect(report.admissionModified).toBe(false);expect(report.probability).toBeNull();
 expect(Object.isFrozen(report)).toBe(true);expect(reader.routeSignalDiagnosticsSnapshot('SPY',capture)).toBeUndefined();
 expect(reader.routeSignalDiagnosticsSnapshot('QQQ',capture+3001)).toBeUndefined();
 const exported=JSON.parse(reader.routeJournal());
 expect(exported.schema).toBe('route-observation-v1');expect(exported.signalDiagnostics).toEqual(report);
 expect(exported.signalDiagnosticsMode).toBe('snapshot-reference-only');expect(exported.signalDiagnosticsLive).toBe(false);
 expect(exported.signalDiagnosticsStatus).toBe('available');expect(exported.executionEnabled).toBe(false);
});

it('keeps the archived web render clock and notice without a new live capture path',()=>{
 const text=(name:string)=>fs.readFileSync(new URL('./'+name,import.meta.url),'utf8');
 const main=text('web-main.ts'),route=text('route-live.ts');
 expect(main).toContain('now=data.capturedMs');expect(main).toContain('savedReadingNotice(frame,selected,now,clock)');
 expect(main).toContain('capturedReadingMarkup(');expect(main).toContain('quickReadingPanels(frame,selected,true,buildIntradayControl(frame,selected,true,now),now,true)');
 expect(route).not.toContain('RouteOpportunityRecorder');expect(route).not.toContain('intraday-case');expect(route).not.toContain('intraday-capture');
 expect(route).toContain("signalDiagnosticsMode:'snapshot-reference-only'");
});
