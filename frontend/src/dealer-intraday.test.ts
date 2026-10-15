import {it,expect} from 'vitest';
import {dealerIntradayPanel} from './dealer-intraday';
import {demoPayload} from '../../integrations/neuro-trader-engine/runtime/frontend/src/lib/demo';
import {deriveView,magnet} from '../../integrations/neuro-trader-engine/runtime/frontend/src/lib/analytics';
import type {NeuroContext} from './neuro-context';
import {buildIntradayControl,controlPanel} from './intraday-control';
const p=demoPayload('QQQ',200);
const c:NeuroContext={symbol:'QQQ',at:100000,available:true,reason:'',netGamma:1,charmFlow:1,gammaMagnet:500,gammaFlip:499,contracts:100,used:80,modelPayload:p};
it('shows the dealer control score while stock candle context is pending',()=>{
 const r=buildIntradayControl(undefined,'QQQ',false);
 r.neuroContext={...c,at:r.readingAt!};
 expect(r.priceNow?.ready).toBe(false);
 expect(controlPanel(r)).toContain(`${Math.round(deriveView(p,true,'std').sc.control)}%`);
});
it('uses the same bias, dealer bars and magnet shares as Neuro',()=>{
 const html=dealerIntradayPanel(c,'QQQ',true,100000),v=deriveView(p,true,'std'),m=magnet(p,v);
 expect(html).toContain(`${Math.round(v.sc.bias)}/100`);
 expect(html).toContain(`${Math.round(v.sc.control)}%`);
 for(const row of m.rows)expect(html).toContain(`${Math.round(row.share*100)}%`);
 expect(html).toContain('MOMENTUM · 30 MIN');expect(html).toContain('Max pain');expect(html).toContain('Zona de aceleración');
});
it('withholds readings for mismatched, stale, future, absent or disconnected data',()=>{
 for(const [context,symbol,active,now] of [[c,'SPY',true,100000],[c,'QQQ',false,100000],[c,'QQQ',true,99999],[c,'QQQ',true,145001],[{...c,modelPayload:null},'QQQ',true,100000],[{...c,available:false},'QQQ',true,100000]] as const){
 expect(dealerIntradayPanel(context,symbol,active,now)).toContain('Sesgo pendiente');
 }
});
it('keeps closed-session reason internal without presenting a live dealer reading',()=>{
 const unavailable={...c,available:false,reason:'Neuro sin evidencia de sesión regular en tiempo real',modelPayload:null};
 const html=dealerIntradayPanel(unavailable,'QQQ',true,100000);
 expect(html).not.toContain(unavailable.reason);
 expect(html).toContain('Sesgo pendiente');
 expect(html).not.toContain('Índice teórico');
});
it('keeps dealer metrics inside summary column before the magnet',()=>{
 for(const html of [dealerIntradayPanel(c,'QQQ',true,100000),dealerIntradayPanel(undefined,'QQQ',false,100000)]){
 const metrics=html.indexOf('class="dealer-metrics"'),end=html.indexOf('</section>'),magnet=html.indexOf('class="dealer-card dealer-magnet"');
 expect(metrics).toBeGreaterThan(0);expect(metrics).toBeLessThan(end);expect(end).toBeLessThan(magnet);
 }
});
