import {expect,it} from 'vitest';
import {botReadingSummary,type BotQuickContext} from './bot-reading-summary';
import {botAssetMode} from './bot-universe';
const context:BotQuickContext={symbol:'QQQ',at:10000,cvd:{status:'contradicts',side:'PUT'},atr5m:2,targetAtr:1.5,alignment:'pending',structure:[{mode:'DAY',side:'pending'}],absorption:'pending'};
it('keeps experimental arrival comparison inside existing details and rejects stale or foreign context',()=>{
 const arrivalBaseline={version:'bot-arrival-context-v1',symbol:'QQQ',readAtMs:10000,state:'available',executionEnabled:false,forecasts:[{relationToBot:'same-direction',forecast:{probability:0.7,issuedMs:9000,case:{deadlineMs:200000,direction:'up',level:{horizon:'DAY',zone:{low:105,high:105}}}}}]};
 const html=botReadingSummary({...context,arrivalBaseline},10000);
 expect(html).toContain('70.0%');expect(html).toContain('mismo sentido que el bot');expect(html).toContain('Sin calibrar');expect(html.match(/<details/g)).toHaveLength(1);
 for(const bad of [{...arrivalBaseline,symbol:'SPY'},{...arrivalBaseline,readAtMs:10001},{...arrivalBaseline,readAtMs:4999},{...arrivalBaseline,executionEnabled:true}])expect(botReadingSummary({...context,arrivalBaseline:bad},10000)).not.toContain('70.0%');
});
it('explains EMA9 admission with scoped real pressure and escapes blockers',()=>{
 const ema={version:'ema9-rebound-v1',symbol:'QQQ',at:10000,confirmed:false,detected:true,side:'PUT',volumeRatio:1.5,buyShares:10,sellShares:100,unknownShares:20,coveragePct:84.6,cvdCloses:1,rewardRisk:2,reasons:['<script>pending</script>'],strategyConnected:true};
 const html=botReadingSummary({...context,ema9Rebound:ema},10000);expect(html).toContain('EMA 9 · AGRESIVO · PUT');expect(html).toContain('1.50×');expect(html).toContain('CVD 1 cierres');expect(html).toContain('&lt;script&gt;');expect(html).not.toContain('<script>');expect(html).toContain('rentabilidad en opciones todavía no validada');
 for(const bad of [{...ema,symbol:'SPY'},{...ema,at:4999},{...ema,at:10001}])expect(botReadingSummary({...context,ema9Rebound:bad},10000)).not.toContain('EMA 9 · AGRESIVO · PUT');
});
it('shows scoped gaps inside the existing bot details without overriding FVG',()=>{
 const gaps={version:'gap-bot-v1',symbol:'QQQ',at:10000,available:true,mode:'context-only',zones:[{lower:103,upper:104,direction:'above',inPath:true,state:'OPEN'}]};
 const html=botReadingSummary({...context,gaps},10000);expect(html).toContain('GAP ENGINE · $103.00–$104.00 · en recorrido');expect(html).toContain('no añade votos');expect(html.match(/<details/g)).toHaveLength(1);
 for(const bad of [{...gaps,symbol:'SPY'},{...gaps,at:4999},{...gaps,at:10001}])expect(botReadingSummary({...context,gaps:bad},10000)).not.toContain('GAP ENGINE · $103');
});
it('keeps FVG in existing details, validates scope and separates support from authorization',()=>{
 const fvg={version:'fvg-v1',symbol:'QQQ',at:10000,available:true,alignment:'supports-pullback',pathBlocked:false,selected:{symbol:'QQQ',side:'BULLISH',timeframe:2,low:101,midpoint:102,high:103,state:'PARTIAL',strength:'STRONG',distance:0}};
 const html=botReadingSummary({...context,fvg},10000);expect(html).toContain('FVG STRUCTURE · BULLISH | 2M');expect(html).toContain('distancia $0.00');expect(html).toContain('Un FVG no activa entradas');expect(html).not.toContain('70%');
 for(const bad of [{...fvg,symbol:'SPY'},{...fvg,at:4999},{...fvg,selected:{...fvg.selected,state:'FILLED'}}])expect(botReadingSummary({...context,fvg:bad},10000)).not.toContain('FVG STRUCTURE · BULLISH');
});
it('shows only the chosen entry mode and preserves holding-management explanation',()=>{
 expect(botAssetMode('qqq')).toContain('data-bot-asset-mode="qqq" aria-pressed="true"');
 expect(botAssetMode('five')).toContain('data-bot-asset-mode="five" aria-pressed="true"');
 expect(botAssetMode('qqq')).toContain('posiciones abiertas siguen bajo gestión');
});
it('reports contradiction and ATR without claiming exhaustion or absorption',()=>{
 const html=botReadingSummary(context,10000);expect(html).toContain('CVD contradice');expect(html).toContain('$2.00');expect(html).toContain('1.50 ATR');expect(html).toContain('no confirma agotamiento');expect(html).toContain('pendiente de confirmación independiente');
});
it('never presents stale or future confirmations as current',()=>{
 expect(botReadingSummary(context,16000)).toContain('esperando datos');expect(botReadingSummary(context,9999)).toContain('esperando datos');
});
it('shows own route and insufficient samples inside the existing details',()=>{const c:BotQuickContext={...context,priceRoute:{symbol:'QQQ',at:10000,mode:'context-only',fourHourAvailable:true,intradayAlignment:'opposes',fourHour:{scenario:{side:1,target:105,invalidation:98,targetSource:'known-pivot'}},projections:[{horizon:'swing2',state:'insufficient',n:3,typical:null}]}};const html=botReadingSummary(c,10000);expect(html).toContain('4H CALL');expect(html).toContain('contradice la lectura intradía');expect(html).toContain('historial insuficiente');expect(html).toContain('No cambia objetivos ni stops');expect(html).not.toContain('70%');});
it('does not show foreign or stale route targets',()=>{for(const route of [{symbol:'SPY',at:10000},{symbol:'QQQ',at:4999}]){const c:BotQuickContext={...context,priceRoute:{...route,mode:'context-only',fourHourAvailable:true,intradayAlignment:'supports',fourHour:{scenario:{side:1,target:109,invalidation:98,targetSource:'known-pivot'}}}};expect(botReadingSummary(c,10000)).not.toContain('$109.00');}});


it('shows only fresh own-symbol chart sequences with arrival and invalidation',()=>{const chartRoute={version:'bot-chart-route-v1',symbol:'QQQ',at:10000,state:'sustained',side:'CALL',confirmed:true,target:105,invalidation:99,rewardRisk:2,reason:'<script>',atr14FiveMinute:2,targetAtr:2,cvdCloses:2,strategyConnected:true};const html=botReadingSummary({...context,chartRoute},10000);expect(html).toContain('posible llegada $105.00');expect(html).toContain('invalida cierre en $99.00');expect(html).toContain('CVD en 2 cierres');expect(html).not.toContain('<script>');for(const bad of [{...chartRoute,symbol:'SPY'},{...chartRoute,at:4999},{...chartRoute,at:10001}])expect(botReadingSummary({...context,chartRoute:bad},10000)).not.toContain('posible llegada $105.00');});
