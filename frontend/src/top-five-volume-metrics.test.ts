import {it,expect} from 'vitest';
import {topFiveVolumeMetrics} from './top-five-volume-metrics';
it('formats independent volume context and observed zero changes',()=>{
 const html=topFiveVolumeMetrics({notionalUsd:1000000,change10Pct:0,change30Pct:-25,rvolPremarket:2,rvolSessions:5});
 expect(html).toContain('Monto negociado aprox.');expect(html).toContain('0.0%');expect(html).toContain('-25.0%');expect(html).toContain('2.00×');expect(html).toContain('5/5 sesiones');
});
it('does not manufacture values for missing invalid or insufficient history',()=>{
 expect(topFiveVolumeMetrics(undefined).match(/<dd>Sin datos<\/dd>/g)).toHaveLength(4);
 const html=topFiveVolumeMetrics({notionalUsd:NaN,change10Pct:Infinity,change30Pct:null,rvolPremarket:9,rvolSessions:2});
 expect(html.match(/<dd>Sin datos<\/dd>/g)).toHaveLength(4);expect(html).not.toContain('9.00×');
});
