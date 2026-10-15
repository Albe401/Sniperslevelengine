import { invoke, isTauri } from './snapshot-observation';
import fixture from '../../fixtures/demo-market.json';
import { isSnapshot, type Snapshot } from './model';

export async function getSnapshot(): Promise<{ snapshot: Snapshot; transport: string }> {
  // No silent browser fallback if the native command fails.
  const native = isTauri();
  const snapshot: unknown = native ? await invoke('get_demo_snapshot') : fixture;
  if (!isSnapshot(snapshot)) throw new Error('El contrato de datos de demostración no es válido.');
  return { snapshot, transport: native ? 'Rust · Tauri' : 'Vista previa web' };
}
