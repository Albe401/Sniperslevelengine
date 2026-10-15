import {describe,it,expect} from 'vitest';
import {sectorActivityPanel} from './sector-activity';
import type {Scanner} from './scanner';
const data:Scanner={rows:[],top:[],total:75,historicalReady:75,connected:true,sectorActivity:{barEndMs:1000,sectors:[{name:'TECNOLOGIA',expected:12,observed:2,notional:3000,sharePct:100,aboveVwapPct:100,vwapObserved:2,meanRvol:2,rvolObserved:2,leaders:['AAPL','<script>']}]}};
describe('sector activity',()=>{
 it('shows joint rotation and hides stale conclusions',()=>{
  const copy=structuredClone(data);
  copy.sectorActivity!.rotation=[{minutes:10,ready:true,reason:'Comparable',coveragePct:100,sectors:[{name:'ENERGIA',currentUsd:2000,previousUsd:1000,changeUsd:1000,sharePct:60,changePp:10,observed:5,expected:5,state:'GANANDO CONCENTRACIÓN'},{name:'TECNOLOGIA',currentUsd:1000,previousUsd:2000,changeUsd:-1000,sharePct:40,changePp:-10,observed:12,expected:12,state:'PERDIENDO CONCENTRACIÓN'}]}];
  const html=sectorActivityPanel(copy,true,1000,1000);
  expect(html).toContain('Ganan: ENERGIA');expect(html).toContain('Pierden: TECNOLOGIA');expect(html).toContain('+10.0 pp');expect(html).toContain('-10.0 pp');
  expect(sectorActivityPanel(copy,false,1000,1000)).not.toContain('Ganan:');
  copy.sectorActivity!.rotation[0].ready=false;
  expect(sectorActivityPanel(copy,true,1000,1000)).not.toContain('+10.0 pp');
  copy.sectorActivity!.rotation[0].coveragePct=60;
  expect(sectorActivityPanel(copy,true,1000,1000)).toContain('Cobertura comparable: 60%');
  copy.sectorActivity!.rotation[0].ready=true;
  copy.sectorActivity!.rotation[0].sectors[0].state='SIN ROTACIÓN CLARA';
  copy.sectorActivity!.rotation[0].sectors[0].changeUsd=-100;
  const neutral=sectorActivityPanel(copy,true,1000,1000);
  expect(neutral).not.toContain('Ganan: ENERGIA');
  expect(neutral).toContain('class="rotation-neutral">+10.0 pp');
 });
 it('renders coverage, evidence and escaped leaders',()=>{
  const html=sectorActivityPanel(data,true,1000,1000);
  expect(html).toContain('100.0%');expect(html).toContain('2/12 activos');
  expect(html).toContain('&lt;script&gt;');expect(html).not.toContain('<script>');
  expect(html).toContain('No mide entradas netas');
 });
 it('hides values on disconnect, stale publication, expired or future bars',()=>{
  for(const html of [sectorActivityPanel(data,false,1000,1000),sectorActivityPanel({...data,connected:false},true,1000,1000),sectorActivityPanel(data,true,1000,5000),sectorActivityPanel(data,true,122000,122000),sectorActivityPanel(data,true,999,999)]){
   expect(html).not.toContain('100.0%');expect(html).not.toContain('AAPL');expect(html).toContain('0/12 activos');
  }
 });
 it('keeps unavailable metrics missing',()=>{
  const copy=structuredClone(data);copy.sectorActivity!.sectors[0].meanRvol=null;copy.sectorActivity!.sectors[0].aboveVwapPct=null;
  expect(sectorActivityPanel(copy,true,1000,1000)).not.toContain('2.0×');
 });
});
