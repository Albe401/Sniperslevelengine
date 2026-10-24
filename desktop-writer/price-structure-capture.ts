import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {capturePriceStructure} from '../frontend/src/saved-price-structure';
const input=JSON.parse(readFileSync(0,'utf8'));
const result:Record<string,unknown>={};
for(const [symbol,frame] of Object.entries(input.frames)){
 let history,sha:string|null=null;
 try{const bytes=readFileSync(input.histories[symbol]);history=JSON.parse(bytes.toString('utf8'));sha=createHash('sha256').update(bytes).digest('hex');}catch{/* Truthful pending coverage when history is unavailable. */}
 result[symbol]=capturePriceStructure(frame as any,symbol,input.capturedMs,history,sha);
}
process.stdout.write(JSON.stringify(result));
