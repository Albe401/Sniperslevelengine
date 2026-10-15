import {expect,it} from 'vitest';
import {optionActivityPanel} from './option-activity-panel';
import type {OptionResearchStatus} from './option-research-live';
const now=Date.parse('2026-10-12T14:40:20Z');
const status:OptionResearchStatus={status:'OBSERVING',audit:{generatedMs:now,reconciliation:[
 {underlying:'QQQ',date:'2026-10-12',endMs:now-20000,priceContextCompatible:true,volume:{comparable:true,unusual:true},premium:{comparable:false,unusual:false}},
 {underlying:'QQQ',date:'2026-10-12',endMs:now-20000,priceContextCompatible:true,volume:{comparable:true,unusual:false},premium:{comparable:true,unusual:true}},
 {underlying:'QQQ',date:'2026-10-12',endMs:now-320000,priceContextCompatible:true,volume:{comparable:true,unusual:true},premium:{comparable:true,unusual:true}}
]}};
it('shows latest closed window with pending premium references separate from zero activity',()=>{
 const html=optionActivityPanel('QQQ',true,now,status);
 expect(html).toContain('10:35–10:40 ET');
 expect(html.match(/<strong>1<\/strong>/g)).toHaveLength(2);
 expect(html).toContain('<strong>0</strong>');
 expect(html).toContain('<dt>SIN REFERENCIA</dt><dd>1</dd>');
 expect(html).toContain('data-horizon-details="option-activity"');
 expect(html).not.toContain('%');
});
it('does not show archived readings as current on stale status, disconnect or other symbol',()=>{
 for(const html of [optionActivityPanel('QQQ',false,now,status),optionActivityPanel('SPY',true,now,status),optionActivityPanel('QQQ',true,now+31000,status),optionActivityPanel('QQQ',true,now+500000,{...status,audit:{...status.audit,generatedMs:now+500000}})]){
  expect(html.match(/<strong>—<\/strong>/g)).toHaveLength(3);
  expect(html).not.toContain('<strong>1</strong>');
 }
});
