import {describe,it,expect} from 'vitest';

import {zonePanel,distance,matchesEma,emaCards,researchPanel,type Research} from './investment-research';

import {needsLevels,type Company} from './investment';

const c={ticker:'TEST',priceDate:'2026-09-25',metrics:[{id:'last_close',value:102},{id:'ema50',value:100},{id:'ema200',value:110},{id:'sma200',value:108}]} as Company;

describe('Investment structure presentation',()=>{

 it('filters symmetrically above and below an EMA',()=>{expect(matchesEma(c,'ema50',3)).toBe(true);expect(matchesEma(c,'ema50',1)).toBe(false);expect(matchesEma(c,'ema200',10)).toBe(true);expect(matchesEma(c,'either',3)).toBe(true);});

 it('missing EMA never matches proximity',()=>{const missing={...c,metrics:[]};expect(distance(missing,'ema50')).toBeNull();expect(matchesEma(missing,'either',10)).toBe(false);expect(matchesEma(missing,'',10)).toBe(true);});

 it('distinguishes EMA from SMA and above from below',()=>{const html=emaCards(c);expect(html).toContain('EMA 50');expect(html).toContain('EMA 200');expect(html).toContain('SMA 200');expect(html).toContain('Precio por encima');expect(html).toContain('Precio por debajo');});

 it('never confirms a dominant level from a partial chain',()=>{const r={ticker:'TEST',fetchedMs:Date.now(),chainComplete:false,chainRows:1,referenceDate:'2026-09-25',structure:{supports:[],resistances:[],dominantSupport:null,dominantResistance:null,accepted:0},tradeComplete:false,examinedTrades:30000,blocks:[],errors:['<script>unsafe</script>']} as unknown as Research;const html=researchPanel({...c,research:r},'');expect(html).toContain('provisional');expect(html).toContain('MUESTRA PARCIAL');expect(html).not.toContain('<script>');expect(html).toContain('&lt;script&gt;');});

});



describe('Automatic level loading',()=>{

 it('requests missing and expired levels but reuses matching current data',()=>{const now=100000000;expect(needsLevels(c,now)).toBe(true);const current={...c,research:{fetchedMs:now,referenceDate:c.priceDate,referencePrice:102} as Research};expect(needsLevels(current,now)).toBe(false);expect(needsLevels(current,now+86400001)).toBe(true);expect(needsLevels({...current,priceDate:'2026-09-28'},now)).toBe(true);});

 it('does not misreport unloaded trades as an empty completed search',()=>{const r={fetchedMs:Date.now(),referencePrice:102,referenceDate:c.priceDate,structure:{supports:[],resistances:[],accepted:0},blocks:[],errors:[],tradesRequested:false} as unknown as Research;const html=researchPanel({...c,research:r},'');expect(html).toContain('Los niveles se cargan automáticamente');expect(html).not.toContain('No se observaron bloques válidos');});

});



it('historical zones preserve missing data and only match current OI inside the band',()=>{

 expect(zonePanel(c)).toContain('vuelve a explorar');

 const company={...c,zones:{supports:[{center:100,low:99,high:101,pivots:2,evidence:['EMA 50 diaria']}],resistances:[],sessions:260,atr:2},research:{fetchedMs:Date.now(),referenceDate:c.priceDate,referencePrice:102,chainComplete:false,structure:{supports:[{strike:100}],resistances:[]}} as unknown as Research};

 expect(zonePanel(company)).toContain('Coincide con OI PUT');

 expect(zonePanel(company)).toContain('muestra parcial');

 expect(zonePanel(company)).toContain('Sin resistencias históricos confirmados');

 expect(zonePanel({...company,research:{...company.research,fetchedMs:1}})).not.toContain('Coincide con OI PUT');

});



