import {expect,it} from 'vitest';
import {scannerPanel} from './scanner';
it('shows analytical signal state separately from admissions and removes it when stale',()=>{
 const c={symbol:'QQQ',sector:null,price:101,volume2m:25000,notional2m:5000000,vwap:100,rvol:1,volumeClass:'BUENO',direction:'ALCISTA',baseReady:true,barEndMs:1000,entryConfirmed:false};
 const d={rows:[],top:[c],historicalReady:1,total:1,connected:true,confirmations:{QQQ:{state:'ENTRADA CONFIRMADA',reason:'<MEMORIA>',targets:[103,105],signalActive:true,reference:101,invalidation:99,entryConfirmed:false}}};
 const html=scannerPanel(d,true,1000,1000);
 expect(html).toContain('Objetivos 103.00 / 105.00');expect(html).toContain('&lt;MEMORIA&gt;');expect(html).toContain('Señal analítica activa · sin admisión de operaciones');
 for(const [active,now] of [[false,1000],[true,4001]] as const) expect(scannerPanel(d,active,1000,now)).not.toContain('ENTRADA CONFIRMADA');
});
it('shows only current candidates and escapes sector and source text',()=>{
 const c={symbol:'<QQQ>',sector:'<sector>',price:500,volume2m:100,notional2m:50000,vwap:499,rvol:1.2,volumeClass:'BUENO',direction:'ALCISTA',baseReady:true,barEndMs:1000,entryConfirmed:false};
 const d={rows:[{...c,status:'<estado>'}],top:[c],historicalReady:1,total:1,connected:true};
 const ready=scannerPanel(d,true,1000,1000);expect(ready).toContain('&lt;QQQ&gt;');expect(ready).toContain('&lt;estado&gt;');expect(ready).toContain('Vigilar');
 for(const [active,published,now] of [[false,1000,1000],[true,1000,4001],[true,121001,121001],[true,999,999]] as const) {expect(scannerPanel(d,active,published,now)).not.toContain('<td>Vigilar');}
});
it('shows trigger references only on a fresh row and preserves fixed identities without live data',()=>{
 const c={symbol:'QQQ',sector:null,price:101,volume2m:25000,notional2m:5000000,vwap:100,rvol:1,volumeClass:'BUENO',direction:'ALCISTA',baseReady:true,barEndMs:1000,entryConfirmed:false};
 const d={rows:[],top:[c],historicalReady:1,total:1,connected:true,
  selection:{session:'2026-09-24',phase:'TOP5 FIJO PROVISIONAL',symbols:['QQQ','<SPY>'],locked:true,lockedAt:1000,entryConfirmed:false},
  triggers:{QQQ:{ready:true,reason:'<RECLAIM>',triggerType:'RECLAIM VWAP 2M',referencePrice:101,invalidation:99,volumeRatio:1,baselineBars:3,entryConfirmed:false}}};
 const current=scannerPanel(d,true,1000,1000);
 expect(current).toContain('&lt;RECLAIM&gt;');expect(current).toContain('Referencia 101');expect(current).toContain('Fijados: QQQ, &lt;SPY&gt;');
 const stale=scannerPanel(d,true,1000,4001);expect(stale).not.toContain('Referencia 101');expect(stale).toContain('Fijados: QQQ, &lt;SPY&gt;');
});
it('keeps each asset flow reason separate and hides expired confirmations',()=>{
 const c={symbol:'QQQ',sector:null,price:101,volume2m:25000,notional2m:5000000,vwap:100,rvol:1,volumeClass:'BUENO',direction:'ALCISTA',baseReady:true,barEndMs:1000,entryConfirmed:false};
 const d={rows:[],top:[c,{...c,symbol:'SPY'}],historicalReady:2,total:2,connected:true,flowConfirmations:{QQQ:{ready:true,aligned:true,reason:'<FLOW QQQ>',entryConfirmed:false}}};
 const html=scannerPanel(d,true,1000,1000);
 expect(html.match(/&lt;FLOW QQQ&gt;/g)?.length).toBe(1);
 expect(html).toContain('Esperando flow propio del activo');
 expect(scannerPanel(d,true,1000,4001)).not.toContain('&lt;FLOW QQQ&gt;');
});
