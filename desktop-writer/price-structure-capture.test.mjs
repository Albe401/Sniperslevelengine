import {test} from 'node:test';import assert from 'node:assert/strict';import {mkdtempSync,writeFileSync,utimesSync,unlinkSync,rmdirSync} from 'node:fs';import {tmpdir} from 'node:os';import {join} from 'node:path';import {spawnSync} from 'node:child_process';
test('frozen writer rejects a history file completed after the immutable capture',()=>{
 const directory=mkdtempSync(join(tmpdir(),'snipers-structure-'));
 try{
  const capturedMs=Date.parse('2026-10-24T10:00:00Z'),file=join(directory,'QQQ.json');
  writeFileSync(file,JSON.stringify({symbol:'QQQ',asOfMs:capturedMs-1000,throughMs:capturedMs-86400000,sessions:[],days:[],intraday:{sessions:[],minutes:[]}}));
  const run=()=>{
   const p=spawnSync(process.execPath,[new URL('price-structure-capture.mjs',import.meta.url).pathname.replace(/^\/(?=[A-Z]:)/,'')],{input:JSON.stringify({capturedMs,frames:{QQQ:{symbol:'QQQ',publishedMs:capturedMs,chartHistory:{sessions:[],minutes:[]}}},histories:{QQQ:file}}),encoding:'utf8'});
   assert.equal(p.status,0,p.stderr);return JSON.parse(p.stdout).QQQ;
  };
  utimesSync(file,new Date(capturedMs+1000),new Date(capturedMs+1000));assert.equal(run().historyAsOfMs,null);
  utimesSync(file,new Date(capturedMs-1000),new Date(capturedMs-1000));assert.equal(run().historyAsOfMs,capturedMs-1000);
 }finally{unlinkSync(join(directory,'QQQ.json'));rmdirSync(directory);}
});
