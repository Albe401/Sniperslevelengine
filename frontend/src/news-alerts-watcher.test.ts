import {it,expect,vi} from 'vitest';
const mocks=vi.hoisted(()=>({invoke:vi.fn(),sound:vi.fn(()=>true)}));
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:mocks.invoke}));
vi.mock('./alert-sound',()=>({playAlertSound:mocks.sound,enableAlertSound:async()=>true}));
import {pollNewsAlerts,globalNewsAlerts,globalNewsPopup} from './news-alerts';
import {setWatchedNewsSymbols} from './news-watchlist';
import type {RadarNews} from './investment-news';
it('silently baselines all five, queries hidden slots, alerts globally once and follows replacement symbols',async()=>{
 vi.stubGlobal('document',{querySelector:()=>null,dispatchEvent:()=>true});
 const news=(id:string,ticker:string):RadarNews=>({id,ticker,title:'Titular en español '+id,summary:'Noticia',source:'Benzinga',publishedAt:new Date().toISOString(),sentiment:'unknown',importance:'low',horizon:'intraday',translated:true});
 let phase=0;
 mocks.invoke.mockImplementation(async(_cmd:string,args:{ticker:string})=>({fetchedMs:Date.now(),truncated:args.ticker==='',items:args.ticker==='NVDA'?[news('old','NVDA'),...(phase>=1?[news('new','NVDA')]:[])]:args.ticker==='ON'?[news('on-old','ON'),...(phase>=3?[news('on-new','ON')]:[])]:args.ticker==='SONY'?[news('sony','SONY')]:[]}));
 await pollNewsAlerts();expect(mocks.sound).not.toHaveBeenCalled();
 expect(mocks.invoke.mock.calls.map(c=>c[1].ticker)).toEqual(['','QQQ','SPY','GOOGL','NVDA','AAPL']);
 phase=1;await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(1);expect(mocks.sound).toHaveBeenLastCalledWith('bell');
 expect(globalNewsPopup()).toContain('news-watch-popup');expect(globalNewsPopup()).toContain('Titular en español');
 await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(1);
 setWatchedNewsSymbols(['QQQ','SPY','GOOGL','ON','AAPL']);phase=2;await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(1);
 phase=3;await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(2);
 expect(globalNewsAlerts()).toContain('Vigilando: QQQ · SPY · GOOGL · ON · AAPL');
 mocks.invoke.mockImplementation(async(_cmd:string,args:{ticker:string})=>{
  if(args.ticker!=='ON')throw new Error('Consulta temporalmente no disponible');
  return {fetchedMs:Date.now(),truncated:false,items:[news('on-newer','ON')]};
 });
 await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(3);
 expect(globalNewsAlerts()).toContain('5 consultas pendientes');
 mocks.invoke.mockRejectedValue(new Error('Sin conexión'));
 await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(3);
 expect(globalNewsAlerts()).toContain('se reintentará');
 vi.unstubAllGlobals();
});

it.each(['2026-10-10T16:00:00Z','2026-10-11T16:00:00Z'])('polls news on weekend %s without a market session gate',async(date)=>{
 vi.useFakeTimers();vi.setSystemTime(new Date(date));vi.stubGlobal('document',{querySelector:()=>null,dispatchEvent:()=>true});
 mocks.invoke.mockClear();mocks.invoke.mockResolvedValue({fetchedMs:Date.now(),truncated:false,items:[]});
 try{await pollNewsAlerts();expect(mocks.invoke).toHaveBeenCalledWith('get_benzinga_news',{ticker:''});expect(mocks.invoke.mock.calls.length).toBe(1);}finally{vi.useRealTimers();vi.unstubAllGlobals();}
});

it('does not lose a new alert while its translation is pending',async()=>{vi.stubGlobal('document',{querySelector:()=>null,dispatchEvent:()=>true});const count=mocks.sound.mock.calls.length;let translated=false;mocks.invoke.mockImplementation(async(_cmd:string,args:{ticker:string})=>({fetchedMs:Date.now(),truncated:false,items:args.ticker===''?[{id:'pending-translation',ticker:'QQQ',title:'Noticia nueva',summary:'Texto',source:'Benzinga',publishedAt:new Date().toISOString(),sentiment:'unknown',importance:'low',horizon:'intraday',translated}]:[]}));try{await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(count);translated=true;await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(count+1);await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(count+1);}finally{vi.unstubAllGlobals();}});


it('critical original alerts immediately, enriches without duplicate sound, and keeps revised events',async()=>{
 vi.stubGlobal('document',{querySelector:()=>null,dispatchEvent:()=>true});const before=mocks.sound.mock.calls.length;
 let translated=false,revision=1;
 mocks.invoke.mockImplementation(async(cmd)=>cmd==='record_news_alerts'?undefined:({fetchedMs:Date.now(),truncated:false,items:[{id:'critical-v2',eventId:'halt-qqq',revision,ticker:'QQQ',tickerSymbols:['QQQ'],title:translated?'Suspensión de cotización':'Trading Halt',originalTitle:'Trading Halt',originalSummary:revision===1?'Reported':'Correction',summary:revision===1?'Reported':'Correction',source:'Benzinga',publishedAt:new Date().toISOString(),sentiment:'unknown',importance:'high',horizon:'intraday',rawCritical:true,translated}]}));
 try{await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(before+1);expect(globalNewsPopup()).toContain('Trading Halt');translated=true;await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(before+1);expect(globalNewsPopup()).toContain('Suspensión de cotización');revision=2;await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(before+2);}finally{vi.unstubAllGlobals();}
});

it('metadata-only revisions do not ring again and a content correction replaces its card',async()=>{
 vi.stubGlobal('document',{querySelector:()=>null,dispatchEvent:()=>true});const before=mocks.sound.mock.calls.length;
 let revision=1,summary='Original report';
 mocks.invoke.mockImplementation(async(cmd)=>cmd==='record_news_alerts'?undefined:({fetchedMs:Date.now(),truncated:false,items:[{id:'repeated-metadata',eventId:'event-'+revision,revision,ticker:'QQQ',tickerSymbols:['QQQ'],title:'Titular único corregible',originalTitle:'Same headline',originalSummary:summary,summary,source:'Benzinga',publishedAt:new Date().toISOString(),sentiment:'unknown',importance:'high',horizon:'intraday',translated:true}]}));
 try{await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(before+1);for(revision=2;revision<12;revision++)await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(before+1);summary='Corrected report';await pollNewsAlerts();expect(mocks.sound).toHaveBeenCalledTimes(before+2);expect(globalNewsPopup().match(/Titular único corregible/g)).toHaveLength(1);}finally{vi.unstubAllGlobals();}
});

