import {beforeEach} from 'vitest';
import {auctionDisplayMemory} from './auction-display-memory';
beforeEach(()=>auctionDisplayMemory.clear());
import {describe,it,expect} from 'vitest';
import {acceptanceIntensity,levelIntensity,rejectionEvidence,auctionMapPanel,quickMapStack,type AuctionReading} from './auction-map-panel';
const now=1_800_000;
function fixture():AuctionReading{return {version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:now,provisional:true,acceptedTrades:10,totalVolume:1000,pocLow:101,pocHigh:102,val:101,vah:103,calibrated:false,reaction:'upper-rejection',routes:[],bins:[{low:99,high:100,volume:200,brackets:2,volumeRatio:.5,timeRatio:1,kind:'acceptance'},{low:105,high:106,volume:200,brackets:2,volumeRatio:.5,timeRatio:1,kind:'acceptance'}]};}
it('distinguishes loading RTH history, recovered opening coverage and recovery failures',()=>{
 const r=fixture();r.sessionHistory={status:'loading',fromOpen:false,throughMs:null,pages:5,rows:250000,liveProvisional:true};
 expect(auctionMapPanel('QQQ',true,now,r,102)).toContain('Recuperando sesión RTH · 5 páginas · perfil parcial');
 r.sessionHistory={...r.sessionHistory,status:'ready',fromOpen:true,throughMs:now-1000};
 expect(auctionMapPanel('QQQ',true,now,r,102)).toContain('Perfil RTH desde apertura · REST hasta');
 expect(auctionMapPanel('QQQ',true,now,r,102)).toContain('en vivo provisional');
 r.sessionHistory={...r.sessionHistory,status:'Acceso REST rechazado',fromOpen:false,throughMs:null};
 expect(auctionMapPanel('QQQ',true,now,r,102)).toContain('Perfil parcial · Acceso REST rechazado');
});
describe('Live auction map quality and layout',()=>{
 it('identifies the live partial observation start instead of claiming a full session',()=>{
  const r=fixture();r.observedSinceMs=now-60000;
  const html=auctionMapPanel('QQQ',true,now,r,102.5);
  expect(html).toContain('Perfil observado · parcial · desde');expect(html).not.toContain('Referencia histórica estimada');
 });
 it('scores value borders and POC independently and highlights only confirmed rejection',()=>{
  const r=fixture();r.bins.push({low:101,high:102,volume:500,brackets:3,volumeRatio:1,timeRatio:1,kind:'other'},{low:102,high:103,volume:100,brackets:1,volumeRatio:.2,timeRatio:.2,kind:'other'});
  const html=auctionMapPanel('QQQ',true,now,r,102.5);
  expect(html.match(/data-level-strength=/g)).toHaveLength(6);
  expect(html).toContain('Participación 100% · Alta');expect(html).toContain('Participación 20% · Baja');expect(html).toContain('Rechazo observado');
  expect(html).toMatch(/rejection pending" data-horizon-details/);
  expect(levelIntensity({...r.bins[2],timeRatio:NaN})).toBeUndefined();
 });
 it('intensifies acceptance only when comparable volume and time both increase',()=>{
  const b=fixture().bins[0],low=acceptanceIntensity({...b,volumeRatio:.5,timeRatio:.5})!,mid=acceptanceIntensity({...b,volumeRatio:.7,timeRatio:.7})!,high=acceptanceIntensity({...b,volumeRatio:.95,timeRatio:.95})!;
  expect([low.label,mid.label,high.label]).toEqual(['Baja','Media','Alta']);expect(low.fill).toBeLessThan(mid.fill);expect(mid.fill).toBeLessThan(high.fill);expect(low.edge).toBeLessThan(high.edge);
  expect(acceptanceIntensity({...b,volumeRatio:1,timeRatio:.5})!.label).toBe('Media');
  for(const bad of [{...b,timeRatio:NaN},{...b,volumeRatio:1.1},{...b,brackets:1},{...b,kind:'untraded'}])expect(acceptanceIntensity(bad)).toBeUndefined();
 });
 it('gives the stronger observed acceptance a stronger green and leaves pending slots unscored',()=>{
  const r=fixture();r.bins[0].volumeRatio=.5;r.bins[0].timeRatio=.5;r.bins[1].volumeRatio=.95;r.bins[1].timeRatio=.95;
  const html=auctionMapPanel('QQQ',true,now,r,102.5);expect(html).toContain('Aceptación 95% · Alta');expect(html).toContain('Aceptación 50% · Baja');expect(html.match(/data-acceptance-strength=/g)).toHaveLength(2);
  const saved=auctionMapPanel('QQQ',false,now);expect(saved).toContain('desconectado');expect(saved).toContain(' · guardada');expect(saved).not.toContain('Rechazo observado');expect(html).toContain('no mide probabilidad');
 });
 it('has seven semantic references without sample prices when evidence is missing',()=>{
  const html=auctionMapPanel('QQQ',true,now);expect(html.match(/class="auction-map-node/g)).toHaveLength(7);expect(html).toContain('Pendiente de perfil del subyacente');expect(html).not.toContain('487,');expect(html).not.toContain('489,');
 });
 it('shows native profile and only actually observed rejection',()=>{
  const html=auctionMapPanel('QQQ',true,now,fixture(),102.5);expect(html).toContain('Perfil observado · parcial');expect(html).toContain('99.00–100.00');expect(html).toContain('105.00–106.00');expect(html).toContain('class="auction-map-node rejection"');expect(html).toContain('class="auction-map-node rejection pending"');expect(html).toContain('Sin porcentajes de llegada calibrados');
 });
 it('rejects stale, future, disconnected and foreign-symbol readings',()=>{
  for(const r of [{...fixture(),asOfMs:now-15001},{...fixture(),asOfMs:now+1},{...fixture(),symbol:'SPY'}])expect(auctionMapPanel('QQQ',true,now,r)).not.toContain('99.00–100.00');expect(auctionMapPanel('QQQ',false,now,fixture())).not.toContain('99.00–100.00');
 });
 it('shows a real conditional destination with explicit invalidation',()=>{
  const r=fixture();r.routes=[{direction:-1,destinationLow:99,destinationHigh:100,invalidationLevel:103,trigger:'Mantener salida',invalidation:'Cerrar sobre VAH',probability:null}];const html=auctionMapPanel('QQQ',true,now,r);expect(html).toContain('Posible destino 99.00–100.00');expect(html).toContain('Cerrar sobre VAH: 103.00');
 });
 it('rejects invalid numerical evidence and does not publish probability fields',()=>{
  const r=fixture();r.val=NaN;expect(auctionMapPanel('QQQ',true,now,r)).toContain('Pendiente de perfil');const t=fixture();t.routes=[{direction:1,destinationLow:105,destinationHigh:106,invalidationLevel:103,invalidation:'Cerrar bajo',trigger:'Salida',probability:.8 as unknown as null}];expect(auctionMapPanel('QQQ',true,now,t)).toContain('Pendiente de perfil');
 });
 it('keeps the auction map above the structural map without a dealer header',()=>{
  const html=quickMapStack('<section>MAPA ESTRUCTURAL</section>',auctionMapPanel('QQQ',false,now));expect(html.indexOf('MAPA ESTRUCTURAL')).toBeGreaterThan(html.indexOf('MAPA DE ACEPTACIÓN'));expect(html).toContain('PERFIL DE VOLUMEN');for(const label of ['ACTIVIDAD INUSUAL','CONTROL INTRADÍA','Control de dealers','Engranaje','EXTENSIÓN DEL PRECIO'])expect(html).not.toContain(label);
 });
});
it('shows participation percentage meters only for supported acceptance bins',()=>{
 const now=1800000;
 const r={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:now,provisional:true,acceptedTrades:10,totalVolume:1000,pocLow:101,pocHigh:102,val:101,vah:103,calibrated:false,reaction:'pending',routes:[],bins:[{low:105,high:106,volume:200,brackets:2,volumeRatio:.8,timeRatio:.6,kind:'acceptance'}]} as AuctionReading;
 const html=auctionMapPanel('QQQ',true,now,r,102.5);expect(html).toContain('Aceptación 69%');expect(html).toContain('aria-valuenow="69"');expect(html).toContain('width:69%');expect(html.match(/role="meter"/g)).toHaveLength(1);expect(auctionMapPanel('QQQ',false,now)).toContain('Aceptación 69% · guardada');
});
it('publishes all seven meters when native rejection and participation evidence exist',()=>{
 const r=fixture();r.bins.push({low:101,high:102,volume:500,brackets:3,volumeRatio:1,timeRatio:1,kind:'other'},{low:102,high:103,volume:100,brackets:1,volumeRatio:.2,timeRatio:.2,kind:'other'});
 const evidence={version:'auction-rejection-index-v1',symbol:'QQQ',status:'observed',score:80,distance:.8,permanence:.8,participation:.8,level:103,windowStartMs:now-300000,windowEndMs:now,calibrated:false,probability:null} as const;
 r.rejectionUpper=evidence;r.rejectionLower={...evidence,level:101};const html=auctionMapPanel('QQQ',true,now,r,102.5);expect(html.match(/role="meter"/g)).toHaveLength(7);expect(html.match(/Fuerza de rechazo 80%/g)).toHaveLength(2);expect(html).toContain('Participación 100%');expect(html).toContain('Participación 20%');
 for(const bad of [{...evidence,symbol:'SPY'},{...evidence,windowEndMs:now+1},{...evidence,windowStartMs:1,windowEndMs:now-300001},{...evidence,score:90},{...evidence,status:'invalidated'}])expect(rejectionEvidence(bad,'QQQ',now)).toBeUndefined();
});

it('shows relative percentages with an initial-sample label without claiming high confidence',()=>{
 const b={low:278.85,high:278.90,volume:3623,kind:'transition',brackets:1,volumeRatio:1,timeRatio:1};
 const r={...fixture(),symbol:'IWM',bins:[b],totalVolume:3623,pocLow:b.low,pocHigh:b.high,val:b.low,vah:b.high,reaction:'pending'};
 const html=auctionMapPanel('IWM',true,now,r,278.86);
 expect(html).toContain('Participación relativa 100% · inicial');expect(html).not.toContain('100% · Alta');expect(html.match(/role="meter"/g)).toHaveLength(3);
});
