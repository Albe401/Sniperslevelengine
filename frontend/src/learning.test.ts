import {describe,expect,it} from 'vitest';
import {learningContent,recordingState,type LearningReport} from './learning';
const now=1790258400000;
function report():LearningReport{return {exists:false,connected:false,generatedMs:now,symbols:[],observations:0,eligible:0,sessions:0,buildSessions:0,collector:null,bridge:null,quality:[],outcomes:[],recent:[]};}
describe('learning dashboard',()=>{
 it('shows an honest empty state without invented scores',()=>{
  const html=learningContent(report(),false,'','',now);
  expect(html).toContain('Esperando conexión');expect(html).toContain('Todavía no hay lecturas guardadas');
  expect(html).toContain('0 de 10 sesiones');expect(html).not.toContain('100%');
 });
 it('distinguishes a fresh recorder from stopped and stale sessions',()=>{
  const d=report();d.exists=true;d.connected=true;d.collector={observed_ms:now,recorded:true,eligible:false,symbol:'QQQ',quality:'OUTSIDE_REGULAR_SESSION',error:null};
  expect(recordingState(d,now).label).toBe('Registro activo');expect(recordingState(d,now).detail).toContain('Fuera de sesión');
  expect(recordingState(d,now+76000).label).toBe('Registro en pausa');d.connected=false;
  expect(recordingState(d,now).label).toBe('Registro en pausa');
 });
 it('does not mistake a stale bridge for live context',()=>{
  const d=report();d.connected=true;d.bridge={capturedMs:now-11000,symbol:'QQQ',hasContext:true,quality:'READY',stocksPhase:'streaming',optionsPhase:'streaming'};
  expect(learningContent(d,false,'','',now)).toContain('Sin contexto vigente');
  d.bridge.capturedMs=now;expect(learningContent(d,false,'','',now)).toContain('Contexto vigente');
 });
 it('shows errors and escapes provider text',()=>{
  const d=report();d.symbols=['<img src=x>'];
  const html=learningContent(d,false,'<script>error</script>','',now);
  expect(html).toContain('&lt;script&gt;');expect(html).not.toContain('<img src=x>');expect(html).toContain('última consulta');
 });
 it('keeps pending and absent outcomes distinct from measured zero',()=>{
  const d=report();d.recent=[{id:1,symbol:'QQQ',session:'2026-09-24',observedMs:now,price:500,eligible:true,quality:'READY',side:'CALL',build:'v1',outcomes:[{minutes:5,status:'observed',returnBps:0,outcomeMs:now},{minutes:15,status:'pending',returnBps:null,outcomeMs:null},{minutes:30,status:'missing',returnBps:null,outcomeMs:null}]}];
  const html=learningContent(d,false,'','',now);expect(html).toContain('0.000%');expect(html).toContain('En seguimiento');expect(html).toContain('Sin observación');
 });
 it('does not claim a model is trained when the session minimum is met',()=>{
  const d=report();d.buildSessions=10;
  const html=learningContent(d,false,'','',now);expect(html).toContain('falta verificar muestras');expect(html).toContain('Todavía no hay una mejora validada');
 });
});
