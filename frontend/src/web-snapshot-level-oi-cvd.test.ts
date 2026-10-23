import {expect,it} from 'vitest';
import {chartLevelOiReport} from './contract-profile';
import {levelOpenInterest} from './level-open-interest';
import {dayMap} from './day-map';
import {blockPressurePanel} from './block-pressure';
import {structuralEnrichmentPanel} from './structural-enrichment';
import {etfDealersPage} from './etf-dealers';
import type {EtfReport} from './etf-dealers';
import type {Horizon,LiveState} from './live';

const now=Date.parse('2026-10-22T15:00:00Z'),expiry='2026-10-23',strike=279;
const ticker=(side:'call'|'put')=>`O:QQQ261023${side==='call'?'C':'P'}00279000`;
const detail=(side:'call'|'put',oi:number)=>({ticker:ticker(side),expiration:expiry,strike,kind:side,oi,volume:0,dex:null,gex:null,delta:null,gamma:null});
const full=(at:number,oiCall:number,oiPut:number)=>({symbol:'QQQ',at,days:183,spot:279,priceAt:at,expirations:[expiry],contracts:2,rows:[],gammaComplete:false,deltaComplete:false,
 enrichment:{oiReferenceDate:'2026-10-21',details:[detail('call',oiCall),detail('put',oiPut)]}} as unknown as EtfReport);
const reference=(side:'call'|'put')=>({ticker:ticker(side),underlying_ticker:'QQQ',expiration_date:expiry,strike_price:strike,contract_type:side,shares_per_contract:100});
const observation=(side:'call'|'put',oi:number)=>({details:reference(side),underlying_asset:{ticker:'QQQ'},open_interest:oi});
const horizon={mode:'DAY',observedMs:now,expirations:[expiry],structure:{complete:true,selected:{M1:strike},magnet:strike,mainMagnetScore:61,liquidityMagnetScore:62,routes:[]}} as unknown as Horizon;
function captured():LiveState {
 const local=full(now,100,300),chain=full(now-60_000,150,250);
 chain.heatmap={reference:[reference('call'),reference('put')],snapshots:[observation('call',150),observation('put',250)],referenceComplete:true,scopeStart:'2026-10-22',scopeEnd:expiry};
 return {symbol:'QQQ',publishedMs:now,optionSnapshotStatus:{phase:'ready',observedMs:now,structuralEnrichment:local,horizons:[horizon]},
  contractChain:{observation:{command:'get_contract_inventory',args:{symbol:'QQQ'},observedMs:now-30_000,value:chain}}} as unknown as LiveState;
}

it('uses the archived independent OI reference for the same symbol, date and expiration',()=>{
 const frame=captured(),r=chartLevelOiReport(frame,'QQQ',true,horizon,now)!;
 expect(levelOpenInterest(r,strike,[expiry])).toMatchObject({call:150,put:250,total:400,verification:'reference',scopeComplete:true,oiDate:'2026-10-21'});
 const html=dayMap(horizon,true,[],false,null,'',undefined,true,'',undefined,undefined,'',{oiReport:r});
 expect(html).toContain('CALL 150 · PUT 250');
 expect(html).toContain('cotejado con el mapa de OI');
 expect(html).toContain('Fecha efectiva OI 2026-10-21');
});

it('labels a local-only snapshot and never invents OI when the independent cell is incomplete',()=>{
 const frame=captured(),record=(frame as LiveState&{contractChain:{observation:{args:{symbol:string};observedMs:number;value:EtfReport}}}).contractChain.observation;
 record.args.symbol='SPY';
 expect(levelOpenInterest(chartLevelOiReport(frame,'QQQ',true,horizon,now),strike,[expiry])).toMatchObject({call:100,put:300,verification:'snapshot',scopeComplete:false});
 record.args.symbol='QQQ';record.value.heatmap!.snapshots=[observation('call',150)];
 expect(levelOpenInterest(chartLevelOiReport(frame,'QQQ',true,horizon,now),strike,[expiry])).toBeNull();
 record.observedMs=now+1;
 expect(levelOpenInterest(chartLevelOiReport(frame,'QQQ',true,horizon,now),strike,[expiry])?.verification).toBe('snapshot');
});

it('excludes an adjusted deliverable in the independently verified OI without subtracting its volume',()=>{
 const frame=captured(),record=(frame as LiveState&{contractChain:{observation:{value:EtfReport}}}).contractChain.observation;
 const adjusted={...reference('call'),shares_per_contract:50};
 record.value.heatmap!.reference=[adjusted,reference('put')];
 record.value.heatmap!.snapshots=[{details:adjusted,underlying_asset:{ticker:'QQQ'},open_interest:150},observation('put',250)];
 expect(levelOpenInterest(chartLevelOiReport(frame,'QQQ',true,horizon,now),strike,[expiry])).toMatchObject({call:0,put:250,verification:'reference'});
});

it('omits the visual CVD card from the web panels while preserving the other sections',()=>{
 const blocks=blockPressurePanel(undefined,'QQQ',false,now),structure=structuralEnrichmentPanel('QQQ',undefined,false,now,[]),scanner=etfDealersPage();
 for(const html of [blocks,structure,scanner])expect(html).not.toContain('MOTOR CVD · ACCIÓN');
 expect(blocks).toContain('Bloques');
 expect(structure).toContain('Flujo de opciones y bloques');
 expect(scanner).toContain('CADENA DE OPCIONES');
});
