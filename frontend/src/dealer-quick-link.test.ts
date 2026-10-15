import {it,expect} from 'vitest';
import {dealerQuickLink} from './dealer-quick-link';
import {dealerIntradayPanel} from './dealer-intraday';
import {buildIntradayControl} from './intraday-control';
import type {LiveState} from './live';
import type {DirectionScore} from './direction-scanner';
const now=Date.parse('2026-10-02T15:00:00Z'),s={symbol:'QQQ',publishedMs:now,stocks:{phase:'streaming'},metrics:{session:'2026-10-02',phase:'Sesión RTH'},ticks:{}} as unknown as LiveState;
const d:DirectionScore={call:95,put:5,side:'CALL',state:'Confluencia fuerte',reason:'',components:[]};
const levels={summary:'CALL',detail:'PUT WALL rebasado',rows:[]};
function r(sign=-1){const v=buildIntradayControl(undefined,'QQQ',false,now);v.priceNow={ready:true,direction:sign,spot:500,headline:sign<0?'PRECIO CAYENDO':'PRECIO SUBIENDO',tone:'sellers',pace:'',structure:'',vwap:'',change:'',levels:'',at:'',flow:''};return v;}
it('detects 95 CALL with falling price and updates to PUT without retaining Neuro bias',()=>{
 const link=dealerQuickLink(s,r(),d,levels,true,now);expect(link.headline).toBe('DIVERGENCIA · CALL');expect(link.call).toBe(95);
 expect(dealerIntradayPanel(undefined,'QQQ',true,now,link)).toContain('95/100');
 expect(link.rows.join(' ')).toContain('PUT WALL rebasado');
 const next=dealerQuickLink(s,r(),{...d,call:15,put:85,side:'PUT'},levels,true,now);expect(next.call).toBe(15);expect(next.headline).not.toContain('DIVERGENCIA');
});
it('detects rising price against PUT',()=>expect(dealerQuickLink(s,r(1),{...d,side:'PUT',call:5,put:95},levels,true,now).headline).toBe('DIVERGENCIA · PUT'));
it('withholds stale, future, wrong-symbol, disconnected and invalid confluence',()=>{
 for(const x of [undefined,{...s,publishedMs:now-3001},{...s,publishedMs:now+1},{...s,symbol:'SPY'}])expect(dealerQuickLink(x as LiveState|undefined,r(),d,levels,true,now).call).toBeNull();
 expect(dealerQuickLink(s,r(),d,levels,false,now).call).toBeNull();expect(dealerQuickLink(s,r(),{...d,call:NaN},levels,true,now).call).toBeNull();
});
it('preserves inputs and exposes pending structural horizons and absorption',()=>{
 const v=r(),before=JSON.stringify(v),dd=JSON.stringify(d),link=dealerQuickLink(s,v,d,levels,true,now);
 expect(JSON.stringify(v)).toBe(before);expect(JSON.stringify(d)).toBe(dd);expect(link.rows.join(' ')).toContain('ALINEACIÓN DELTA · PENDIENTE');expect(link.rows.join(' ')).toContain('SIN INDICIOS SUFICIENTES');
});
it('rejects stale and wrong-symbol linkage',()=>{
 const link=dealerQuickLink(s,r(),d,levels,true,now);expect(dealerIntradayPanel(undefined,'SPY',true,now,link)).not.toContain('95/100');expect(dealerIntradayPanel(undefined,'QQQ',true,now+3001,link)).not.toContain('95/100');
});

it('flags contradictory CVD and keeps the shared percentages',()=>{const link=dealerQuickLink(s,r(1),{...d,cvd:{status:'contradicts',label:'CVD contrario',detail:''}},levels,true,now);expect(link.headline).toBe('DIVERGENCIA · CALL');expect(link.call).toBe(95);});
import {demoPayload} from '../../integrations/neuro-trader-engine/runtime/frontend/src/lib/demo';
import type {NeuroContext} from './neuro-context';
it('replaces the main Neuro index with Snipers while preserving theoretical metrics',()=>{
 const c:NeuroContext={symbol:'QQQ',at:now,available:true,reason:'',netGamma:1,charmFlow:1,gammaMagnet:500,gammaFlip:499,contracts:100,used:80,modelPayload:demoPayload('QQQ',200)};
 const link=dealerQuickLink(s,r(),d,levels,true,now),html=dealerIntradayPanel(c,'QQQ',true,now,link);
 expect(html).toContain('CONFLUENCIA INTRADÍA · SNIPERS');expect(html).toContain('DIVERGENCIA · CALL · 95/100');expect(html).toContain('Índice teórico Neuro');expect(html).toContain('Engranaje con lectura rápida');
 const put=dealerQuickLink(s,r(),{...d,call:15,put:85,side:'PUT'},levels,true,now);
 expect(dealerIntradayPanel(c,'QQQ',true,now,put)).toContain('PUT · Confluencia fuerte · 85/100');
});
