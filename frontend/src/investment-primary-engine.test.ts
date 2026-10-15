import {describe,it,expect} from 'vitest';
import {analyzePrimaryInvestment,primaryInvestmentPanel} from './investment-primary-engine';
import {shortlistTable} from './investment-shortlist';
import {opportunityPanel} from './investment-opportunity';
import type {Company} from './investment';
const now=Date.parse('2026-10-09T18:00:00Z');
const sample=()=>({ticker:'TEST',name:'Test company',industry:'Software',eligible:true,status:'Candidata a estudiar',coverage:100,period:'2026-06-30',asOf:'2026-10-08',priceDate:'2026-10-08',warnings:[],sourceErrors:[],metrics:[['roic',20],['operating_margin',20],['fcf_computed',100],['net_debt_ebitda',1],['interest_cover',5],['revenue_yoy',15],['eps_yoy',20],['ema200',90],['last_close',100]].map(([id,value])=>({id,value,source:'Reported test source'})),opportunity:{at:now,fairValue:120}} as unknown as Company);
describe('Primary research framework',()=>{
 it('reserves missing evidence weights without renormalizing partial research',()=>{const r=analyzePrimaryInvestment(sample(),now);expect(r.available).toBe(50);expect(r.earned).toBe(50);expect(r.status).toBe('Investigación incompleta');expect(r.blocks.reduce((s,b)=>s+b.weight,0)).toBe(100);});
 it('rejects stale financial evidence and future model timestamps',()=>{const c=sample();c.period='2020-01-01';c.opportunity!.at=now+1;const r=analyzePrimaryInvestment(c,now);expect(r.available).toBe(5);expect(r.margin).toBeNull();expect(r.status).toBe('Revisar datos y vigencia');});
 it('preserves critical risk priority',()=>{const c=sample();c.metrics.push({id:'equity',value:-1,source:'test'} as Company['metrics'][number]);expect(analyzePrimaryInvestment(c,now).status).toBe('Riesgo prioritario');});
 it('cannot earn invented moat or catalyst scores',()=>{const r=analyzePrimaryInvestment(sample(),now);expect(r.pending).toContain('Ventaja competitiva documentada');expect(r.blocks[5].available).toBe(0);});
 it('handles missing opportunity and escapes supplied sources',()=>{const c=sample();c.opportunity=null;c.metrics[0].source='<script>';const html=primaryInvestmentPanel(c,now);expect(html).not.toContain('<script>');expect(html).toContain('&lt;script&gt;');expect(opportunityPanel(c,now)).toContain('MOTOR PRINCIPAL');});
 it('keeps detailed analysis inside the company rather than on the card list',()=>{const html=shortlistTable([sample()],{},[]);expect(html).toContain('ir-company-card');expect(html).toContain('Abrir análisis completo');expect(html).not.toContain('Ver evidencias y fuentes');expect(html).not.toContain('MOTOR PRINCIPAL');});
});
