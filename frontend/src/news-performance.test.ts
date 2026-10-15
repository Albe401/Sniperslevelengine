import {it,expect} from 'vitest';
import {readFileSync,writeFileSync} from 'node:fs';
import {newsSections,newsToday} from './investment-news';
import type {RadarNews} from './investment-news';
const dir=new URL('../../../docs/audits/2026-10-13-news-performance/',import.meta.url);
const capture=JSON.parse(readFileSync(new URL('../../../docs/audits/2026-10-13-news-v2/live-news-input.json',import.meta.url),'utf8').replace(/^\uFEFF/,''));
it('measures actual news HTML and New York filtering on the dated real capture',()=>{
 const rows=capture.report.items as RadarNews[];const samples:number[]=[];const dateSamples:number[]=[];let rendered=0;
 for(let i=0;i<8;i++){let start=performance.now();newsToday(rows,capture.capturedMs);dateSamples.push(performance.now()-start);start=performance.now();const html=newsSections(rows);samples.push(performance.now()-start);rendered=(html.match(/class="ir-news-card /g)??[]).length;expect(html).toContain('Noticias');}
 const stats=(values:number[])=>{const sorted=[...values].sort((a,b)=>a-b);return {meanMs:values.reduce((a,b)=>a+b,0)/values.length,p95Ms:sorted[Math.ceil(sorted.length*.95)-1],maxMs:Math.max(...values),samplesMs:values};};
 const out={label:(globalThis as unknown as {process:{env:Record<string,string|undefined>}}).process.env.SNIPERS_NEWS_BENCH_LABEL??('verification-'+Date.now()),source:'dated Massive normalized input, 2026-10-13 news-v2 capture',articles:rows.length,renderedCards:rendered,runs:8,html:stats(samples),nyDateFilter:stats(dateSamples),includesDomLayout:false,thresholdReference:'https://web.dev/articles/optimize-long-tasks; 50ms long task, not a trading criterion'};
 writeFileSync(new URL(out.label+'-frontend.json',dir),JSON.stringify(out,null,2));console.log('NEWS_SPEED',JSON.stringify(out));expect(rows.length).toBe(590);expect(rendered).toBeLessThanOrEqual(40);
},30000);

