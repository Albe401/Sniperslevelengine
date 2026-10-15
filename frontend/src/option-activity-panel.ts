import {escapeHtml as esc} from './model';
import {optionResearchSnapshot,pollOptionResearch,type OptionResearchStatus} from './option-research-live';
import './structural-map-preview.css';
import './option-activity-panel.css';
const dateAt=(ms:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}).format(ms);
const timeAt=(ms:number)=>new Intl.DateTimeFormat('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hour12:false}).format(ms);
export function optionActivityPanel(symbol:string,active:boolean,now=Date.now(),status:OptionResearchStatus|undefined=optionResearchSnapshot()):string {
 pollOptionResearch(now);
 const audit=status?.audit;
 const fresh=active&&symbol==='QQQ'&&status?.status==='OBSERVING'&&typeof audit?.generatedMs==='number'&&audit.generatedMs<=now&&now-audit.generatedMs<=30000;
 const rows=fresh?(audit?.reconciliation??[]).filter(r=>r.underlying===symbol&&r.date===dateAt(now)&&Number.isFinite(r.endMs)&&r.endMs<=now):[];
 const end=rows.length?Math.max(...rows.map(r=>r.endMs)):undefined;
 const current=end!=null&&now-end<=420000?rows.filter(r=>r.endMs===end):[];
 const reason=symbol!=='QQQ'?'Disponible para QQQ':!active?'Conexión pendiente':!status?'Esperando datos':!fresh?'Esperando sesión y datos vigentes':!current.length?'Esperando ventana cerrada':'Ventana '+timeAt(end!-300000)+'–'+timeAt(end!)+' ET';
 const cards=([
  ['VOLUMEN','Contratos negociados','volume'],['PRIMAS','Importe negociado','premium'],['COINCIDENCIA','Volumen + primas','joint']
 ] as const).map(([name,subtitle,kind])=>{
  const certified=current.filter(r=>kind==='joint'?r.volume&&r.premium&&r.priceContextCompatible===true:!!r[kind]);
  const comparable=certified.filter(r=>kind==='joint'?r.volume?.comparable===true&&r.premium?.comparable===true:r[kind]?.comparable===true);
  const unusual=comparable.filter(r=>kind==='joint'?r.volume?.unusual===true&&r.premium?.unusual===true:r[kind]?.unusual===true).length;
  const ready=comparable.length>0;
  return `<article class="horizon partial"><header class="card-head"><div><h3>${name}</h3><span class="subtitle">${subtitle} · 5 min</span></div></header><div class="main"><div class="ring"><div class="ring-core"><strong>${ready?unusual:'—'}</strong><span>${ready?'INUSUALES':'PENDIENTE'}</span></div></div><dl class="deltas"><div><dt>OBSERVADOS</dt><dd>${certified.length||'—'}</dd></div><div><dt>COMPARABLES</dt><dd>${certified.length?comparable.length:'—'}</dd></div><div><dt>SIN REFERENCIA</dt><dd>${certified.length?certified.length-comparable.length:'—'}</dd></div></dl></div><div class="structural-cvd">${ready?(unusual?'Actividad inusual observada':'Sin actividad inusual en la muestra'):certified.length?'Referencia insuficiente':'Captura pendiente'}</div></article>`;
 }).join('');
 return `<section class="structural-map-preview option-activity-panel" aria-label="Actividad inusual por volumen y primas"><div class="section-head"><strong>ACTIVIDAD INUSUAL</strong><span>${esc(symbol)} · 5 MIN</span></div><div class="structural-price">${esc(reason)}</div><div class="horizons">${cards}</div><details class="legend" data-horizon-details="option-activity"><summary>Lectura rápida · volumen y primas</summary><p>Conteos de contratos con lectura en la última ventana cerrada de cinco minutos. Observados: evidencia certificada; comparables: referencia histórica suficiente; inusuales: lectura confirmada por cada motor.</p><p>La coincidencia requiere ambos motores y contexto de precio compatible. Prima bruta negociada; no identifica compras netas ni dirección CALL/PUT. Lectura experimental, sin probabilidad de éxito ni ajuste de la fórmula cuantitativa.</p></details></section>`;
}
