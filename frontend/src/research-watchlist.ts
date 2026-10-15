import {invoke,isTauri} from './snapshot-observation';
import {watchedNewsSymbols} from './news-watchlist';

/** Serializes selection changes; a failed save is retried without needing a visible page. */
export class ResearchWatchlistSync {
 private desired:string[]=[];private committed='';private running:Promise<void>|undefined;
 constructor(private send:(symbols:string[])=>Promise<unknown>){}
 request(symbols:string[]):Promise<void>{
  this.desired=[...symbols];
  if(!this.running)this.running=this.flush().finally(()=>{this.running=undefined;});
  return this.running;
 }
 private async flush(){
  while(JSON.stringify(this.desired)!==this.committed){
   const selected=[...this.desired],signature=JSON.stringify(selected);
   await this.send(selected);this.committed=signature;
  }
 }
}
const sync=new ResearchWatchlistSync(symbols=>invoke('set_research_watchlist',{symbols}));
let started=false;
export function startResearchWatchlistSync(){
 if(started||!isTauri())return;started=true;
 const update=()=>{void sync.request(watchedNewsSymbols()).catch(error=>console.error('No se pudo sincronizar la vigilancia de investigación; se reintentará.',error));};
 document.addEventListener('snipers:news-watchlist',update);
 setInterval(update,10_000);update();
}
