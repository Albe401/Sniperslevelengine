import {it,expect} from 'vitest';
import {legacyContent,type LegacyPage} from './legacy-journal';
it('preserves open legacy status and does not fabricate missing results',()=>{
  const data:LegacyPage={source:'LEGACY_ARCHIVE',total:2,openTrades:2,closedTrades:0,nextBeforeId:12,entries:[{id:15,session:'2026-09-22',timestampMs:0,status:'OPEN',record:{symbol:'WMT',side:'CALL',entry_price:109.68,pnl_points:null,exit_reason:'<script>'}}]};
  const html=legacyContent(data,false,'','',undefined,true);expect(html).toContain('Abierta en el origen');expect(html).toContain('&lt;script&gt;');expect(html).not.toContain('<script>');expect(html).toContain('data-legacy="import" disabled');expect(html).toContain('data-legacy="next" >');
  expect(legacyContent(undefined,false,'','',undefined,false)).toContain('data-legacy="preview" disabled');
});
