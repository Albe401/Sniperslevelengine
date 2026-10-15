import {it,expect,vi} from 'vitest';
import {learningContent} from './learning';
import type {LearningReport} from './learning';
it('shows gamma learning status and delayed results separately from the generic history',()=>{
 const now=100000;const r={exists:true,connected:true,generatedMs:now,symbols:[],observations:0,eligible:0,sessions:0,buildSessions:0,collector:null,bridge:null,quality:[],outcomes:[],recent:[],gammaCollector:{recorded:true,observed_ms:now,symbol:'QQQ',quality:'READY'},gammaMap:{observations:1,eligible:1,outcomes:[],recent:[{id:1,session:'2026-10-01',price:100,eligible:true,quality:'READY',build:'test',symbol:'QQQ',observedMs:now,side:'CAMBIO DE CALL A PUT',outcomes:[{minutes:5,status:'observed',returnBps:-100,outcomeMs:now}]}]}} as LearningReport;
 const html=learningContent(r,false,'','',now);expect(html).toContain('Registro gamma activo');expect(html).toContain('CAMBIO DE CALL A PUT');expect(html).toContain('-1.000%');
 r.gammaCollector!.recorded=false;expect(learningContent(r,false,'','',now)).toContain('Esperando registro gamma');
});

