import {it,expect} from 'vitest';
import {analystPanel,type AnalystContext} from './analyst';
it('keeps external analysis disabled and does not present stale context as current',()=>{
  const now=Date.parse('2026-09-24T15:00:00Z');
  const context:AnalystContext={symbol:'QQQ',snapshot_generated_at_ms:now,data_quality:{evidence_timestamp_ms:now,stocks_phase:'streaming',options_phase:'streaming',snapshot_phase:'ready'},decision_runtime:{decision:{reason:'DAILY_LIMIT'}}};
  expect(analystPanel(context,true,true,false,'',now)).toContain('Contexto preparado');
  expect(analystPanel(context,true,true,false,'',now+15001)).toContain('Contexto sin vigencia');
  expect(analystPanel(context,false,true,false,'',now)).not.toContain('Contexto preparado');
  expect(analystPanel(undefined,false,false,false,'<script>',now)).toContain('&lt;script&gt;');
  expect(analystPanel(undefined,false,false,false,'',now)).toContain('data-analyst-context disabled');
});
