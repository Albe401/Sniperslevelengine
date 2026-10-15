import {afterEach,expect,it,vi} from 'vitest';
import {auctionProfileRange,moveAuctionProfile,resetAuctionProfile,bindAuctionProfileNavigation} from './auction-profile-navigation';
import {auctionVolumeProfile} from './auction-volume-profile';
import type {AuctionReading} from './auction-map-panel';
afterEach(()=>{resetAuctionProfile();vi.unstubAllGlobals();});
it('pans higher on upward wheel or downward drag and lower on downward wheel',()=>{
 const fit={lo:100,hi:110};
 expect(moveAuctionProfile('QQQ',fit,.2)).toEqual({lo:102,hi:112});
 expect(moveAuctionProfile('QQQ',fit,-.3)).toEqual({lo:99,hi:109});
 expect(moveAuctionProfile('QQQ',fit,100)).toEqual({lo:105,hi:115});
});
it('retains the manual window across ticks, separates companies and resets to all data',()=>{
 const fit={lo:100,hi:110};moveAuctionProfile('QQQ:day',fit,.2);
 expect(auctionProfileRange('QQQ:day',{lo:99,hi:113})).toEqual({lo:102,hi:112});
 expect(auctionProfileRange('NVDA:day',fit)).toEqual(fit);
 expect(auctionProfileRange('QQQ:next-day',fit)).toEqual(fit);
 resetAuctionProfile('QQQ:day');expect(auctionProfileRange('QQQ:day',fit)).toEqual(fit);
});
it('expands around the chosen center if regrouping needs more room to keep rows thin',()=>{
 const fit={lo:100,hi:110};moveAuctionProfile('QQQ',fit,.2);
 expect(auctionProfileRange('QQQ',fit,20)).toEqual({lo:97,hi:117});
 expect(moveAuctionProfile('QQQ',fit,NaN)).toEqual({lo:102,hi:112});
});
it('switching between dense and sparse companies never stretches any bar beyond 1.25 percent',()=>{
 for(const [symbol,count] of [['QQQ',201],['NVDA',3],['IWM',1]] as const){
  const bins=Array.from({length:count},(_,i)=>({low:100+i*.01,high:100+(i+1)*.01,volume:i+1,brackets:1,volumeRatio:.5,timeRatio:1,kind:'transition'}));
  const r={symbol,version:'auction-map-ticks-v1',asOfMs:Date.UTC(2026,9,12,17),provisional:true,acceptedTrades:count,totalVolume:bins.reduce((s,b)=>s+b.volume,0),pocLow:bins.at(-1)!.low,pocHigh:bins.at(-1)!.high,val:100,vah:bins.at(-1)!.high,bins,reaction:'pending',routes:[],calibrated:false} as AuctionReading;
  const before=JSON.stringify(r),html=auctionVolumeProfile(r,100);
  const heights=[...html.matchAll(/height:([\d.]+)%/g)].map(m=>Number(m[1]));
  expect(heights.length).toBeGreaterThan(0);expect(Math.max(...heights)).toBeLessThanOrEqual(1.2501);
  expect(JSON.stringify(r)).toBe(before);expect(html).toContain('Arrastra o usa la rueda');
 }
});

it('captures drag inside the plot, releases refresh suppression on cancellation and prevents wheel page scrolling',()=>{
 const layers=[{style:{transform:''}},{style:{transform:''}}],captures=new Set<number>();
 const plot={dataset:{auctionProfileKey:'QQQ',fitLo:'100',fitHi:'110',viewLo:'100',viewHi:'110'},clientHeight:400,
  querySelectorAll:()=>layers,closest:()=>({querySelectorAll:()=>[]}),classList:{add:vi.fn(),remove:vi.fn()},
  setPointerCapture:(id:number)=>captures.add(id),releasePointerCapture:(id:number)=>captures.delete(id),hasPointerCapture:(id:number)=>captures.has(id)
 } as unknown as HTMLElement;
 vi.stubGlobal('document',{querySelectorAll:()=>[plot]});const refresh=vi.fn(),busy=vi.fn();bindAuctionProfileNavigation(refresh,busy);
 const pointer={button:0,isPrimary:true,pointerId:1,clientY:100,preventDefault:vi.fn(),stopPropagation:vi.fn()} as unknown as PointerEvent;
 plot.onpointerdown?.call(plot,pointer);expect(captures.has(1)).toBe(true);expect(busy).toHaveBeenLastCalledWith(true);
 plot.onpointermove?.call(plot,{...pointer,clientY:180} as PointerEvent);expect(layers[0].style.transform).toBe('translateY(80px)');expect(auctionProfileRange('QQQ',{lo:100,hi:110})).toEqual({lo:102,hi:112});
 plot.onpointercancel?.call(plot,pointer);expect(captures.has(1)).toBe(false);expect(busy).toHaveBeenLastCalledWith(false);expect(refresh).toHaveBeenCalledTimes(1);
 const wheel={deltaY:40,deltaMode:0,preventDefault:vi.fn(),stopPropagation:vi.fn()} as unknown as WheelEvent;
 plot.onwheel?.call(plot,wheel);expect(wheel.preventDefault).toHaveBeenCalled();expect(wheel.stopPropagation).toHaveBeenCalled();expect(refresh).toHaveBeenCalledTimes(2);
});
