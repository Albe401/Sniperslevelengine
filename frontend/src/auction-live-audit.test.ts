import {beforeEach} from 'vitest';
import {auctionDisplayMemory} from './auction-display-memory';
beforeEach(()=>auctionDisplayMemory.clear());
import {it,expect} from 'vitest';
import {auctionMapPanel,type AuctionReading} from './auction-map-panel';
import {auctionVolumeProfile} from './auction-volume-profile';
const now=Date.parse('2026-10-12T15:00:00Z');
function live():AuctionReading{const b={low:746.70,high:746.75,volume:100,brackets:1,volumeRatio:1,timeRatio:1,kind:'transition'};return {version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:now,observedSinceMs:now-60000,provisional:true,acceptedTrades:2,totalVolume:100,pocLow:b.low,pocHigh:b.high,val:b.low,vah:b.high,bins:[b],reaction:'pending',routes:[],calibrated:false};}
const historical=()=>({symbol:'QQQ',date:'2026-10-09',close:751.24,reading:{...live(),version:'auction-map-bars-reference-v1',asOfMs:now-3*86400000,acceptedTrades:0}});
it('retains the current-day profile after the 15-second freshness limit without history substitution',()=>{
 const r=live(),ref=historical();
 expect(auctionMapPanel('QQQ',true,now+15000,r,746.72,ref)).toContain('Perfil observado · parcial');
 const stale=auctionMapPanel('QQQ',true,now+15001,r,746.72,ref);
 expect(stale).toContain('Perfil del día guardado');expect(stale).toContain('sin vigencia');expect(stale).not.toContain('Cierre 751.24');expect(stale).toContain('Participación 100% · guardada');
 expect(auctionMapPanel('QQQ',true,now+15001,{...r,asOfMs:now+15001},746.72,ref)).toContain('Perfil observado · parcial');
});
it('keeps current-day display memory when the native profile disappears',()=>{
 auctionMapPanel('QQQ',true,now,live(),746.72);
 expect(auctionMapPanel('QQQ',true,now+1,undefined,746.72,historical())).toContain('Perfil del día guardado');
 expect(auctionMapPanel('QQQ',false,now+1,undefined,null,historical())).toContain('desconectado');
});
it('does not borrow historical or live evidence from another asset',()=>{
 const r=live();expect(auctionMapPanel('IWM',true,now,r,278,historical())).toContain('Pendiente de perfil');
 expect(auctionMapPanel('QQQ',true,now,{...r,asOfMs:now+1},746.72)).toContain('Pendiente de perfil');
});
it('renders initial percentages within bounds without presenting acceptance or high confidence',()=>{
 const html=auctionMapPanel('QQQ',true,now,live(),746.72);
 const values=[...html.matchAll(/aria-valuenow="([0-9]+)"/g)].map(m=>Number(m[1]));
 expect(values).toEqual([100,100,100]);expect(html).toContain('Participación relativa 100% · inicial');
 expect(html).not.toContain('100% · Alta');expect(html).not.toContain('Aceptación 100%');
});
it('draws adjacent observed intervals touching while keeping true zero-volume gaps empty',()=>{
 const r=live(),b=r.bins[0];r.bins=[b,{...b,low:746.75,high:746.80,volume:50},{...b,low:746.80,high:746.85,volume:0},{...b,low:746.85,high:746.90,volume:20}];
 const html=auctionVolumeProfile(r,746.72),bars=[...html.matchAll(/style="top:([0-9.]+)%;height:([0-9.]+)%"/g)].map(m=>({top:+m[1],height:+m[2]}));
 expect(bars).toHaveLength(3);expect(Math.abs(bars[1].top+bars[1].height-bars[0].top)).toBeLessThan(.001);
 expect(bars[2].top+bars[2].height).toBeLessThan(bars[1].top);expect(html).toContain('Precio');expect(html).not.toContain('height:min(6px');
});

it('never retains current rejection or routes after losing publication and clears display memory on a new ET day',()=>{
 const r={...live(),reaction:'upper-rejection',routes:[{direction:1,destinationLow:747,destinationHigh:747.05,invalidationLevel:746.7,invalidation:'Cerrar debajo',trigger:'Salida',probability:null}]};
 auctionMapPanel('QQQ',true,now,r,746.72);
 const retained=auctionMapPanel('QQQ',true,now+16000,undefined,746.72,historical());
 expect(retained).not.toContain('Rechazo observado');expect(retained).not.toContain('Posible destino 747.00');expect(retained).toContain('Destino pendiente');
 const nextDay=auctionMapPanel('QQQ',true,now+86400000,undefined,746.72,historical());expect(nextDay).not.toContain('Perfil del día guardado');expect(nextDay).toContain('Referencia histórica estimada');
});
