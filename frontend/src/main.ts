import {snapshotEnabled,snapshotButton,snapshotPage,bindSnapshotMode} from './snapshot-mode';
import {startSnapshotSectionCollection} from './snapshot-observation';
import {globalNewsAlerts,globalNewsPopup,bindNewsAlerts,startNewsAlertWatcher} from './news-alerts';
import {etfDealersPage,bindEtfDealers,setEtfScannerVisible} from './etf-dealers';
import {configureBackgroundAlerts} from './chart-alerts';
import {globalBlockAlerts,bindBlockAlerts,startBlockAlertWatcher} from './block-alerts-ui';
import {selectLiveAsset} from './live';
import {investmentPage,investmentTabs,bindInvestment} from './investment';
import {neuroTraderPage,bindNeuroTrader} from './neuro-trader';
import {learningPage,bindLearning} from './learning';
import {botPage,bindBot} from './paper-bot';
import {clockDiagnosticPanel,bindClockDiagnostic} from './clock-diagnostic';
import {overnightPage,bindOvernight} from './overnight-page';
import '@fontsource-variable/inter';
import './styles.css';
import './restrained.css';
import './visual-readings.css';
import './brand.css';
import './day-map.css';
import './reading-tabs.css';
import './compact-reading-height.css';
import { createIcons, LayoutDashboard, AudioLines, Layers3, Cable, Crosshair, ChevronDown, ArrowUpRight, ArrowDownRight, Play, Pause, RotateCcw, SlidersHorizontal, ArrowRight, Radio, ShieldCheck, X, Check, ExternalLink } from 'lucide';
import { isTauri } from '@tauri-apps/api/core';
import { livePage, bindLive, initLive, massiveConnectionButton } from './live';

const icons = { LayoutDashboard, AudioLines, Layers3, Cable, Crosshair, ChevronDown, ArrowUpRight, ArrowDownRight, Play, Pause, RotateCcw, SlidersHorizontal, ArrowRight, Radio, ShieldCheck, X, Check, ExternalLink };
const icon = (name: string, cls = '') => `<i data-lucide="${name}" class="${cls}"></i>`;
const app = document.querySelector<HTMLDivElement>('#app')!;
const transport = isTauri() ? 'Rust · Tauri' : 'Vista previa web · sin conexión local';
let view = 'quick';

function connections(): string {
  return `${clockDiagnosticPanel()}<div class="connection-grid"><section class="panel connection-panel"><span class="eyebrow">FUENTES DE DATOS</span><h2>Una conexión, cada mercado</h2><p>Mercado, Flujo y Niveles comparten el mismo monitor del activo seleccionado.</p>
  ${[['Acciones y ETF','Massive · WebSocket Stocks','Disponible en Mercado, Flujo y Niveles'],['Opciones','Massive · WebSocket Options','Disponible en Mercado, Flujo y Niveles'],['Snapshots','Massive · REST','Griegas, IV e interés abierto'],['Futuros CME','Licencia pendiente · Conexión deshabilitada','Integración preparada y probada localmente. Acceso real pendiente de licencia.']].map(([title,sub,desc])=>`<div class="connection-row"><span class="connection-icon">${icon('cable')}</span><div><strong>${title}</strong><small>${sub}</small><p>${desc}</p></div><span class="tag">${title==='Acciones y ETF'||title==='Opciones'?'MONITOR':title==='Futuros CME'?'SIN LICENCIA':'INTEGRADO'}</span></div>`).join('')}</section>
  <section class="panel connection-panel"><span class="eyebrow">ESTADO DEL WORKSPACE</span><h2>Base de la nueva aplicación</h2><div class="status-line"><span>Interfaz</span><strong>${transport}</strong></div><div class="status-line"><span>Datos del monitor</span><strong>Massive · conexión manual</strong></div><div class="status-line"><span>Analista IA opcional</span><strong>Deshabilitado · sin envíos</strong></div><div class="status-line"><span>Órdenes a broker</span><strong>Alpaca Paper · Snipers Control</strong></div><div class="status-line"><span>Credenciales en la interfaz</span><strong>Ninguna</strong></div><div class="connection-note">La clave de Massive se carga desde el .env local en Rust. El monitor incorpora reconexión, recuperación histórica y análisis. Las entradas simuladas requieren habilitación explícita al conectar. Si instalas fuera del proyecto, configura SNIPERS_ENV_FILE con la ruta de tu archivo .env.</div><h3>Documentación oficial</h3><a href="https://massive.com/docs/websocket/quickstart" target="_blank" rel="noopener noreferrer">Massive WebSocket ${icon('external-link')}</a><a href="https://v2.tauri.app/" target="_blank" rel="noopener noreferrer">Tauri 2 ${icon('external-link')}</a></section></div>`;
}

function render(): void {
  setEtfScannerVisible(view==='etf-dealers');
  const realView=['gamma-live','spx','top-five','live','flow','quick','detailed','asset','gex'].includes(view);
  const title = {'gamma-live':'Mapa gamma en vivo','etf-dealers':'Scanner SPY / QQQ',neuro:'Neuro Trader Engine',spx:'Lectura SPX',investment:'Radar de inversión',learning:'Aprendizaje y análisis','top-five':'Top 5 activos',bot:'Snipers Control',gex:'Mapa GEX',quick:'Lectura rápida',detailed:'Lectura detallada',overnight:'Institutional Overnight Pressure',asset:'Lectura del activo',live:'Monitor de mercado',flow:'El pulso de las opciones.',connections:'Todo empieza con buenos datos.'}[view];
  const oldFocus = document.activeElement instanceof HTMLElement ? document.activeElement.dataset : undefined;
  app.innerHTML = `<div class="workspace ${view==='bot'?'bot-console':''}"><aside class="sidebar"><a class="brand" href="#" aria-label="Snipers Trading Academy, ir a Mercado en vivo"><img class="brand-lion" src="/brand/snipers-original-gold.png" alt="Logo original de Snipers Trading Academy" width="132" height="132" fetchpriority="high"></a><div class="workspace-label">MARKET WORKSPACE</div><nav aria-label="Navegación principal">${[
    ['bot','shield-check','Snipers Control'],['quick','layout-dashboard','Lectura rápida'],['top-five','layers-3','Top 5 activos'],['overnight','layout-dashboard','Presión overnight'],['asset','crosshair','Lectura del activo'],['spx','crosshair','Lectura SPX'],['etf-dealers','layers-3','Scanner SPY / QQQ'],['live','radio','Mercado en vivo'],['investment','crosshair','Radar de inversión'],['learning','audio-lines','Aprendizaje y análisis'],['neuro','layers-3','Neuro Trader Engine'],['connections','cable','Conexiones'],
  ].map(([id,ico,label])=>`<button data-view="${id}" aria-label="${label}" title="${label}" class="nav-item ${view===id?'active':''}" ${view===id?'aria-current="page"':''}>${icon(ico)}<span>${label}</span>${id===view?'<b></b>':''}</button>`).join('')}</nav>
    <div class="sidebar-bottom"><div class="workspace-card"><span class="card-orbit">${icon('shield-check')}</span><strong>Primero, el contexto.</strong><p>Un espacio para leer el mercado con claridad.</p><span>PREVIEW 0.1 <b>RUST + TAURI</b></span></div><button class="settings-button" data-action="settings">${icon('sliders-horizontal')} Preferencias <span>⌘</span></button><div class="profile"><span class="avatar">S</span><div><strong>Snipers Workspace</strong><small>Entorno de desarrollo</small></div></div></div>
  </aside><div class="main-shell"><header class="topbar"><div><span class="breadcrumb">Workspace</span><span class="slash">/</span>${view==='investment'?`<strong>Radar de inversión</strong>${investmentTabs()}`:['quick','detailed'].includes(view)?`<nav class="reading-view-tabs" aria-label="Vistas de lectura"><button data-view="quick" ${view==='quick'?'aria-current="page"':''}>Lectura rápida</button><button data-view="detailed" ${view==='detailed'?'aria-current="page"':''}>Lectura detallada</button></nav>`:`<strong>${{'gamma-live':'Mapa gamma en vivo','etf-dealers':'Scanner SPY / QQQ',neuro:'Neuro Trader Engine',spx:'Lectura SPX',investment:'Radar de inversión',learning:'Aprendizaje y análisis','top-five':'Top 5 activos',bot:'Snipers Control',gex:'Mapa GEX',quick:'Lectura rápida',detailed:'Lectura detallada',overnight:'Presión overnight',asset:'Lectura del activo',live:'Mercado en vivo',flow:'Flujo de opciones',connections:'Conexiones'}[view]}</strong>`}</div>${["quick","detailed"].includes(view)?'<span class="probability-header-title">Algoritmo de probabilidad</span>':''}<div class="topbar-right">${snapshotEnabled()?'':`<div data-global-news-alerts>${globalNewsAlerts()}</div>`}<div data-massive-connection>${massiveConnectionButton()}</div><span class="demo-pill"><span></span> ${view==='learning'?'APRENDIZAJE LOCAL':view==='bot'?'ALPACA PAPER':realView?'DATOS REALES':view==='connections'?'CONFIGURACIÓN':'DATOS REALES'}</span><span class="header-divider"></span><span class="timezone">Nueva York · ET</span><button class="icon-button" data-action="settings" aria-label="Abrir preferencias">${icon('sliders-horizontal')}</button></div></header>
  ${snapshotEnabled()?'':`<div data-news-popup-host>${globalNewsPopup()}</div><div data-global-block-alerts>${globalBlockAlerts()}</div>`}<main><div class="page-heading" ${view==='connections'?'':'hidden'}><div><h1>${title}</h1><p>Fuentes de datos y estado de integración.</p></div></div>
  ${snapshotEnabled()?snapshotPage(view):view==='overnight'?overnightPage():view==='etf-dealers'?etfDealersPage():view==='neuro'?neuroTraderPage():view==='investment'?investmentPage():view==='learning'?learningPage():view==='bot'?botPage():realView?livePage(view==='gamma-live'?'gamma-live':view==='spx'?'spx':view==='top-five'?'top-five':view==='gex'?'gex':view==='quick'?'quick':view==='detailed'?'detailed':view==='asset'?'asset':view==='live'?'monitor':view==='flow'?'flow':'monitor'):connections()}
  <footer><span><span class="small-dot"></span> ${transport} <span class="footer-separator">/</span> ${view==='learning'?'Aprendizaje local · solo lectura':realView?'Monitor Massive · simulador opcional':view==='connections'?'Configuración local':view==='bot'?'Alpaca Paper · datos Massive':'Datos de mercado'}</span><span>Diseñado para ver con claridad.<b> SNIPERS 0.1</b></span></footer></main></div></div>
  <dialog id="preferences"><div class="dialog-head"><div><span class="eyebrow">TU WORKSPACE</span><h2>Preferencias visuales</h2></div><button class="icon-button" data-action="close-settings" aria-label="Cerrar preferencias">${icon('x')}</button></div><p>Ajusta la interfaz a tu forma de analizar.</p><label class="setting-row"><span><strong>Modo compacto</strong><small>Más información en el mismo espacio.</small></span><input type="checkbox" id="compact" ${document.body.classList.contains('compact')?'checked':''}/></label><label class="setting-row"><span><strong>Reducir transparencia</strong><small>Paneles sólidos para mayor contraste.</small></span><input type="checkbox" id="solid" ${document.body.classList.contains('solid')?'checked':''}/></label><div class="snapshot-preference">${snapshotButton()}</div><div class="dialog-note">Tipografía Inter · cifras tabulares · fuentes incluidas localmente.</div><button class="primary-button" data-action="close-settings">Listo ${icon('check')}</button></dialog>`;
  createIcons({icons, attrs: {'stroke-width': 1.6}});
  bind();
  bindSnapshotMode(render);
  bindLive(); bindBot();
  bindClockDiagnostic();
  bindLearning(); bindInvestment(); bindBlockAlerts(); bindNewsAlerts(); bindNeuroTrader(); bindEtfDealers(); bindOvernight(app);
  if (oldFocus) {
    for (const k of ['symbol','view','filter','interval','action']) {
      if (oldFocus[k]) { app.querySelector<HTMLButtonElement>(`[data-${k}="${oldFocus[k]}"]`)?.focus({preventScroll:true}); break; }
    }
  }
}

function bind(): void {
  app.querySelectorAll<HTMLButtonElement>('[data-view]').forEach(b=>b.onclick=()=>{view=b.dataset.view!;render();window.scrollTo({top:0,left:0,behavior:'instant'});});
  app.querySelector<HTMLAnchorElement>('.brand')!.onclick=e=>{e.preventDefault();view='live';render();window.scrollTo({top:0,left:0,behavior:'instant'});};
  app.querySelectorAll<HTMLButtonElement>('[data-action]').forEach(b=>b.onclick=()=>{
    switch(b.dataset.action) {
      case 'settings':render();app.querySelector<HTMLDialogElement>('#preferences')!.showModal();break;
      case 'close-settings':app.querySelector<HTMLDialogElement>('#preferences')!.close();break;
    }
  });
  for(const [id,cls] of [['compact','compact'],['solid','solid']]) {
    const input=app.querySelector<HTMLInputElement>(`#${id}`)!;
    input.onchange=()=>{document.body.classList.toggle(cls,input.checked);};
  }
}

async function boot() {
  try {
    await initLive();
    render();
    startBlockAlertWatcher(); startNewsAlertWatcher();
    startSnapshotSectionCollection();
  } catch(error) {
    app.innerHTML='<div class="boot-error"><h1>No se pudo iniciar el workspace</h1><p id="boot-message"></p><button id="retry">Reintentar</button></div>';
    app.querySelector('#boot-message')!.textContent=error instanceof Error?error.message:'Error de inicialización';
    app.querySelector<HTMLButtonElement>('#retry')!.onclick=boot;
  }
}
configureBackgroundAlerts(symbol=>{view='quick';render();void selectLiveAsset(symbol);});
void boot();


