import {describe,it,expect} from 'vitest';
import {learningHealthPanel} from './learning-health';
import type {LearningReport} from './learning';
describe('learning health evidence',()=>{
 it('distinguishes pending data and journal anomalies from a verified state',()=>{
  const d={generatedMs:1000,connected:true,observations:10,eligible:3,collector:{symbol:'QQQ',observed_ms:1000,recorded:true,eligible:false,quality:'INCOMPLETE_INTRADAY_STRUCTURE'},bot:{available:true,closed:2,open:0,pending:0,incomplete:1,journalAudit:{available:true,records:7,future:1,invalidJson:0,orphaned:0}}} as LearningReport;
  const html=learningHealthPanel(d,1000);expect(html).toContain('Revisar registro');expect(html).toContain('3 aptas de 10');expect(html).toContain('Revisar incompletas');expect(html).toContain('datos incompletos');
 });
});
