import {test} from 'node:test';import assert from 'node:assert/strict';import {gzipSync} from 'node:zlib';import {createHash} from 'node:crypto';
import {makeServer,decodeFrame,validBundle,symbols} from './server.mjs';
import {loadSavedAsset} from './server.mjs';
test('unchanged capture checks metadata without downloading the archive again',async()=>{
 const b=bundle();const calls=[];const sql={query:async(q,p)=>{calls.push({q,p});return [{id:b.id}]}};
 assert.equal(await loadSavedAsset(sql,'QQQ',b),b);assert.equal(calls.length,1);assert(!calls[0].q.includes('payload_compressed'));
});
test('changed capture downloads the requested asset from the same complete batch',async()=>{
 const now=Date.now();const calls=[];const sql={query:async(q,p)=>{calls.push({q,p});return calls.length===1?[{id:'new'}]:[{id:'new',captured_ms:now-1000,coverage:{},extras:{},symbol:'QQQ',frame:{symbol:'QQQ',publishedMs:now-1001}}]}};
 const r=await loadSavedAsset(sql,'QQQ',bundle(),now);assert.equal(r.id,'new');assert.deepEqual(calls[1].p,['new','QQQ']);assert.equal(r.frames.QQQ.symbol,'QQQ');assert(calls[1].q.includes('count(*)'));
});
test('quota failure gives safe actionable message and five-minute retry',async()=>{
 const server=makeServer({provider:async()=>{throw Error('HTTP 402: account exceeded quota; private diagnostic')}});await new Promise(r=>server.listen(0,'127.0.0.1',r));
 try{const r=await fetch(`http://127.0.0.1:${server.address().port}/api/frame?symbol=QQQ`);const body=await r.json();assert.equal(r.status,503);assert.equal(r.headers.get('retry-after'),'300');assert.equal(body.error,'database_quota_exceeded');assert.equal(body.retryAfterMs,300000);assert(!JSON.stringify(body).includes('private diagnostic'));}finally{await new Promise(r=>server.close(r))}
});
const bundle=()=>({available:true,id:'test',capturedMs:Date.now()-300001,frames:Object.fromEntries(symbols.map(symbol=>[symbol,{symbol,publishedMs:Date.now()-300002}]))});
test('conditional frame reads return no body for exact unchanged data and preserve every strike on revisions',async()=>{
 const b=bundle();b.frames.QQQ.rawOptions={strikes:[450,500,695,700,900]};const server=makeServer({provider:async()=>b});await new Promise(r=>server.listen(0,'127.0.0.1',r));
 try{
  const url=`http://127.0.0.1:${server.address().port}/api/frame?symbol=QQQ`,first=await fetch(url),tag=first.headers.get('etag'),data=await first.json();
  assert(tag);assert.deepEqual(data.frame.rawOptions.strikes,[450,500,695,700,900]);
  const same=await fetch(url,{headers:{'If-None-Match':tag}});assert.equal(same.status,304);assert.equal(await same.text(),'');assert.equal(same.headers.get('cache-control'),'no-store');
  b.frames.QQQ.rawOptions.strikes.push(1000);const revised=await fetch(url,{headers:{'If-None-Match':tag}});assert.equal(revised.status,200);assert.notEqual(revised.headers.get('etag'),tag);assert.deepEqual((await revised.json()).frame.rawOptions.strikes,[450,500,695,700,900,1000]);
  const foreign=await fetch(url.replace('QQQ','SPY'),{headers:{'If-None-Match':tag}});assert.equal(foreign.status,200);assert.equal((await foreign.json()).frame.symbol,'SPY');
 }finally{await new Promise(r=>server.close(r));}
});
test('rejects future data and requires correct identity for all ten',()=>{const b=bundle();assert(validBundle(b));b.frames.AAPL.symbol='QQQ';assert(!validBundle(b));b.frames.AAPL.symbol='AAPL';b.capturedMs=Date.now()+1000;assert(!validBundle(b))});
test('archives are exact and corruption fails',()=>{const raw=Buffer.from('{"symbol":"AAPL","price":123.45}');const f={compressedBase64:gzipSync(raw).toString('base64'),sha256:createHash('sha256').update(raw).digest('hex')};assert.equal(decodeFrame(f).price,123.45);assert.throws(()=>decodeFrame({...f,sha256:'bad'}))});
test('public service starts without password',()=>{makeServer().close()});
test('public API preserves frozen price structure without adding market credentials',async()=>{
 const b=bundle();b.frames.QQQ.priceStructureReading={version:'price-structure-five-wilder14-pivots2-v1',symbol:'QQQ',capturedMs:b.capturedMs,cards:[]};b.frames.QQQ.structuralActivity={version:'structural-activity-v1',symbol:'QQQ',windows:[]};b.frames.QQQ.apiKey='must-not-ship';
 const server=makeServer({provider:async()=>b});await new Promise(r=>server.listen(0,'127.0.0.1',r));
 try{const v=await(await fetch(`http://127.0.0.1:${server.address().port}/api/frame?symbol=QQQ`)).json();assert.deepEqual(v.frame.priceStructureReading,b.frames.QQQ.priceStructureReading);assert.deepEqual(v.frame.structuralActivity,b.frames.QQQ.structuralActivity);assert(!('apiKey' in v.frame));}finally{await new Promise(r=>server.close(r));}
});
test('web response excludes photos and unrelated sections',async()=>{
 const b=bundle();b.frames.QQQ.learningReport={private:true};b.images={QQQ:{base64:'should_not_ship'}};b.extras={news:{command:'get_benzinga_news',value:{}},other:{command:'get_contract_inventory',args:{symbol:'AAPL'},value:{}}};
 const server=makeServer({provider:async()=>b});await new Promise(r=>server.listen(0,'127.0.0.1',r));
 try{const r=await fetch(`http://127.0.0.1:${server.address().port}/api/frame?symbol=QQQ`);const v=await r.json();assert(!('photograph' in v));assert(!('learningReport' in v.frame));assert.deepEqual(v.extras,{});assert.equal(r.headers.get('content-security-policy').includes("connect-src 'self'"),true);assert.equal(r.headers.get('x-frame-options'),null);assert.match(r.headers.get('content-security-policy'),/frame-ancestors 'self' https:\/\/sniperstradingacademy\.com https:\/\/www\.sniperstradingacademy\.com http:\/\/localhost:3000/);assert.equal(r.headers.get('set-cookie'),null)}finally{await new Promise(r=>server.close(r))}
});
test('public API returns own asset, and blocks execution',async()=>{const server=makeServer({password:'test-password',provider:async()=>bundle()});await new Promise(r=>server.listen(0,'127.0.0.1',r));const base=`http://127.0.0.1:${server.address().port}`;try{assert.equal((await fetch(base+'/api/frame')).status,200);const cookie='';const r=await fetch(base+'/api/frame?symbol=NVDA',{headers:{cookie}});assert.equal((await r.json()).frame.symbol,'NVDA');assert.equal((await fetch(base+'/api/frame?symbol=INVALID',{headers:{cookie}})).status,400);assert.equal((await fetch(base+'/api/orders',{method:'POST',headers:{cookie}})).status,405)}finally{await new Promise(r=>server.close(r))}});
test('saved block scanner status is delivered through the read-only API',async()=>{
 const b=bundle();b.extras={blocks:{command:'get_block_alerts',observedMs:b.capturedMs-1,value:{enabled:true,total:186,history:[]}},secret:{command:'get_api_credentials',value:'must-not-leak'}};
 const server=makeServer({provider:async()=>b});await new Promise(r=>server.listen(0,'127.0.0.1',r));
 try{const response=await fetch(`http://127.0.0.1:${server.address().port}/api/frame?symbol=QQQ`);const data=await response.json();assert.equal(data.extras.blocks.value.total,186);assert(!data.extras.secret);}finally{await new Promise(r=>server.close(r))}
});
