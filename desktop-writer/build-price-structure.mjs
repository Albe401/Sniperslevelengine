import {createRequire} from 'node:module';
import {realpathSync} from 'node:fs';
import {resolve} from 'node:path';
const root=resolve(import.meta.dirname,'..');
const require=createRequire(realpathSync(resolve(root,'frontend/node_modules/vite/package.json')));
const {build}=require('esbuild');
await build({entryPoints:[resolve(root,'desktop-writer/price-structure-capture.ts')],bundle:true,platform:'node',format:'esm',outfile:resolve(root,'desktop-writer/price-structure-capture.mjs')});
