import {describe,it,expect} from 'vitest';
import {investmentThesis,investmentThesisPanel} from './investment-thesis';
import type {Company} from './investment';
const now=Date.parse('2026-10-09T18:00:00Z');
const company=()=>({ticker:'TEST',eligible:true,status:'Candidata a estudiar',coverage:95,priceDate:'2026-10-08',metrics:[{id:'last_close',value:80},{id:'drawdown',value:40}],warnings:[],sourceErrors:[],opportunity:{at:now,fairValue:100,margin:99,bear:[],invalidation:70,target:110}} as unknown as Company);
describe('Long-term investment thesis',()=>{
 it('derives margin from price and fair value rather than drawdown or inconsistent stored margin',()=>expect(investmentThesis(company(),now).margin).toBe(20));
 it('does not call a falling expensive stock a bargain',()=>{const c=company();c.opportunity!.fairValue=60;expect(investmentThesis(c,now).title).toBe('Sin rebaja según el modelo');});
 it('suppresses stale and future valuations',()=>{expect(investmentThesis(company(),now+8*86400000).fair).toBeNull();const c=company();c.opportunity!.at=now+1;expect(investmentThesis(c,now).fresh).toBe(false);});
 it('requires investigation when eligibility or coverage fails',()=>{const c=company();c.coverage=50;expect(investmentThesis(c,now).title).toBe('Completar investigación');});
 it('escapes risks and makes missing research explicit',()=>{const c=company();c.warnings=['<script>'];const html=investmentThesisPanel(c,now);expect(html).not.toContain('<script>');expect(html).toContain('guidance');expect(html).toContain('pendiente de verificar');});
});
