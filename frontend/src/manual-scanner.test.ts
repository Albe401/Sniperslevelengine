import {describe,it,expect} from 'vitest';
import {manualScannerContent,type ManualReport} from './manual-scanner';
const now=Date.parse('2026-09-28T11:30:00Z');
const report=():ManualReport=>({date:'2026-09-28',startMs:Date.parse('2026-09-28T08:00:00Z'),cutoffMs:now,total:75,processed:75,succeeded:75,empty:0,errors:[],complete:true,candidates:Array.from({length:7},(_,i)=>({symbol:`A${i}`,sector:'Tecnología',volume:100+i,price:25,changePct:i-2,lastBarMs:now,bars:10}))});
describe('manual scanner',()=>{
 it('renders native options for each of the five manual results without Neuro',()=>{
  const html=manualScannerContent(report(),false,'',true,now);
  expect(html.match(/OPCIONES · SNIPERS NATIVO/g)).toHaveLength(5);
  expect(html).not.toContain('CONTEXTO TEÓRICO DE DEALERS');
  expect(html).toContain('Pendiente de consulta nativa');
 });
 it('offers early search without a live connection and prevents duplicate clicks while busy',()=>{
  expect(manualScannerContent(undefined,false,'',true,now)).toContain('07:30');
  expect(manualScannerContent(undefined,false,'',true,now)).toContain('data-manual-scan >Buscar ahora');
  expect(manualScannerContent(undefined,true,'',true,now)).toContain('data-manual-scan disabled');
  expect(manualScannerContent(undefined,false,'',false,now)).toContain('data-manual-scan disabled');
 });
 it('limits five sorted share-volume candidates and labels the snapshot cutoff',()=>{
  const html=manualScannerContent(report(),false,'',true,now);
  expect(html.match(/class="manual-scan-card"/g)).toHaveLength(5);
  expect(html.indexOf('A6')).toBeLessThan(html.indexOf('A5'));
  expect(html).not.toContain('<b>A0</b>');expect(html).toContain('07:30 ET');
  expect(html).toContain('no cotización en vivo');expect(html).toContain('No cambia la selección automática');
 });
 it('makes incomplete and previous-session results explicit and escapes provider errors',()=>{
  const r=report();r.complete=false;r.succeeded=4;r.processed=5;r.errors=['<script>'];r.candidates[0].symbol='<img>';
  const html=manualScannerContent(r,false,'<error>',true,now+86400000);
  expect(html).toContain('sesión anterior');expect(html).toContain('Resultado parcial');expect(html).toContain('4 / 75');
  expect(html).toContain('&lt;script&gt;');expect(html).not.toContain('<script>');expect(html).toContain('&lt;error&gt;');
 });
});

it('adds volume context without ranking by dollars or RVOL',()=>{const r=report();r.candidates[0]={...r.candidates[0],volumeMetrics:{notionalUsd:999999999,change10Pct:500,change30Pct:900,rvolPremarket:99,rvolSessions:5}};r.candidates[6]={...r.candidates[6],volumeMetrics:{notionalUsd:100,change10Pct:-10,change30Pct:20,rvolPremarket:2,rvolSessions:5}};const html=manualScannerContent(r,false,'',true,now);expect(html).not.toContain('<b>A0</b>');expect(html).toContain('Monto negociado aprox.');expect(html).toContain('2.00×');expect(html).toContain('-10.0%');});
