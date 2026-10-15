import {it,expect,vi} from 'vitest';
const {invoke}=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke,isTauri:()=>true}));
import {bindInventory,inventoryProfile,inventoryPanel} from './contract-inventory';
import type {EtfReport} from './etf-dealers';
const at=Date.now(),date=new Date(at).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
const report=(symbol:string)=>({symbol,days:183,at,contracts:2,enrichment:{details:[{ticker:'O:'+symbol+'C',kind:'call',expiration:date,strike:100,oi:50,volume:3,dex:null,gex:null},{ticker:'O:'+symbol+'P',kind:'put',expiration:date,strike:100,oi:70,volume:null,dex:null,gex:null}]}} as EtfReport);
function ui(){const call={dataset:{inventorySideTab:'call'},onclick:undefined as undefined|(()=>void)},put={dataset:{inventorySideTab:'put'},onclick:undefined as undefined|(()=>void)};const button={onclick:undefined as undefined|(()=>void)},select={onchange:undefined};vi.stubGlobal('document',{querySelector:()=>({querySelector:(selector:string)=>selector.includes('refresh')?button:select,querySelectorAll:(selector:string)=>selector.includes('side-tab')?[call,put]:[]})});return {button,select,call,put};}
it('loads the Radar chain once, renders exact totals, and keeps foreign results separate',async()=>{
 const controls=ui();let resolve!:(r:EtfReport)=>void;invoke.mockReturnValueOnce(new Promise<EtfReport>(r=>resolve=r));const refresh=vi.fn();
 bindInventory('QQQ',refresh);bindInventory('QQQ',refresh);expect(invoke).toHaveBeenCalledTimes(1);expect(invoke).toHaveBeenCalledWith('get_contract_inventory',{symbol:'QQQ'});
 expect(inventoryProfile('QQQ',at)).toBeUndefined();resolve(report('QQQ'));await vi.waitFor(()=>expect(inventoryProfile('QQQ',at)?.contracts).toBe(2));
 expect(inventoryProfile('QQQ',at)!.rows[0].put.oi).toBe(70);expect(inventoryProfile('SPY',at)).toBeUndefined();
 const html=inventoryPanel('QQQ',at);expect(html).toContain('TOTAL DEL FILTRO');expect(html).toContain('OI 50');expect(html).toContain('OI 70');expect(html).toContain('Sin datos');expect(html).toContain('O:QQQC');expect(html).toContain('Página 1 / 1');
 controls.put.onclick!();const puts=inventoryPanel('QQQ',at);expect(puts).toContain('<th>O:QQQP</th>');expect(puts).not.toContain('<th>O:QQQC</th>');expect(puts).toContain('Contratos PUT · 1 series');controls.call.onclick!();const calls=inventoryPanel('QQQ',at);expect(calls).toContain('<th>O:QQQC</th>');expect(calls).not.toContain('<th>O:QQQP</th>');
 expect(inventoryProfile('QQQ',at+300001)).toBeUndefined();
});
it('shows a rejected provider query rather than presenting zero contracts or a false complete total',async()=>{
 const {button}=ui();invoke.mockRejectedValueOnce(new Error('Acceso rechazado 403 <script>'));bindInventory('AAPL',vi.fn());await vi.waitFor(()=>expect(inventoryPanel('AAPL',at)).toContain('Acceso rechazado 403'));
 expect(inventoryPanel('AAPL',at)).not.toContain('<script>');expect(inventoryPanel('AAPL',at)).not.toContain('TOTAL DEL FILTRO');expect(button.onclick).toBeTypeOf('function');
});
