import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc,price} from './model';
export interface LegacyPage {source:string;total:number;openTrades:number;closedTrades:number;nextBeforeId:number|null;entries:{id:number;session:string;timestampMs:number;status:string;record:Record<string,unknown>}[]}
interface Preview {references?:{file:string;bytes:number;records:number}[];path:string;counts:{decisions:number;trades:number;openTrades:number;closedTrades:number}}
let page:LegacyPage|undefined,preview:Preview|undefined,path='',busy=false,message='';
const text=(v:unknown)=>v==null?'—':String(v);
const number=(v:unknown)=>typeof v==='number'&&Number.isFinite(v)?price(v):'—';
export function legacyContent(data:LegacyPage|undefined,loading:boolean,error:string,sourcePath:string,review:Preview|undefined,available:boolean):string {
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">ARCHIVO DEL PROYECTO ANTERIOR</span><h2>Diario Python</h2></div><button class="replay-button" data-legacy="refresh" ${loading||!available?'disabled':''}>Actualizar archivo</button></div>
  <p class="panel-caption">Se conservan los identificadores, campos y estados originales. Las posiciones abiertas bloquean nuevas admisiones hasta su cierre en el seguimiento Rust. Para reanudarlas, detén el programa anterior y activa el seguimiento de importadas al conectar el monitor. Estas operaciones no se mezclan con las estadísticas Rust.</p>
  <div class="history-controls"><label>Archivo del diario<input data-legacy-path type="text" placeholder="Ruta a snipers_paper_trades.db (opcional)" value="${esc(sourcePath)}" ${loading?'disabled':''}></label><button class="replay-button" data-legacy="preview" ${loading||!available?'disabled':''}>Revisar origen</button><button class="replay-button" data-legacy="import" ${loading||!available||!review?'disabled':''}>Importar copia al diario</button></div>
  ${review?`<p class="panel-caption">Origen revisado: ${review.counts.decisions} decisiones · ${review.counts.trades} operaciones · ${review.counts.openTrades} abiertas. Se crearán copias nuevas del origen y del diario Rust antes de importar. ${review.references?.length?`Referencias OI/DEX para conservar: ${review.references.map(r=>`${esc(r.file)} (${r.records} registros)`).join(', ')}. Se archivan sin convertirlas en bases de cálculo en vivo. `:''}Esta copia refleja ese momento; detén el programa anterior antes del traspaso definitivo.</p>`:''}
  ${error?`<p class="panel-caption" role="status">${esc(error)}</p>`:''}
  ${data?`<p class="panel-caption">Archivo importado: ${data.total} operaciones · ${data.openTrades} abiertas en el origen · ${data.closedTrades} cerradas.</p>`:''}
  <div class="table-wrap"><table><thead><tr><th>ID / SESIÓN</th><th>ACTIVO / LADO</th><th>ESTADO ORIGINAL</th><th>ENTRADA / ÚLTIMO</th><th>STOP / OBJETIVOS</th><th>PnL · PUNTOS</th></tr></thead><tbody>${data?.entries.map(e=>`<tr><td>#${e.id}<small class="exposure-dates">${esc(e.session)}</small></td><td>${esc(text(e.record.symbol))} · ${esc(text(e.record.side))}</td><td>${e.status==='OPEN'?'Abierta en el origen':'Cerrada'}<small class="exposure-dates">${esc(text(e.record.exit_reason))}</small></td><td>${number(e.record.entry_price)} / ${number(e.record.current_price)}</td><td>${number(e.record.stop_price)}<small class="exposure-dates">${number(e.record.target_1)} / ${number(e.record.target_2)}</small></td><td>${number(e.record.pnl_points)}</td></tr>`).join('')||`<tr><td colspan="6">${loading?'Consultando archivo…':data?'No hay operaciones del legado importadas.':'Consulta el archivo local o revisa el diario del programa anterior.'}</td></tr>`}</tbody></table></div>
  <div class="panel-head"><span class="muted">25 registros por página · orden original de IDs</span><button class="replay-button" data-legacy="next" ${loading||data?.nextBeforeId==null?'disabled':''}>Ver anteriores</button></div></section>`;
}
export const legacyJournal=()=>`<div id="legacy-journal">${legacyContent(page,busy,message,path,preview,isTauri())}</div>`;
function paint(){const root=document.querySelector('#legacy-journal');if(root){root.innerHTML=legacyContent(page,busy,message,path,preview,isTauri());bindLegacy();}}
export function bindLegacy(){
  const input=document.querySelector<HTMLInputElement>('[data-legacy-path]');if(input)input.onchange=()=>{path=input.value;preview=undefined;paint();};
  document.querySelectorAll<HTMLButtonElement>('[data-legacy]').forEach(button=>button.onclick=async()=>{
    const action=button.dataset.legacy;busy=true;message='';paint();
    try {
      if(action==='preview'){preview=await invoke<Preview>('preview_legacy_database',{path:path||null});path=preview.path;}
      else if(action==='import'&&preview){const result=await invoke<{imported:{decisions:number;paper_trades:number};legacyBackup:string;rustBackup:string;referenceBackup:string|null}>('import_legacy_database',{path:preview.path});message=`Copia importada: ${result.imported.decisions} decisiones y ${result.imported.paper_trades} operaciones. Respaldo: ${result.legacyBackup}${result.referenceBackup?` · Archivo OI/DEX: ${result.referenceBackup}`:''}`;page=await invoke<LegacyPage>('get_legacy_journal',{beforeId:null});}
      else {page=await invoke<LegacyPage>('get_legacy_journal',{beforeId:action==='next'?page?.nextBeforeId:null});}
    }catch(e){message=String(e);}finally{busy=false;paint();}
  });
}
