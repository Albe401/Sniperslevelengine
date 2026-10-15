import {expect,it} from 'vitest';
import {quickFlowPanel,type FlowWindow} from './quick-flow';
const window:FlowWindow={minutes:5,event_count:4,contracts:20,premium:12000,premium_complete:true,bullish_pct:75,bearish_pct:25,direction:'BULLISH FLOW',buckets:{'CALL BUY':{contracts:8,premium:8000,trades:1},'CALL SELL':{contracts:2,premium:2000,trades:1},'PUT BUY':{contracts:0,premium:0,trades:0},'PUT SELL':{contracts:10,premium:2000,trades:2}}};
const render=(w:FlowWindow[],fresh=true)=>quickFlowPanel(w,fresh,undefined,false,undefined,'QQQ',Date.now());
it('muestra primas USD del bloque de cinco minutos y las cuatro ventanas',()=>{
 const html=render([window]);expect(html).toContain('$8,000');expect(html).toContain('$0');expect(html).toContain('CALL SELL');expect(html).toContain('PUT SELL');expect(html).toContain('30 MIN');expect(html).toContain('Alcista 75%');expect(html).not.toContain('Volumen CALL / PUT');
});
it('no presenta dinero ni dirección vigente con datos caducados, incompletos o sin operaciones',()=>{
 for(const html of [render([window],false),render([{...window,premium_complete:false}]),render([{...window,event_count:0}]),render([])]){expect(html).not.toContain('$8,000');expect(html).not.toContain('Alcista 75%');expect(html).toContain('ESPERANDO');}
});
it('no inventa una prima cero si el bucket falta o tiene un valor no finito',()=>{
 expect(render([{...window,buckets:undefined}])).not.toContain('$0');
 expect(render([{...window,buckets:{'CALL BUY':{contracts:1,premium:NaN,trades:1}}}])).not.toContain('$NaN');
});
it('identifica 0DTE y deja 30 minutos cerrado como contexto secundario',()=>{
 const html=render([window]);expect(html).toContain('OPCIONES 0DTE');expect(html).toContain('<details class="quick-flow-long" data-horizon-details="quick-flow-30">');expect(html).not.toContain('quick-flow-30" open');
});
it('identifies a tiny classified sample beside its directional percentage',()=>{
 const partial:FlowWindow={...window,premium:1200000,bullish_pct:100,bearish_pct:0,buckets:{'CALL BUY':{contracts:1,premium:12000,trades:1},'CALL SELL':{contracts:0,premium:0,trades:0},'PUT BUY':{contracts:0,premium:0,trades:0},'PUT SELL':{contracts:0,premium:0,trades:0},UNKNOWN:{contracts:99,premium:1188000,trades:99}}};
 expect(render([partial])).toContain('Alcista 100%');
 expect(render([partial])).toContain('Prima clasificada: 1.0% de la observada');
 expect(render([partial],false)).not.toContain('Prima clasificada: 1.0%');
});
