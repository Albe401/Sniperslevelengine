import {createServer} from 'node:http';
import {readFile,stat} from 'node:fs/promises';
import {resolve,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {gunzipSync,gzipSync} from 'node:zlib';
import {neon} from '@neondatabase/serverless';
import {loadWeeklyArchive} from './weekly-archive.mjs';
export const symbols=['AAPL','MSFT','AMZN','GOOGL','META','NVDA','TSLA','QQQ','SPY','IWM'];
const watchlist=JSON.parse(await readFile(new URL('../frontend/src/web-watchlist.json',import.meta.url),'utf8'));
const allowedSymbols=new Set(watchlist.map(a=>a.symbol));
const quickFields=new Set('symbol publishedMs startedMs stocks options ticks bars minutes chartHistory history metrics optionSnapshotStatus selection flow flowExpiry cvd offExchange dailyAtr dailyRange positionMaps wallPull wallFlow adjustedReadings adjustedAlignment levelDeltas magnetConfirmations optionBaselineRecords gapReport priceAlerts qqq_report treasury auctionMap structuralPaths directionOi rawOptions levelDynamics intradayAtr priceNow snapshotUnavailable contractChain'.split(' '));
for(const field of ['rangeContext','localVolume','greekSupport','thetaVega','optionLiquidity','swingReport','neuroContext','qqqReport','optionTiming','etfBlocks','etfCvd','etfFlow','dataService','ivHistory'])quickFields.add(field);
quickFields.add('weeklyRangeSnapshot');
const root=resolve(fileURLToPath(new URL('..',import.meta.url)));
const caches=new Map();
let quotaRetryAt=0;
export function isQuotaError(error){const message=String(error?.message??'');return /quota/i.test(message)&&/402|exceed/i.test(message);}
function quotaError(){return Object.assign(new Error('Database quota exceeded'),{code:'database_quota_exceeded'});}
const hash=value=>createHash('sha256').update(value).digest('hex');
export function decodeFrame(value){if(!value?.compressedBase64)return value;const raw=gunzipSync(Buffer.from(value.compressedBase64,'base64'));if(hash(raw)!==value.sha256)throw Error('Frame integrity');return JSON.parse(raw);}
export function validBundle(value,now=Date.now()){
 return !!value?.available&&Number.isFinite(value.capturedMs)&&value.capturedMs<=now&&symbols.every(s=>value.frames?.[s]?.symbol===s&&(!(typeof value.frames[s].publishedMs==='number')||value.frames[s].publishedMs<=value.capturedMs));
}
/** Check a tiny batch identifier before downloading the same archived asset again. */
export async function loadSavedAsset(sql,symbol,cached,now=Date.now()){
 const heads=await sql.query(`SELECT b.id::text FROM snipers_snapshots.batches b WHERE b.captured_ms<=$1 AND b.asset_count=10 AND (SELECT count(*) FROM snipers_snapshots.assets complete WHERE complete.batch_id=b.id)=10 ORDER BY b.captured_ms DESC LIMIT 1`,[now]);
 if(!heads[0])return {available:false};
 if(cached?.available&&cached.id===heads[0].id&&validAssetBundle(cached,symbol,now))return cached;
 const rows=await sql.query(`SELECT b.id::text,b.captured_ms,b.coverage,b.extras,a.symbol,CASE WHEN a.payload_compressed IS NULL THEN a.payload ELSE jsonb_build_object('compressedBase64',encode(a.payload_compressed,'base64'),'sha256',a.payload_sha256) END AS frame FROM snipers_snapshots.batches b JOIN snipers_snapshots.assets a ON a.batch_id=b.id WHERE b.id=$1::uuid AND a.symbol=$2 AND b.asset_count=10 AND (SELECT count(*) FROM snipers_snapshots.assets complete WHERE complete.batch_id=b.id)=10`,[heads[0].id,symbol]);
 const row=rows[0];return row?{available:true,id:row.id,capturedMs:Number(row.captured_ms),coverage:row.coverage,extras:row.extras,frames:{[row.symbol]:decodeFrame(row.frame)},completeBatchVerified:true}:{available:false};
}
async function latest(symbol='QQQ'){
 if(Date.now()<quotaRetryAt)throw quotaError();
 const cached=caches.get(symbol);if(cached?.until>Date.now())return cached.value;
 let value;
 if(process.env.DATABASE_URL){
  const sql=neon(process.env.DATABASE_URL);
  try{value=await loadSavedAsset(sql,symbol,cached?.value);
   if(value.available){const frame=value.frames[symbol];
    if(!frame.weeklyRangeSnapshot){try{frame.weeklyRangeSnapshot=await loadWeeklyArchive(sql,frame,value.capturedMs,decodeFrame);}catch{ /* Preserve the normal frame if the optional archive is unavailable. */ }}
   }
  }
  catch(error){if(isQuotaError(error)){quotaRetryAt=Date.now()+300000;throw quotaError();}throw error;}
 }else value={available:false};
 if(value.available&&!validAssetBundle(value,symbol))throw Error('Invalid capture');
 caches.set(symbol,{value,until:Date.now()+30000});return value;
}
export function validAssetBundle(b,symbol,now=Date.now()){
 if(validBundle(b,now))return true;
 const f=b?.frames?.[symbol];return b?.completeBatchVerified===true&&Number.isFinite(b.capturedMs)&&b.capturedMs<=now&&f?.symbol===symbol&&(!(typeof f.publishedMs==='number')||f.publishedMs<=b.capturedMs);
}
export function makeHandler({provider=latest}={}){
 return async(req,res)=>{
  res.setHeader('X-Content-Type-Options','nosniff');res.setHeader('Referrer-Policy','same-origin');res.setHeader('X-Frame-Options','DENY');
  res.setHeader('Content-Security-Policy',"default-src 'self'; connect-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'");
  const json=(status,body)=>{let bytes=Buffer.from(JSON.stringify(body));const headers={'Content-Type':'application/json','Cache-Control':'no-store','Vary':'Accept-Encoding'};if(bytes.length>1024&&/\bgzip\b/.test(req.headers['accept-encoding']??'')){bytes=gzipSync(bytes);headers['Content-Encoding']='gzip';}res.writeHead(status,headers);res.end(bytes)};
  try{
   const url=new URL(req.url,'http://localhost');
   if(url.pathname.startsWith('/api/')){
    if(req.method!=='GET')return json(405,{error:'read_only'});
    if(url.pathname!=='/api/frame'&&url.pathname!=='/api/health')return json(404,{error:'not_found'});
    if(url.pathname==='/api/health')return json(200,{service:'snipers-web',mode:'saved-data',refreshMs:300000,ordersEnabled:false});
    const symbol=url.searchParams.get('symbol')??'QQQ';if(!allowedSymbols.has(symbol))return json(400,{error:'symbol'});
    const bundle=await provider(symbol);if(!bundle.available)return json(200,{available:false});
    if(!validAssetBundle(bundle,symbol))throw Error('Invalid capture');
    const extras=Object.fromEntries(Object.entries(bundle.extras??{}).filter(([,r])=>['get_contract_inventory','get_fmp_targets','preview_route_4h','get_block_alerts'].includes(r.command)&&(r.command!=='get_contract_inventory'||r.args?.symbol===symbol)));
    const frame=Object.fromEntries(Object.entries(bundle.frames[symbol]).filter(([key])=>quickFields.has(key)));
    return json(200,{available:true,id:bundle.id,capturedMs:bundle.capturedMs,coverage:bundle.coverage,frame,extras});
   }
   if(req.method!=='GET'&&req.method!=='HEAD')return json(405,{error:'read_only'});
   const base=resolve(root,'frontend/dist');const decoded=decodeURIComponent(url.pathname);
   let path=resolve(base,'.'+(decoded==='/'?'/index.html':decoded));
   if(!path.startsWith(base+sep))return json(403,{error:'path'});
   try{if(!(await stat(path)).isFile())throw Error('file')}catch{path=resolve(base,'index.html')}
   const type=path.endsWith('.html')?'text/html':path.endsWith('.js')?'text/javascript':path.endsWith('.css')?'text/css':path.endsWith('.png')?'image/png':path.endsWith('.woff2')?'font/woff2':'application/octet-stream';
   res.writeHead(200,{'Content-Type':type});res.end(await readFile(path));
  }catch(error){if(error?.code==='database_quota_exceeded'||isQuotaError(error)){res.setHeader('Retry-After','300');json(503,{error:'database_quota_exceeded',retryAfterMs:300000,detail:'Neon ha bloqueado la base de datos por cuota excedida. Revisa los límites o el plan en Neon. Se conservará la captura visible, si existe.'});}else json(503,{error:'data_unavailable',detail:'La captura o conexión está pendiente. No se sustituyen los datos.'})}
 };
}
export function makeServer(options={}){return createServer(makeHandler(options));}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const host=process.env.HOST??'127.0.0.1';makeServer().listen(Number(process.env.PORT??8080),host,()=>console.log('Snipers web started; credentials remain server-side'));
}
