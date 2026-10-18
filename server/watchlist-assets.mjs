/** Supplemental immutable engine captures, independent of the legacy ten-asset batch. */
export async function loadWatchlistAsset(sql,symbol,decode,now=Date.now()){
 let rows;
 try{rows=await sql.query(`SELECT symbol,captured_ms,source,payload_sha256,coverage,jsonb_build_object('compressedBase64',encode(payload_compressed,'base64'),'sha256',payload_sha256) AS frame FROM snipers_snapshots.watchlist_assets WHERE symbol=$1 AND captured_ms<=$2`,[symbol,now]);}
 catch(error){if(error?.code==='42P01')return {available:false};throw error;}
 const row=rows[0];if(!row)return {available:false};
 const capturedMs=Number(row.captured_ms),frame=decode(row.frame);
 if(row.symbol!==symbol||row.source!=='snipers-production-rest-workers-v1'||!Number.isFinite(capturedMs)||capturedMs<=0||capturedMs>now||frame?.symbol!==symbol||!Number.isFinite(frame.publishedMs)||frame.publishedMs>capturedMs)throw Error('Invalid watchlist capture');
 return {available:true,id:`watchlist:${symbol}:${capturedMs}:${row.payload_sha256}`,capturedMs,frames:{[symbol]:frame},coverage:{...row.coverage,source:row.source},extras:{},completeBatchVerified:true};
}
