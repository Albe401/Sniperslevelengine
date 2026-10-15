import contextlib, json, pathlib, sqlite3, tempfile, unittest
from unittest.mock import patch
import snapshot_service as service
from snapshot_neon_http import Connection

class SnapshotTests(unittest.TestCase):
    def test_cache_recovers_from_windows_replace_lock(self):
        original=service.os.replace
        with tempfile.TemporaryDirectory(dir=service.DATA) as folder,patch.object(service,'DATA',pathlib.Path(folder)),patch.object(service.STOP,'wait'):
            with patch.object(service.os,'replace',side_effect=[PermissionError('locked'),None]) as replace:
                service.write_cache('status.json',{'alive':True})
                self.assertEqual(replace.call_count,2)
            self.assertEqual(list(pathlib.Path(folder).glob('*.tmp')),[])
    def test_cache_persistent_lock_is_bounded_and_cleans_temporary(self):
        with tempfile.TemporaryDirectory(dir=service.DATA) as folder,patch.object(service,'DATA',pathlib.Path(folder)),patch.object(service.STOP,'wait'),patch.object(service.os,'replace',side_effect=PermissionError('locked')) as replace:
            with self.assertRaises(PermissionError):service.write_cache('status.json',{})
            self.assertEqual(replace.call_count,6)
            self.assertEqual(list(pathlib.Path(folder).glob('*.tmp')),[])
    def test_recovery_sends_newest_first_and_preserves_monotonic_saved_time(self):
        sent=[]
        class Fake:
            def __enter__(self):return self
            def __exit__(self,*args):pass
            def transaction(self):return contextlib.nullcontext()
            def execute(self,sql,args=()):
                if 'INSERT INTO snipers_snapshots.batches' in sql:sent.append(args[3])
        with tempfile.TemporaryDirectory(dir=service.DATA) as folder,patch.object(service,'DATA',pathlib.Path(folder)),patch.object(service,'connect_db',return_value=Fake()),patch.dict(service.STATE,{'lastStoredMs':None}):
            db=service.local_db();new=self.bundle();old=json.loads(json.dumps(new));old['capturedMs']-=service.INTERVAL
            for f in old['frames'].values():f['publishedMs']=old['capturedMs']
            service.stage(db,old,'old');service.stage(db,new,'new');self.assertEqual(service.flush(db),2)
            self.assertEqual(sent,[new['capturedMs'],old['capturedMs']]);self.assertEqual(service.STATE['lastStoredMs'],new['capturedMs'])
            self.assertEqual(db.execute('SELECT count(*) FROM outbox WHERE sent=0').fetchone()[0],0);db.close()
    def test_old_observation_does_not_overwrite_newer_section(self):
        with tempfile.TemporaryDirectory(dir=service.DATA) as folder,patch.object(service,'DATA',pathlib.Path(folder)),patch.dict(service.EXTRAS,{},clear=True):
            inbox=pathlib.Path(folder)/'observations';inbox.mkdir();key=service.digest({'command':'get_fmp_targets','args':{}})
            service.EXTRAS[key]={'command':'get_fmp_targets','args':{},'observedMs':200,'value':{'latest':True}}
            (inbox/'100-1-1.json').write_text(json.dumps({'command':'get_fmp_targets','args':{},'value':{'old':True}}),encoding='utf-8')
            service.drain_observations();self.assertEqual(service.EXTRAS[key]['observedMs'],200);self.assertEqual(service.EXTRAS[key]['value'],{'latest':True})
    def test_quota_backoff_keeps_local_capture_and_does_not_slide_retry_forever(self):
        with patch.dict(service.STATE,{},clear=True),patch.object(service,'now_ms',return_value=1_000_000):
            self.assertTrue(service.record_database_error(RuntimeError('Server error HTTP status 402: project exceeded quota')))
            retry=service.STATE['databaseRetryAtMs']
            service.record_database_error(RuntimeError('database_quota_exceeded'))
            self.assertEqual(service.STATE['databaseRetryAtMs'],retry)
            with self.assertRaisesRegex(RuntimeError,'database_quota_exceeded'):service.connect_db()
            with tempfile.TemporaryDirectory(dir=service.DATA) as folder,patch.object(service,'DATA',pathlib.Path(folder)):
                db=service.local_db();service.stage(db,self.bundle(),'test-local-during-quota')
                self.assertEqual(db.execute('SELECT count(*) FROM outbox WHERE sent=0').fetchone()[0],1);db.close()
    def test_transient_errors_are_not_reported_as_quota(self):
        with patch.dict(service.STATE,{},clear=True):
            self.assertFalse(service.record_database_error(RuntimeError('network timeout')))
            self.assertEqual(service.STATE['database'],'unavailable')
    def test_empty_startup_lanes_do_not_replace_saved_data(self):
        with tempfile.TemporaryDirectory(dir=service.DATA) as folder,patch.object(service,'DATA',pathlib.Path(folder)):
            db=service.local_db();b=self.bundle();b['coverage']={'independentLanes':10}
            with self.assertRaisesRegex(RuntimeError,'capture_warming_up'):service.stage(db,b,'native-file-handoff-v1')
            self.assertEqual(db.execute('SELECT count(*) FROM outbox').fetchone()[0],0)
            b['frames']['QQQ']['history']={'phase':'failed'}
            service.stage(db,b,'native-file-handoff-v1')
            self.assertEqual(db.execute('SELECT count(*) FROM outbox').fetchone()[0],1)
            db.close()
    def test_chain_storage_is_lossless_and_not_duplicated_in_batch_extras(self):
        payload={'bundle':self.bundle(),'extras':{'chain-key':{'command':'get_contract_inventory','args':{'symbol':'AAPL'},'observedMs':1,'value':{'contracts':[{'strike':101.25,'oi':123}]}}}}
        frames,extras=service.storage_layout(payload)
        self.assertNotIn('chain-key',extras)
        archived,sha=service.archive_frame(frames['AAPL']);decoded=service.decode_frame({'compressedBase64':archived,'sha256':sha})
        self.assertEqual(decoded['contractChain']['observation'],payload['extras']['chain-key'])
    def test_web_scope_excludes_other_sections_and_photos(self):
        with tempfile.TemporaryDirectory(dir=service.DATA) as folder,patch.object(service,'DATA',pathlib.Path(folder)):
            db=service.local_db();b=self.bundle();b['frames']['AAPL']['learningReport']={'must_not_ship':True}
            with patch.dict(service.EXTRAS,{'unrelated':{'command':'get_benzinga_news','observedMs':0,'value':{}},'chain':{'command':'get_contract_inventory','observedMs':0,'value':{},'args':{'symbol':'AAPL'}}},clear=True):service.stage(db,b,'test')
            payload=json.loads(db.execute('SELECT payload FROM outbox').fetchone()[0]);db.close()
            self.assertEqual(payload['bundle']['webSections'],['quick','detailed','contract-chain'])
            self.assertNotIn('learningReport',payload['bundle']['frames']['AAPL'])
            self.assertNotIn('unrelated',payload['extras']);self.assertIn('chain',payload['extras'])
    def bundle(self):
        now=service.now_ms()
        return {'version':'test','capturedMs':now,'coverage':{},'frames':{s:{'symbol':s,'publishedMs':now,'auctionMap':None} for s in service.SYMBOLS}}
    def test_identity_and_future_guard(self):
        b=self.bundle();service.validate_bundle(b)
        b['frames']['AAPL']['symbol']='QQQ'
        with self.assertRaises(ValueError): service.validate_bundle(b)
        b=self.bundle();b['frames']['AAPL']['publishedMs']+=1
        with self.assertRaises(ValueError): service.validate_bundle(b)
    def test_slot_idempotency_and_restart_recovery(self):
        with tempfile.TemporaryDirectory(dir=service.ROOT/'data/snapshots') as folder,patch.object(service,'DATA',pathlib.Path(folder)):
            db=service.local_db();b=self.bundle();service.stage(db,b,'test');service.stage(db,b,'retry')
            self.assertEqual(db.execute('SELECT count(*) FROM outbox').fetchone()[0],1)
            db.close();db=service.local_db()
            self.assertEqual(db.execute('SELECT count(*) FROM outbox WHERE sent=0').fetchone()[0],1)
            payload,sha=db.execute('SELECT payload,sha FROM outbox').fetchone()
            self.assertEqual(service.digest(json.loads(payload)),sha);db.close()
    def test_missing_never_becomes_complete(self):
        section=service.sections({'auctionMap':None},1000)['auctionMap']
        self.assertEqual(section['status'],'missing');self.assertIsNone(section['value'])
    def test_credentials_and_orders_excluded(self):
        value=service.secret_free({'apiKey':'secret','normal':4,'nested':{'password':'secret'},'uri':'postgresql://private'})
        self.assertNotIn('apiKey',value);self.assertEqual(value['normal'],4);self.assertEqual(value['uri'],'[REDACTED]')
        for command in ['send_order','get_alpaca_paper_snapshot','get_api_credentials']:self.assertFalse(service.safe_command(command))
    def test_atomic_batch_queries_and_rollback_on_error(self):
        db=Connection('unused');sent=[]
        db.send=lambda queries:sent.append(list(queries)) or []
        with db.transaction():
            db.execute('INSERT INTO x VALUES(%s,%s)',(1,2));db.execute('INSERT INTO y VALUES(%s)',(3,))
        self.assertEqual(len(sent),1);self.assertEqual(len(sent[0]),2);self.assertEqual(sent[0][0]['query'],'INSERT INTO x VALUES($1,$2)')
        with self.assertRaises(ValueError):
            with db.transaction():db.execute('INSERT x');raise ValueError('simulate failure')
        self.assertEqual(len(sent),1)
    def test_read_uses_cutoff_and_single_batch(self):
        calls=[]
        class Fake:
            def __enter__(self):return self
            def __exit__(self,*args):pass
            def execute(self,sql,args):calls.append((sql,args));return self
            def fetchone(self):return None
        with patch.object(service,'connect_db',return_value=Fake()):
            result=service.latest(1000000)
        self.assertEqual(calls[0][1],(700000,));self.assertIn('HAVING count(*)=10',calls[0][0]);self.assertFalse(result['available'])
    def test_native_file_handoff_works_without_any_loopback(self):
        with tempfile.TemporaryDirectory(dir=service.ROOT/'data/snapshots') as folder,patch.object(service,'DATA',pathlib.Path(folder)):
            inbox=pathlib.Path(folder)/'inbox';inbox.mkdir();b=self.bundle()
            (inbox/f"capture-{b['capturedMs']}-1.json").write_text(json.dumps(b),encoding='utf-8')
            with patch.object(service,'ipc',side_effect=AssertionError('loopback should not be used')):
                actual,source=service.bundle()
            self.assertEqual(actual,b);self.assertEqual(source,'native-file-handoff-v1')
    def test_lossless_archive_and_corruption_guard(self):
        frame=self.bundle()['frames']['AAPL'];compressed,sha=service.archive_frame(frame)
        self.assertEqual(service.decode_frame({'compressedBase64':compressed,'sha256':sha}),frame)
        with self.assertRaises(ValueError):service.decode_frame({'compressedBase64':compressed,'sha256':'wrong'})

if __name__=='__main__':unittest.main()

