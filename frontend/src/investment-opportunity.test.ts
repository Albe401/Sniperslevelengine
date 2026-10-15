import {describe,it,expect} from 'vitest';
import {opportunityPanel,industryMap,type Opportunity} from './investment-opportunity';
import type {Company} from './investment';
const now=Date.parse('2026-09-29T19:00:00Z');
const o:Opportunity={version:'v1',at:now,score:80,coverage:90,blocks:[],fairValue:123,fairLow:100,fairHigh:140,margin:10,scenarios:[],methods:[],assumptions:['<script>'],ma200:100,maDistance:5,maState:'Arriba',rsi:55,bull:['<img>'],bear:[],outlook:'Análisis',entry:110,invalidation:null,target:null,rewardRisk:null,analysts:{source:'FMP',status:'No configurado',fetchedMs:0,providerDate:null,average:null,low:null,high:null,upside:null,consensus:null,buy:null,hold:null,sell:null,total:null},peerScore:null,peerCount:1,peerGroup:'Industria',history:[]};
const c={ticker:'TEST',status:'Vigilar',priceDate:'2026-09-28',metrics:[],opportunity:o} as unknown as Company;
describe('Opportunity research',()=>{
 it('shows missing analyst data instead of synthetic targets or buy instructions',()=>{const html=opportunityPanel(c,now);expect(html).toContain('No configurado');expect(html).not.toContain('COMPRAR FUERTE');expect(html).toContain('Sin soporte y resistencia válidos');expect(html).toContain('no probabilidad');});
 it('escapes supplied narrative and hides stale valuation',()=>{const html=opportunityPanel(c,now+9*86400000);expect(html).toContain('Revisar vigencia');expect(html).not.toContain('$123');expect(html).not.toContain('<script>');expect(html).not.toContain('<img>');});
 it('distinguishes a sample heatmap from money flow',()=>expect(industryMap([])).toContain('no mide dinero entrando o saliendo'));
 it('keeps the known industry visible for an individual query without peers',()=>{const html=opportunityPanel({...c,industry:'Software',opportunity:{...o,peerGroup:''}},now);expect(html).toContain('Software');expect(html).not.toContain('Industria no identificada');});
});
