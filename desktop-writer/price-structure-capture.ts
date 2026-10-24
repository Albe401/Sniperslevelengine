import {readFileSync,statSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {capturePriceStructure} from '../frontend/src/saved-price-structure';
const input=JSON.parse(readFileSync(0,'utf8'));
const result:Record<string,unknown>={};
for(const [symbol,frame] of Object.entries(input.frames)){
 let history,sha:string|null=null;
 try{
  const file=input.histories[symbol],before=statSync(file,{bigint:true});
  // asOfMs is the REST query's start. The file timestamp also proves download
  // completion before capture, including a cache replacement racing the writer.
  if(before.mtimeNs>BigInt(input.capturedMs)*1000000n)throw Error('Later acquisition');
  const bytes=readFileSync(file),after=statSync(file,{bigint:true});
  if(before.mtimeNs!==after.mtimeNs||before.ino!==after.ino||before.size!==after.size)throw Error('Changed cache');
  history=JSON.parse(bytes.toString('utf8'));sha=createHash('sha256').update(bytes).digest('hex');
 }catch{/* Truthful pending coverage when history is unavailable or acquired later. */}
 result[symbol]=capturePriceStructure(frame as any,symbol,input.capturedMs,history,sha);
}
process.stdout.write(JSON.stringify(result));
