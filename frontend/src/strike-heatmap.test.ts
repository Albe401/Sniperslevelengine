import {it,expect} from 'vitest';
import {strikeHeatmap} from './strike-heatmap';
it.each(['oi','volume'])('shows exact CALL/PUT contract counts for %s',metric=>{
 const svg=strikeHeatmap([{strike:500,call:20000,put:15000,y:120}],520,metric,[],true,20000);
 expect(svg).toContain('PUT 15.000 · CALL 20.000');
 expect(svg).toContain('>15.000</text>');expect(svg).toContain('>20.000</text>');
});
it('renders known quantities, central strike and estimated dealer label without inventing inventory',()=>{
 const svg=strikeHeatmap([{strike:750,call:100,put:null,y:120}],520,'volume',[],false,100);
 expect(svg).toContain('750.00');expect(svg).toContain('PUT —');expect(svg).toContain('muestra parcial');expect(svg).toContain('POSICIONAMIENTO ESTIMADO');expect(svg).not.toContain('NaN');
});
it('missing and invalid values remain unknown; empty ranges show pending',()=>{
 expect(strikeHeatmap([],520,'gex',[],false,1)).toContain('Sin GEX verificable');
 const svg=strikeHeatmap([{strike:10,call:NaN,put:-5,y:120}],520,'oi',[],false,NaN);
 expect(svg).toContain('PUT — · CALL —');expect(svg).not.toContain('NaN');
});
it('caps heat widths and keeps dealer section inside existing chart height',()=>{
 const svg=strikeHeatmap([{strike:10,call:1e6,put:0,y:120}],480,'volume',[],true,1);
 expect(svg).toContain('width="112"');expect(svg).toContain('y="392"');expect(svg).toContain('height="57"');
});
it('renders 100-strike profiles quickly',()=>{
 const rows=Array.from({length:100},(_,i)=>({strike:100+i,call:i*10,put:i,y:90+i*3}));
 const start=performance.now();for(let i=0;i<500;i++)strikeHeatmap(rows,520,'gex',[],true,1000);
 const ms=(performance.now()-start)/500;console.info(`Heatmap 100 strikes: ${ms.toFixed(3)} ms/render`);expect(ms).toBeLessThan(20);
});
