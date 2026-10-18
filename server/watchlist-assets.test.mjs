import {test} from 'node:test';import assert from 'node:assert/strict';
import {loadWatchlistAsset} from './watchlist-assets.mjs';
const row=()=>({symbol:'BAC',captured_ms:1000,source:'snipers-production-rest-workers-v1',payload_sha256:'abc',coverage:{ordersEnabled:false},frame:{symbol:'BAC',publishedMs:999}});
test('BAC and dotted tickers use parameterized per-asset reads without requiring ten assets',async()=>{
 const calls=[];const sql={query:async(q,p)=>{calls.push({q,p});return [row()]}};
 const b=await loadWatchlistAsset(sql,'BAC',v=>v,1001);assert.equal(b.frames.BAC.symbol,'BAC');assert.equal(b.completeBatchVerified,true);assert.deepEqual(calls[0].p,['BAC',1001]);assert(!calls[0].q.includes('asset_count=10'));
});
test('missing new table remains compatible with older deployments; other database errors propagate',async()=>{
 assert.deepEqual(await loadWatchlistAsset({query:async()=>{throw {code:'42P01'}}},'BAC',v=>v),{available:false});
 await assert.rejects(loadWatchlistAsset({query:async()=>{throw Error('quota')}},'BAC',v=>v));
});
test('rejects swapped identity, future observations, unknown source and corrupt archive',async()=>{
 for(const patch of [{symbol:'QQQ'},{captured_ms:2000},{source:'unverified'},{frame:{symbol:'QQQ',publishedMs:999}},{frame:{symbol:'BAC',publishedMs:1001}}])await assert.rejects(loadWatchlistAsset({query:async()=>[{...row(),...patch}]},'BAC',v=>v,1001));
 await assert.rejects(loadWatchlistAsset({query:async()=>[row()]},'BAC',()=>{throw Error('hash')},1001));
});
