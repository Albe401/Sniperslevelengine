import {describe,it,expect} from 'vitest';
import {companyAnalysisDetail,type CompanyAnalysis} from './company-research';
const fixture:CompanyAnalysis={symbol:'TEST',name:'<img onerror=bad>',asOf:'2026-10-09',revision:'2026-10-11',status:'preliminary',technical:{},pending:['<script>bad</script>'],sources:[],sourceErrors:[],completeTenYearStudy:false};
describe('saved company analysis',()=>{
 it('escapes company narratives and pending evidence',()=>{const html=companyAnalysisDetail(fixture);expect(html).not.toContain('<img');expect(html).not.toContain('<script>');expect(html).toContain('&lt;img');});
 it('keeps missing metrics pending and all nine timeframes visible',()=>{const html=companyAnalysisDetail(fixture);for(const frame of ['1 min','2 min','3 min','5 min','15 min','1 hora','4 horas','Diario','Semanal'])expect(html).toContain(frame);expect(html).toContain('Pendiente de crecimiento BPA comparable');expect(html).toContain('Sin estados financieros disponibles');expect(html).not.toContain('0×');});
 it('labels sensitivity as revenue rather than a stock price forecast',()=>{const html=companyAnalysisDetail({...fixture,futureScenarios:{baseYear:2025,years:10,scenarios:[{name:'base',assumedAnnualGrowthPct:5,revenueAfterTenYears:100}]}});expect(html).toContain('sensibilidad de ingresos');expect(html).toContain('no son objetivos de precio');expect(html).toContain('pendientes de revisión');});
});
