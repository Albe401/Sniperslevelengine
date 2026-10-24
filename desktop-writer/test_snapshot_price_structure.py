import importlib.util,json,pathlib,tempfile,unittest
from unittest.mock import patch
from types import SimpleNamespace
spec=importlib.util.spec_from_file_location('structure',pathlib.Path(__file__).with_name('snapshot_price_structure.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class StructureTests(unittest.TestCase):
    def test_date_close_and_consolidation_cache_invalidation(self):
        close=1792785600000 # 2026-10-23 16:00 ET
        h={'asOfMs':close+1000,'throughMs':close,'sessions':[{'closeMs':close}]}
        self.assertTrue(m._current(h,close+2000))
        self.assertFalse(m._current(h,close+1800000))
        self.assertFalse(m._current(h,close-1))
        self.assertFalse(m._current(h,close+86400000))
    def test_keeps_pipeline_available_when_kernel_unavailable(self):
        frame={'QQQ':{'symbol':'QQQ','publishedMs':1,'priceStructureReading':{'bad':True}}}
        with patch.object(m,'_config',side_effect=OSError()):result=m.enrich_frames(frame,1)
        self.assertEqual(result,{'QQQ':{'symbol':'QQQ','publishedMs':1}})
    def test_attaches_only_matching_asset_set(self):
        frames={'QQQ':{'symbol':'QQQ','publishedMs':1}}
        config={'node':'node','kernel':'kernel'}
        with patch.object(m,'_config',return_value=config),patch.object(m,'history_path',return_value=None),patch.object(m.subprocess,'run',return_value=SimpleNamespace(returncode=0,stdout=b'{"SPY":{}}')):
            self.assertNotIn('priceStructureReading',m.enrich_frames(frames,1)['QQQ'])
    def test_snapshot_input_uses_immutable_capture_time(self):
        frames={'QQQ':{'symbol':'QQQ','publishedMs':1}}
        with patch.object(m,'_config',return_value={'node':'n','kernel':'k'}),patch.object(m,'history_path',return_value=None),patch.object(m.subprocess,'run',return_value=SimpleNamespace(returncode=0,stdout=b'{"QQQ":{"version":"recorded"}}')) as run:
            result=m.enrich_frames(frames,1)
            self.assertEqual(json.loads(run.call_args.kwargs['input'])['capturedMs'],1)
            self.assertEqual(result['QQQ']['priceStructureReading']['version'],'recorded')
    def test_rejects_unsafe_history_paths(self):
        for symbol in ['../QQQ','spy','',None,'X'*16]:self.assertIsNone(m.history_path(symbol,1))
if __name__=='__main__':unittest.main()
