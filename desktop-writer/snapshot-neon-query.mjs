// Official Neon HTTP driver. Credentials arrive over stdin, never command arguments.
import {neon} from '../data/snapshot-node/package/index.mjs';
let input='';for await(const chunk of process.stdin)input+=chunk;
let credential='';
try {
 const {connection,queries}=JSON.parse(input);
 credential=connection;
 const sql=neon(connection,{arrayMode:true,fullResults:true});
 const result=await sql.transaction(queries.map(({query,params})=>sql.query(query,params)),{isolationLevel:'ReadCommitted'});
 process.stdout.write(JSON.stringify(result.map(({rows,rowCount,command})=>({rows,rowCount,command}))));
} catch(error) {
 let detail=String(error?.message??'').replaceAll(credential,'[REDACTED]');
 try{const password=decodeURIComponent(new URL(credential).password);if(password)detail=detail.replaceAll(password,'[REDACTED]');}catch{}
 detail=detail.replace(/postgres(?:ql)?:\/\/\S+/g,'[REDACTED]');
 process.stdout.write(JSON.stringify({error:'neon_http_unavailable',type:error?.name,code:error?.code??error?.cause?.code??null,detail:detail.slice(0,300)}));process.exitCode=1;
}
