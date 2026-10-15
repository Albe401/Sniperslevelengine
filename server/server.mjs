import {createServer} from 'node:http';
import {readFile,stat} from 'node:fs/promises';
import {resolve,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash,timingSafeEqual,randomBytes} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {neon} from '@neondatabase/serverless';
export const symbols=['AAPL','MSFT','AMZN','GOOGL','META','NVDA','TSLA','QQQ','SPY','IWM'];
const quickFields=new Set('symbol publishedMs startedMs stocks options ticks bars minutes chartHistory history metrics optionSnapshotStatus selection flow flowExpiry cvd offExchange dailyAtr dailyRange positionMaps wallPull wallFlow adjustedReadings adjustedAlignment levelDeltas magnetConfirmations optionBaselineRecords gapReport priceAlerts qqq_report treasury auctionMap structuralPaths directionOi rawOptions levelDynamics intradayAtr priceNow snapshotUnavailable contractChain'.split(' '));
const root=resolve(fileURLToPath(new URL('..',import.meta.url)));
const sessions=new Map();let cache,cacheUntil=0;
const hash=value=>createHash('sha256').update(value).digest('hex');
export function decodeFrame(value){if(!value?.compressedBase64)return value;const raw=gunzipSync(Buffer.from(value.compressedBase64,'base64'));if(hash(raw)!==value.sha256)throw Error('Frame integrity');return JSON.parse(raw);}
export function validBundle(value,now=Date.now()){
 return !!value?.available&&Number.isFinite(value.capturedMs)&&value.capturedMs<=now&&symbols.every(s=>value.frames?.[s]?.symbol===s&&(!(typeof value.frames[s].publishedMs==='number')||value.frames[s].publishedMs<=value.capturedMs));
}
async function latest(){
 if(cacheUntil>Date.now())return cache;
 let value;
 if(process.env.DATABASE_URL){
  const sql=neon(process.env.DATABASE_URL);
  const rows=await sql.query(`SELECT b.id::text,b.captured_ms,b.coverage,b.extras,jsonb_object_agg(a.symbol,CASE WHEN a.payload_compressed IS NULL THEN a.payload ELSE jsonb_build_object('compressedBase64',encode(a.payload_compressed,'base64'),'sha256',a.payload_sha256) END) AS frames,jsonb_object_agg(a.symbol,a.sections) AS sections FROM snipers_snapshots.batches b JOIN snipers_snapshots.assets a ON a.batch_id=b.id WHERE b.id=(SELECT id FROM snipers_snapshots.batches WHERE captured_ms<=$1 AND asset_count=10 ORDER BY captured_ms DESC LIMIT 1) GROUP BY b.id HAVING count(*)=10`,[Date.now()]);
  const row=rows[0];value=row?{available:true,id:row.id,capturedMs:Number(row.captured_ms),coverage:row.coverage,extras:row.extras,frames:Object.fromEntries(Object.entries(row.frames).map(([s,f])=>[s,decodeFrame(f)])),sections:row.sections}:{available:false};
 }else value={available:false};
 if(value.available&&!validBundle(value))throw Error('Invalid capture');
 cache=value;cacheUntil=Date.now()+30000;return value;
}
const equal=(a,b)=>{const x=Buffer.from(a),y=Buffer.from(b);return x.length===y.length&&timingSafeEqual(x,y)};
export function makeServer({provider=latest,password=process.env.SNIPERS_WEB_PASSWORD??'',requireAuth=false}={}){
 if(requireAuth&&!password)throw Error('SNIPERS_WEB_PASSWORD is required for external access');
 return createServer(async(req,res)=>{
  res.setHeader('X-Content-Type-Options','nosniff');res.setHeader('Referrer-Policy','same-origin');res.setHeader('X-Frame-Options','DENY');
  res.setHeader('Content-Security-Policy',"default-src 'self'; connect-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'");
  const json=(status,body)=>{res.writeHead(status,{'Content-Type':'application/json','Cache-Control':'no-store'});res.end(JSON.stringify(body))};
  try{
   const url=new URL(req.url,'http://localhost');
   if(url.pathname==='/api/login'&&req.method==='POST'){
    if(req.headers.origin&&new URL(req.headers.origin).host!==req.headers.host)return json(403,{error:'origin'});
    let text='';for await(const chunk of req){text+=chunk;if(text.length>4096)return json(413,{error:'size'})}
    const value=JSON.parse(text);if(!password||!equal(String(value.password??''),password))return json(401,{error:'unauthorized'});
    const id=randomBytes(32).toString('hex');sessions.set(id,Date.now()+8*3600000);if(sessions.size>1000)sessions.delete(sessions.keys().next().value);
    res.setHeader('Set-Cookie',`snipers_session=${id}; HttpOnly; SameSite=Strict; Path=/${process.env.SNIPERS_SECURE_COOKIE==='1'?'; Secure':''}`);return json(200,{ok:true});
   }
   if(url.pathname.startsWith('/api/')){
    if(req.method!=='GET')return json(405,{error:'read_only'});
    const id=/(?:^|;\s*)snipers_session=([a-f0-9]+)/.exec(req.headers.cookie??'')?.[1];
    if(password&&(!id||(sessions.get(id)??0)<Date.now()))return json(401,{error:'unauthorized'});
    if(url.pathname!=='/api/frame'&&url.pathname!=='/api/health')return json(404,{error:'not_found'});
    if(url.pathname==='/api/health')return json(200,{service:'snipers-web',mode:'saved-data',refreshMs:300000,ordersEnabled:false});
    const symbol=url.searchParams.get('symbol')??'QQQ';if(!symbols.includes(symbol))return json(400,{error:'symbol'});
    const bundle=await provider();if(!bundle.available)return json(200,{available:false});
    if(!validBundle(bundle))throw Error('Invalid capture');
    const extras=Object.fromEntries(Object.entries(bundle.extras??{}).filter(([,r])=>['get_contract_inventory','get_fmp_targets','preview_route_4h'].includes(r.command)&&(r.command!=='get_contract_inventory'||r.args?.symbol===symbol)));
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
  }catch{json(503,{error:'data_unavailable',detail:'La captura o conexiÃ³n estÃ¡ pendiente. No se sustituyen los datos.'})}
 });
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const host=process.env.HOST??'127.0.0.1';makeServer({requireAuth:!['127.0.0.1','localhost','::1'].includes(host)}).listen(Number(process.env.PORT??8080),host,()=>console.log('Snipers web started; credentials remain server-side'));
}
