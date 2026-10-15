import {expect,it} from 'vitest';
import {decisionReviewPanel} from './decision-review';
it('shows admission only from a current backend result and distinguishes enabled from admitted',()=>{
 const r={reviewedMs:1000,symbol:'QQQ',reason:'PAPER_ENTRY_ADMITTED',entriesEnabled:true,admission:{admitted:true,reason:'ENTRY_ALLOWED',decision:'CALL'}};
 const html=decisionReviewPanel(r,true,1000);expect(html).toContain('SIMULADOR ACTIVO');expect(html).toContain('Simulación abierta');
 expect(decisionReviewPanel({...r,admission:{...r.admission,admitted:false}},true,1000)).not.toContain('>Simulación abierta</strong>');
 expect(decisionReviewPanel(r,true,4001)).not.toContain('SIMULADOR ACTIVO');
 expect(decisionReviewPanel(r,false,1000)).not.toContain('Simulación abierta');
});
it('decision reviews never turn into execution permission and stale reviews are masked',()=>{
 const r={reviewedMs:1000,symbol:'<QQQ>',modelSide:'CALL',prepared:true,reason:'NEW_ENTRIES_DISABLED',entriesEnabled:false};
 expect(decisionReviewPanel(r,true,1000)).toContain('&lt;QQQ&gt;');
 expect(decisionReviewPanel(r,true,1000)).toContain('Entradas automáticas deshabilitadas');
 for(const [active,now] of [[false,1000],[true,999],[true,4001]] as const){expect(decisionReviewPanel(r,active,now)).not.toContain('Modelo &lt;QQQ&gt;');}
 expect(decisionReviewPanel({...r,reason:'<error>'},true,1000)).toContain('&lt;error&gt;');
});
it('position details are escaped and hidden when the review expires',()=>{
 const r={reviewedMs:1000,symbol:'QQQ',reason:'ACTIVE_TRADE_ALREADY_EXISTS',entriesEnabled:false,position:{state:'POSITION_UPDATED',tradeId:7,trade:{side:'<CALL>',entry:500,current:501,stop:499,pnlPoints:1,exitReason:'<reason>'}}};
 const html=decisionReviewPanel(r,true,1000);
 expect(html).toContain('&lt;CALL&gt;');
 expect(html).toContain('&lt;reason&gt;');
 expect(html).toContain('Posición actualizada');
 expect(decisionReviewPanel(r,true,4001)).not.toContain('Posición actualizada');
 expect(decisionReviewPanel(r,false,1000)).not.toContain('&lt;CALL&gt;');
});
