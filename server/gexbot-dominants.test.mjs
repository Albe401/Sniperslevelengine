import test from 'node:test';
import assert from 'node:assert/strict';
import {publicGexbot} from './server.mjs';
const captured=1792494600000;
const sample={source:'Gexbot Classic',symbol:'QQQ',category:'gex_full',generatedMs:captured-1000,positiveOi:718,negativeOi:700,positiveVolume:725,negativeVolume:718,zeroGamma:722.5,apiKey:'must-not-publish'};
test('publishes only licensed reference prices, never private fields',()=>{
 const value=publicGexbot(sample,'QQQ',captured);
 assert.equal(value.positiveVolume,725);
 assert.equal('apiKey' in value,false);
 assert.equal(publicGexbot(sample,'SPY',captured),undefined);
 assert.equal(publicGexbot({...sample,generatedMs:captured-600001},'QQQ',captured),undefined);
 assert.equal(publicGexbot({...sample,negativeOi:NaN},'QQQ',captured),undefined);
});

test('preserves genuine missing references without hiding other supplied dominants',()=>{const r=publicGexbot({...sample,negativeOi:null,zeroGamma:null},'QQQ',captured);assert.equal(r.positiveOi,718);assert.equal(r.negativeOi,null);assert.equal(r.zeroGamma,null);assert.equal(publicGexbot({...sample,positiveOi:null,negativeOi:null,positiveVolume:null,negativeVolume:null,zeroGamma:null},'QQQ',captured),undefined);assert.equal(publicGexbot(sample,'QQQ',NaN),undefined);});
