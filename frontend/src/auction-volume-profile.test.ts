import {it,expect} from 'vitest';
import {profileGroups,auctionVolumeProfile,profileDestinations} from './auction-volume-profile';
import type {AuctionBin,AuctionReading} from './auction-map-panel';
it('conserves volume when aggregating a dense price profile',()=>{const bins=Array.from({length:201},(_,i)=>({low:100+i,high:101+i,volume:i+1,kind:'other',brackets:1,volumeRatio:.2,timeRatio:.2} as AuctionBin));const groups=profileGroups(bins);expect(groups.length).toBeLessThanOrEqual(80);expect(groups.reduce((n,b)=>n+b.volume,0)).toBe(bins.reduce((n,b)=>n+b.volume,0));expect(groups[0].low).toBe(100);expect(groups.at(-1)!.high).toBe(301);});
it('uses actual volume, preserves price order and marks POC plus observed acceptance',()=>{const b={low:100,high:101,volume:10,kind:'acceptance',brackets:2,volumeRatio:.5,timeRatio:.5};const r={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:1,provisional:true,acceptedTrades:1,totalVolume:30,calibrated:false,reaction:'pending',bins:[{...b,low:101,high:102,volume:20,kind:'other'},b],pocLow:101,pocHigh:102,val:100,vah:102} as AuctionReading;const html=auctionVolumeProfile(r,100.5,b);expect(html).toContain('width:50.000%');expect(html).toContain('width:100.000%');expect(html).toContain('is-poc');expect(html).toContain('is-acceptance zone-upper');expect(html).toContain('100.00–101.00');expect(html).not.toContain('probabilidad');});
it('keeps unavailable profiles unfilled',()=>{const html=auctionVolumeProfile(undefined,null);expect(html).toContain('Pendiente');expect(html).not.toContain('auction-profile-bin');});
it('darkens only the actual price interval without changing quantitative widths',()=>{const b={low:100,high:101,volume:10,kind:'other',brackets:1,volumeRatio:.2,timeRatio:.2};const r={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:1,provisional:true,acceptedTrades:1,totalVolume:30,calibrated:false,reaction:'pending',bins:[b,{...b,low:101,high:102,volume:20}],pocLow:101,pocHigh:102,val:100,vah:102,routes:[]} as AuctionReading;const html=auctionVolumeProfile(r,100.5);expect(html.match(/is-current/g)).toHaveLength(1);expect(html).toContain('width:50.000%');expect(html).toContain('Destino pendiente');expect(auctionVolumeProfile(r,105)).not.toContain('is-current');});
it('highlights only supplied acceptance destinations ahead of price, without invented probabilities',()=>{const b={low:105,high:106,volume:20,kind:'acceptance',brackets:2,volumeRatio:.8,timeRatio:.8};const route={direction:1,destinationLow:105,destinationHigh:106,probability:null,invalidationLevel:100,trigger:'Salida mantenida',invalidation:'Cerrar debajo'};const r={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:1,provisional:true,acceptedTrades:1,totalVolume:30,calibrated:false,reaction:'pending',bins:[{...b,low:100,high:101,kind:'other'},b],pocLow:105,pocHigh:106,val:100,vah:106,routes:[route]} as AuctionReading;const html=auctionVolumeProfile(r,100.5);expect(html).toContain('is-destination');expect(html).toContain('auction-profile-path up');expect(html).toContain('Destino condicionado ↑');expect(profileDestinations(r,107)).toHaveLength(0);expect(profileDestinations({...r,routes:[{...route,destinationLow:110,destinationHigh:111}]},100.5)).toHaveLength(0);expect(profileDestinations({...r,routes:[{...route,probability:.8 as unknown as null}]},100.5)).toHaveLength(0);});
it('never paints a gap as volume or calls mixed aggregated bins acceptance',()=>{const b={low:100,high:101,volume:10,kind:'acceptance',brackets:2,volumeRatio:.8,timeRatio:.8};const groups=profileGroups([b,{...b,low:101,high:102,kind:'other'},{...b,low:110,high:111}],1);expect(groups).toHaveLength(2);expect(groups[0].high).toBe(102);expect(groups[1].low).toBe(110);expect(groups[0].acceptance).toBe(false);expect(groups[1].acceptance).toBe(true);expect(groups.reduce((s,g)=>s+g.volume,0)).toBe(30);});
it('keeps explicit zero-volume price intervals empty during aggregation',()=>{const b={low:100,high:101,volume:10,kind:'acceptance',brackets:2,volumeRatio:.8,timeRatio:.8};const groups=profileGroups([b,{...b,low:101,high:110,volume:0,kind:'untraded'},{...b,low:110,high:111}],1);expect(groups).toHaveLength(2);expect(groups[0].high).toBe(101);expect(groups[1].low).toBe(110);});

it('keeps a newly observed single bin small and identifies insufficient temporal coverage',()=>{
 const b={low:278.85,high:278.90,volume:3623,kind:'transition',brackets:1,volumeRatio:1,timeRatio:1};
 const r={version:'auction-map-ticks-v1',symbol:'IWM',asOfMs:1,provisional:true,acceptedTrades:4,totalVolume:3623,calibrated:false,reaction:'pending',bins:[b],pocLow:b.low,pocHigh:b.high,val:b.low,vah:b.high,routes:[]} as AuctionReading;
 const html=auctionVolumeProfile(r,278.86);
 expect(html).toContain('height:1.2500%');expect(html).toContain('min-height:0;flex:1');expect(html).not.toContain('height:min(6px');expect(html).toContain('Muestra inicial');expect(html).toContain('volumen 3,623 acciones');
 expect(auctionVolumeProfile(r,279.10)).toContain('Precio 279.10');
});

it('fills the level column height while using a wider price scale and conserving relative widths',()=>{
 const b={low:100,high:100.05,volume:100,kind:'transition',brackets:1,volumeRatio:1,timeRatio:1};
 const bins=Array.from({length:100},(_,i)=>({...b,low:100+i*.05,high:100+(i+1)*.05,volume:100+i}));
 const r={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:1,provisional:true,acceptedTrades:100,totalVolume:14950,calibrated:false,reaction:'pending',bins,pocLow:104.95,pocHigh:105,val:100,vah:105,routes:[]} as AuctionReading;
 const html=auctionVolumeProfile(r,102);expect(html).toContain('min-height:0;flex:1');expect(html).not.toContain('height:48px');expect(html).toContain('width:100.000%');expect(html).not.toContain('height:min(6px');
});

it('keeps forty observed cent intervals thin instead of expanding their rows',()=>{
 const bins=Array.from({length:40},(_,i)=>({low:747+i*.01,high:747+(i+1)*.01,volume:100+i,kind:'other',brackets:2,volumeRatio:.5,timeRatio:.5} as AuctionBin));
 const r={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:1,provisional:true,acceptedTrades:40,totalVolume:4780,calibrated:false,reaction:'pending',bins,pocLow:747.39,pocHigh:747.40,val:747,vah:747.40,routes:[]} as AuctionReading;
 const html=auctionVolumeProfile(r,747.20);
 expect(html).toContain('height:1.2500%');
 expect(html).toContain('>747.60</span>');
 expect(html).toContain('>746.80</span>');
 expect(html).not.toContain('>749.');
});

it('keeps a partial multi-bin profile thin when switching to a sparse asset',()=>{
 const bins=Array.from({length:10},(_,i)=>({low:747+i*.01,high:747+(i+1)*.01,volume:100+i,kind:'other',brackets:1,volumeRatio:.5,timeRatio:.5} as AuctionBin));
 const r={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:1,provisional:true,acceptedTrades:10,totalVolume:1045,calibrated:false,reaction:'pending',bins,pocLow:747.09,pocHigh:747.10,val:747,vah:747.10,routes:[]} as AuctionReading;
 const html=auctionVolumeProfile(r,747.05);
 expect(html).toContain('height:1.2500%');
 expect(html).toContain('data-auction-profile-key');
 expect(html).toContain('data-auction-pan="reset"');
 expect(html).not.toContain('is-acceptance');
});
