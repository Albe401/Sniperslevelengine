import {makeHandler} from '../server/server.mjs';
export function adaptVercelHandler(handler){
 return async(req,res)=>{
  const url=new URL(req.url,'http://localhost');
  const route=url.searchParams.get('__snipersRoute')??req.query?.__snipersRoute;
  if(route!==undefined){
   if(typeof route!=='string'||!['frame','login','health'].includes(route)){res.writeHead(404,{'Content-Type':'application/json'});res.end(JSON.stringify({error:'not_found'}));return;}
   url.searchParams.delete('__snipersRoute');req.url=`/api/${route}${url.search}`;
  }
  return handler(req,res);
 };
}
export default async function handler(req,res){
 if(!process.env.DATABASE_URL||!process.env.SNIPERS_WEB_PASSWORD){
  res.writeHead(503,{'Content-Type':'application/json','Cache-Control':'no-store'});
  res.end(JSON.stringify({error:'configuration_pending',detail:'Configura DATABASE_URL y SNIPERS_WEB_PASSWORD en Vercel.'}));return;
 }
 return adaptVercelHandler(makeHandler({requireAuth:true}))(req,res);
}
