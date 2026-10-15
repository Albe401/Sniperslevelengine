import {expect,it} from 'vitest';
import {assetDataHealth} from './asset-data-health';
import type {LiveState} from './live';
it('explica que páginas recibidas no equivalen a ausencia de prints',()=>{
 const s={optionSnapshotStatus:{contracts:828,oiMissing:0,greeksMissing:171,detail:'Snapshot recibido'},offExchange:{history:{'2026-09-25':{phase:'loading',pages:10,stockTrades:500000,offExchangeTrades:290717}}}} as unknown as LiveState;
 const html=assetDataHealth(s);expect(html).toContain('828 contratos');expect(html).toContain('171');expect(html).toContain('10 páginas');expect(html).toContain('290.717');expect(html).toContain('0 / 6');expect(html).toContain('no equivale a cero');
});
it('expone errores de descarga de forma segura',()=>{const s={offExchange:{history:{'2026-09-25':{phase:'failed',detail:'<error>'}},error:'<TRF>'}} as unknown as LiveState;const html=assetDataHealth(s);expect(html).toContain('&lt;error&gt;');expect(html).toContain('reintento automático');expect(html).not.toContain('<TRF>');});
