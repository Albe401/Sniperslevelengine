import {describe,it,expect} from 'vitest';
import {horizonCards,quickLevels,assetAlignment,compactStructuralHorizons} from './visual-readings';
it('shows directional alignment without turning unavailable or stale data into mixed',()=>{
 const a={status:'FULL ALIGNMENT',side:'CALL',callCount:3,putCount:0,neutralCount:0,unavailableCount:0};
 expect(assetAlignment(true,a)).toContain('ALINEADO CALL');
 expect(assetAlignment(true,{...a,side:'PUT'})).toContain('ALINEADO PUT');
 expect(assetAlignment(true,{...a,status:'PARTIAL ALIGNMENT'})).toContain('ALINEADO CALL · PARCIAL');
 expect(assetAlignment(true,{...a,status:'MIXED',side:'NEUTRAL'})).toContain('>MIXTO<');
 expect(assetAlignment(true,{...a,status:'INSUFFICIENT DATA'})).toContain('DATOS INSUFICIENTES');
 expect(assetAlignment(false,a)).not.toContain('ALINEADO CALL');
});
import type {Horizon} from './live';
const row={mode:'DAY',label:'Hoy',expirations:['2026-09-25'],exposure:{spot:739.41,gexComplete:true,scanComplete:true,gammaRegime:'NEGATIVE GAMMA',zeroGamma:740.34},structure:{complete:true,reading:{available:true,side:'CALL',reading:'Alcista',reason:'<script>unsafe</script>',callScore:60,putScore:40},magnet:740,callWall:{strike:745},putWall:{strike:736},routes:[]}} as unknown as Horizon;
describe('Lecturas visuales',()=>{
 it('verifica los tres horizontes antes de afirmar alineación estructural',()=>{
  const horizons=['DAY','WEEK','NEXT4'].map(mode=>({...row,mode}));
  expect(compactStructuralHorizons(horizons,true)).toContain('Alineados estructuralmente · CALL');
  expect(compactStructuralHorizons(horizons,false)).toContain('Alineación pendiente');
  expect(compactStructuralHorizons(horizons.slice(0,2),true)).toContain('Alineación pendiente');
  const mixed=horizons.map(h=>h.mode==='WEEK'?{...h,structure:{...h.structure,reading:{...h.structure.reading,side:'PUT'}}}:h);
  expect(compactStructuralHorizons(mixed,true)).toContain('Sin alineación estructural');
  const incomplete=horizons.map(h=>({...h,structure:{...h.structure,complete:false}}));
  expect(compactStructuralHorizons(incomplete,true)).toContain('Alineación pendiente');
 });
 it('mantiene niveles completos aunque falten griegas para puntuar y usa M1 como imán principal',()=>{
  const h={...row,exposure:{...row.exposure,gexComplete:false},structure:{...row.structure,selected:{M1:739.75},magnet:742.25,reading:{...row.structure.reading,available:false,reason:'Cobertura estructural incompleta'}}};
  const html=horizonCards([h],true);
  expect(html).toContain('745.00');expect(html).toContain('739.75');expect(html).not.toContain('742.25');expect(html).not.toContain('60.00');expect(html).toContain('GEX incompleto');
 });
 it('mantiene las tres tarjetas sin fabricar valores cuando faltan datos',()=>{const html=horizonCards(undefined,false);expect(html.match(/class="panel horizon-card"/g)).toHaveLength(3);expect(html).not.toContain('50.00');expect(html).toContain('Esperando datos');});
 it('oculta niveles y puntuaciones caducadas en ambas vistas',()=>{for(const html of [horizonCards([row],false),quickLevels([row],false)]){expect(html).not.toContain('745.00');expect(html).not.toContain('60.00');expect(html).not.toContain('NEGATIVE GAMMA');}});
 it('muestra referencias vigentes sin confundir puntuación con presión',()=>{expect(quickLevels([row],true)).toContain('745.00');expect(quickLevels([row],true)).not.toContain('Puntuación CALL / 100');expect(horizonCards([row],true)).toContain('&lt;script&gt;');});
 it('no sustituye DAY ausente con otro horizonte',()=>{expect(quickLevels([{...row,mode:'NEXT4'}],true)).not.toContain('745.00');});
});
