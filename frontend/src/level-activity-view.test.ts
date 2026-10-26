import {expect,it} from 'vitest';
import type {LevelActivity,LevelActivityMetric} from './level-activity';
import {activityChips,activityStyle} from './level-activity-view';

const metric=(value:number|null,label='Ventas',intensity=1,max=true,tied=false):LevelActivityMetric=>({value,label,intensity,max,tied,detail:'Muestra observada con cobertura verificable'});
const frame=():LevelActivity=>({price:100,captured:false,acceptance:metric(.75,'Media'),stockSales:metric(25000),optionSales:metric(12500)});
const variables=(read:LevelActivity|undefined)=>Object.fromEntries(activityStyle(read).slice(7,-1).split(';').map(s=>{const [name,value]=s.split(':');return [name,Number(value)];}));

it('keeps missing and zero sources neutral even if their maximum flag is malformed',()=>{
 expect(Object.values(variables(undefined))).toEqual(Array(6).fill(0));
 expect(activityChips(undefined).match(/is-pending/g)).toHaveLength(3);
 const r=frame();r.acceptance=metric(null,'Sin lectura');r.stockSales=metric(0);r.optionSales=metric(0);
 expect(Object.values(variables(r))).toEqual(Array(6).fill(0));
 expect(activityChips(r)).not.toContain('★');expect(activityChips(r)).not.toContain('is-max');
});

it('preserves the supplied AMT category without introducing a percentage or thresholds',()=>{
 const r=frame();r.acceptance=metric(.55,'Alta');
 const html=activityChips(r);
 expect(html).toContain('>Alta</b>');expect(html).not.toContain('55%');
 expect(html).toContain('índice descriptivo (0–1), no calibrado');expect(html).toContain('no probabilidad');
});

it('keeps source opacities independent and emits only finite bounded numeric CSS values',()=>{
 const r=frame(),before=variables(r);r.stockSales=metric(5,'Ventas',.1,false);
 const after=variables(r);
 expect(after['--activity-stock-fill']).toBeLessThan(before['--activity-stock-fill']);
 for(const kind of ['acceptance','option'])for(const suffix of ['fill','edge'])expect(after[`--activity-${kind}-${suffix}`]).toBe(before[`--activity-${kind}-${suffix}`]);
 r.acceptance=metric(NaN,'Alta',Infinity);r.stockSales=metric(-1,'Ventas',1);r.optionSales=metric(5,'Ventas',100);
 const bounded=variables(r);
 expect(bounded['--activity-acceptance-fill']).toBe(0);expect(bounded['--activity-stock-fill']).toBe(0);
 expect(bounded['--activity-option-fill']).toBe(.26);expect(bounded['--activity-option-edge']).toBe(.9);
 expect(Object.values(bounded).every(n=>Number.isFinite(n)&&n>=0&&n<=1)).toBe(true);
 expect(activityStyle(r)).not.toContain('--node-color');
 expect(activityStyle(r)).toMatch(/^style="(?:--activity-(?:acceptance|stock|option)-(?:fill|edge):\d\.\d{4};?){6}"$/);
});

it('retains compact amounts, visible units and distinct source windows',()=>{
 const html=activityChips(frame());
 expect(html).toContain('>25K</b>');expect(html).toContain('>acc.</span>');expect(html).toContain('>contr.</span>');
 expect(html).toContain('últimos 60 s');expect(html).toContain('últimos 5 min');expect(html).toContain('AMT · perfil observado');
 expect(html).toContain('25.000 acciones');expect(html.match(/role="img"/g)).toHaveLength(3);
});

it('keeps simultaneous maxima and shared maxima explicit in the capture context',()=>{
 const r=frame();r.captured=true;r.stockSales.tied=true;r.optionSales.tied=true;
 const html=activityChips(r);
 expect(html.match(/is-max/g)).toHaveLength(3);expect(html.match(/is-tied/g)).toHaveLength(2);
 expect(html).toContain('Máximo compartido');expect(html).toContain('en la captura');
 expect(html).toContain('no lectura en vivo');expect(html.match(/★/g)).toHaveLength(3);
});

it('escapes dynamic labels and details in both tooltip and accessible name',()=>{
 const r=frame();r.optionSales.label='"<img src=x onerror=alert(1)>';r.optionSales.detail="<script>bad</script> & 'quoted'";
 const html=activityChips(r);
 expect(html).not.toContain('<img');expect(html).not.toContain('<script>');
 expect(html).toContain('&quot;&lt;img');expect(html).toContain('&lt;script&gt;bad&lt;/script&gt; &amp; &#39;quoted&#39;');
 expect(html).toContain('aria-label="');
});
