import {describe,it,expect} from 'vitest';
import {individualResult,format,matchesInvestmentStatus,investmentPage,investmentTabs,displayStatus,companyDetail,csvCell,type Company} from './investment';
const c:Company={ticker:'TEST',name:'<script>bad</script>',industry:'Industry',sic:'1234',cik:'123',asOf:'2026-09-25',period:'2026-06-30',filing:'2026-08-01',priceDate:'2026-09-25',score:null,coverage:0,status:'Datos incompletos',metrics:[],checks:[],warnings:['<img onerror=bad>'],annual:[],closes:[],sourceErrors:[],eligible:false};
describe('Long term research presentation',()=>{
 it('does not convert unavailable metrics into zeros',()=>{expect(format(null,'pct')).toBe('—');expect(format(NaN)).toBe('—');expect(format(0,'pct')).toBe('0%');});
 it('invalidates stale and future cached classifications',()=>{const strong={...c,status:'Candidata a estudiar'};expect(displayStatus(strong,Date.parse('2026-09-26'))).toBe('Candidata a estudiar');expect(displayStatus(strong,Date.parse('2026-10-20'))).toBe('Revisar vigencia');expect(displayStatus(strong,Date.parse('2026-09-20'))).toBe('Revisar vigencia');});
 it('escapes company names and warnings',()=>{const html=companyDetail(c);expect(html).not.toContain('<script>');expect(html).not.toContain('<img onerror');expect(html).toContain('&lt;script&gt;');});
 it('CSV protects spreadsheet formulas and escapes quotes',()=>{expect(csvCell('=HYPERLINK("bad")')).toBe('"\'=HYPERLINK(""bad"")"');expect(csvCell('A,B')).toBe('"A,B"');expect(csvCell(-12.5)).toBe('"-12.5"');});
});

describe('Investment shortlist',()=>{
 const now=Date.parse('2026-09-26');
 it('defaults to candidates and watch companies only',()=>{
  const statuses=['Candidata a estudiar','Vigilar','Sin caída requerida','Riesgo elevado','Valoración exigente','Datos incompletos','Especializado','Unknown'];
  expect(statuses.filter(status=>matchesInvestmentStatus({...c,status},undefined,now))).toEqual(['Candidata a estudiar','Vigilar']);
  expect(investmentPage()).toContain('value="research" selected');
 });
 it('excludes stale, future and invalid cached candidates and watch companies',()=>{
  for(const status of ['Candidata a estudiar','Vigilar'])for(const asOf of ['2026-09-01','2026-09-27','invalid'])expect(matchesInvestmentStatus({...c,status,asOf},undefined,now)).toBe(false);
 });
 it('allows explicit review of excluded results without changing classification',()=>{
  expect(matchesInvestmentStatus(c,'',now)).toBe(true);
  expect(matchesInvestmentStatus(c,'Datos incompletos',now)).toBe(true);
  expect(matchesInvestmentStatus({...c,status:'Vigilar'},'En vigilancia',now)).toBe(true);
  expect(matchesInvestmentStatus(c,'En vigilancia',now)).toBe(false);
 });
});

describe('Individual company research',()=>{
 it('renders excluded companies with supports and metrics independently of shortlist filters',()=>{const company={...c,ticker:'NIO',status:'Riesgo elevado',zones:{atr:1,sessions:260,supports:[{center:3,low:2.9,high:3.1,pivots:4,evidence:['Pivote confirmado']}],resistances:[{center:5,low:4.9,high:5.1,pivots:3,evidence:['Pivote confirmado']}]}};expect(matchesInvestmentStatus(company,'research',Date.parse('2026-09-26'))).toBe(false);const html=individualResult(company);expect(html).toContain('NIO');expect(html).toContain('Soporte 1');expect(html).toContain('Resistencia 1');expect(html).toContain('EMA 200');expect(html).toContain('ROE');expect(html).toContain('data-ir-research="NIO"');});
 it('provides a separate form and readable errors without fabricating a company',()=>{expect(investmentPage()).toContain('data-ir-individual-form');expect(individualResult(null,'<error>')).toContain('&lt;error&gt;');expect(individualResult(null)).not.toContain('FICHA DE INVESTIGACIÓN');});
});

describe('Investment motor tabs',()=>{
 it('opens the new company radar first and mounts the CALL/PUT scanner beside existing motors',()=>{
  const tabs=investmentTabs(),page=investmentPage();
  expect(tabs.indexOf('data-ir-tab="purchases"')).toBeLessThan(tabs.indexOf('data-ir-tab="radar"'));
  expect(tabs).toContain('data-ir-tab="direction"');
  expect(tabs).toContain('data-ir-tab="individual"');
  expect(tabs).toContain('data-ir-tab="option-prints"');
  expect(tabs).toContain('data-ir-tab="block-alerts"');
  expect(page).toContain('data-ir-active="radar"');
  for(const marker of ['ir-daily-purchases','option-print-scanner','investment-direction','data-ir-individual-form','data-ir-form','data-block-alert-form','data-block-alert-history'])expect(page).toContain(marker);
 });
});

it('places the complete chain in its own top Radar tab',()=>{expect(investmentTabs()).toContain('data-ir-tab="contract-chain"');expect(investmentTabs()).toContain('Cadena de opciones');expect(investmentPage()).toContain('investment-contract-chain');expect(investmentPage()).toContain('data-chain-symbol');});

it('keeps the primary framework out of the separate individual module',()=>{expect(individualResult(c)).not.toContain('MOTOR PRINCIPAL');expect(companyDetail(c)).toContain('MOTOR PRINCIPAL');});
