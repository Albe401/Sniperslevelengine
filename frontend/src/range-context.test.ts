import {it,expect} from 'vitest';
import {rangeContextPanel,type RangeContext} from './range-context';
it('opening context hides foreign, stale and disconnected readings',()=>{
 const now=Date.parse('2026-09-24T14:00:00Z');
 const r:RangeContext={ready:true,symbol:'QQQ',date:'2026-09-24',anchor:500,lower:490,upper:510,remainingCall:1,remainingPut:19,extensionCall:0,extensionPut:0,usedCallPct:60,usedPutPct:30,rangeState:'CERCA DEL LÍMITE CALL',alignmentRangeState:'<context>',detail:'<source>',extremesComplete:true,priceMs:now-14000};
 expect(rangeContextPanel(r,'QQQ',true,now,now)).toContain('&lt;context&gt;');
 expect(rangeContextPanel(r,'QQQ',true,now,now,true)).not.toContain('&lt;context&gt;');
 for(const [symbol,active,at] of [['SPY',true,now],['QQQ',false,now],['QQQ',true,now+1001]] as const)expect(rangeContextPanel(r,symbol,active,now,at)).not.toContain('>CERCA DEL LÍMITE CALL<');
});
import {quickRangePanel} from './range-context';
it('quick range shows the session bands and hides stale or foreign data',()=>{
 const now=Date.parse('2026-09-24T14:00:00Z');
 const r:RangeContext={ready:true,symbol:'QQQ',date:'2026-09-24',anchor:500,lower:490,upper:510,remainingCall:1,remainingPut:19,extensionCall:0,extensionPut:0,usedCallPct:60,usedPutPct:30,rangeState:'DENTRO DEL RANGO',alignmentRangeState:'<context>',detail:'',extremesComplete:true,priceMs:now};
 const html=quickRangePanel(r,'QQQ',true,now,undefined,509,now);
 expect(html).toContain('490.00');expect(html).toContain('510.00');expect(html).toContain('509.00');expect(html).not.toContain('&lt;context&gt;');
 for(const [symbol,active,at] of [['SPY',true,now],['QQQ',false,now],['QQQ',true,now+16000]] as const){
  const hidden=quickRangePanel(r,symbol,active,now,undefined,509,at);
  expect(hidden).not.toContain('490.00');expect(hidden).toContain('Esperando rango vigente');
 }
});
