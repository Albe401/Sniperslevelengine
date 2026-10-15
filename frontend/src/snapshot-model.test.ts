import {describe,it,expect} from 'vitest';
import {validSavedBundle,savedAsset,snapshotSymbols,snapshotCommandAllowed,type SavedBundle} from './snapshot-model';
const now=1_000_000;
function bundle():SavedBundle{return {available:true,id:'batch',capturedMs:now-300000,frames:Object.fromEntries(snapshotSymbols.map(symbol=>[symbol,{symbol,publishedMs:now-300000}]))};}
describe('database snapshot isolation',()=>{
 it('does not duplicate the high-frequency live stream through UI observations',()=>{
  for(const c of ['get_live_state','get_bot_live_state','get_option_snapshot','get_cvd','get_ticks'])expect(snapshotCommandAllowed(c)).toBe(false);
  expect(snapshotCommandAllowed('get_company_research_detail')).toBe(false);
  expect(snapshotCommandAllowed('get_contract_inventory')).toBe(true);
 });
 it('requires ten correctly identified assets and at least five minutes',()=>{const b=bundle();expect(validSavedBundle(b,now)).toBe(true);b.capturedMs!++;expect(validSavedBundle(b,now)).toBe(false);});
 it('rejects future frame data and cross-asset substitution',()=>{const b=bundle();b.frames!.AAPL.publishedMs=now;expect(validSavedBundle(b,now)).toBe(false);b.frames!.AAPL={symbol:'QQQ'};expect(savedAsset(b,'AAPL',now)).toBeUndefined();});
 it('never supplies a realtime fallback or unsupported symbol',()=>{expect(savedAsset(undefined,'QQQ',now)).toBeUndefined();expect(savedAsset(bundle(),'AVGO',now)).toBeUndefined();});
 it('excludes credentials, unrelated sections and broker controls',()=>{for(const c of ['get_alpaca_paper_snapshot','get_paper_bot_state','get_api_credentials','send_order','set_paper_bot_asset_mode','get_benzinga_news'])expect(snapshotCommandAllowed(c)).toBe(false);expect(snapshotCommandAllowed('get_contract_inventory')).toBe(true);});
});
