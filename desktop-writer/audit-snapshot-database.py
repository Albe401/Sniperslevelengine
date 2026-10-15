"""Independent readback of actual persisted data; no fabricated coverage."""
import hashlib,json,pathlib,time,urllib.request,base64
import snapshot_service as s
OUT=s.ROOT/'docs/audits/2026-10-14-snapshots';OUT.mkdir(parents=True,exist_ok=True)
with s.connect_db() as db:
    rows=db.execute("SELECT b.id::text,b.captured_ms,b.sha256,b.coverage,b.extras,b.source,jsonb_object_agg(a.symbol,CASE WHEN a.payload_compressed IS NULL THEN a.payload ELSE jsonb_build_object('compressedBase64',encode(a.payload_compressed,'base64'),'sha256',a.payload_sha256) END) FROM (SELECT * FROM snipers_snapshots.batches ORDER BY captured_ms DESC LIMIT 1) b JOIN snipers_snapshots.assets a ON a.batch_id=b.id GROUP BY b.id,b.captured_ms,b.sha256,b.coverage,b.extras,b.source ORDER BY b.captured_ms DESC LIMIT 1").fetchall()
assert rows,'No real capture in DB'
checks=[]
local=s.local_db()
for identity,captured,sha,coverage,extras,source,frames in rows:
    frames={symbol:s.decode_frame(frame) for symbol,frame in frames.items()}
    stored=local.execute('SELECT payload,sha FROM outbox WHERE id=?',(identity,)).fetchone()
    assert stored,'Missing local provenance'
    payload=json.loads(stored[0]);expected,_=s.storage_layout(payload)
    assert set(frames)==set(s.SYMBOLS) and frames==expected
    assert s.digest(payload)==sha==stored[1]
    frame_coverage={symbol:{'publishedMs':frame.get('publishedMs'),'available':not frame.get('snapshotUnavailable',False),'keys':len(frame)} for symbol,frame in frames.items()}
    checks.append({'id':identity,'capturedMs':int(captured),'sha256':sha,'frameCount':len(frames),'contractChains':sum('contractChain' in f for f in frames.values()),'exactFrameRoundtrip':True,'coverage':coverage,'assets':frame_coverage})
latest=s.latest();assert not latest['available'] or s.now_ms()-latest['capturedMs']>=s.INTERVAL
image_checks=[]
for symbol,record in latest.get('images',{}).items():
    raw=base64.b64decode(record['base64']);assert raw[:2]==b'\xff\xd8'
    assert hashlib.sha256(raw).hexdigest()==record['sha256']
    original=local.execute('SELECT sha FROM outbox WHERE id=?',(latest['id'],)).fetchone()
    assert original and original[0]==record['sourceSha256']
    image_checks.append({'symbol':symbol,'bytes':len(raw),'hashVerified':True,'sameCaptureVerified':True})
report={'checkedAtMs':s.now_ms(),'captures':checks,'images':image_checks,'delayGuard':True,'latestAvailable':latest['available'],
        'scope':'Actual saved ten-asset frames and immutable rendered images. Missing engine inputs remain pending; not certification of every market tick.'}
(OUT/'database-real-readback.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
if latest['available']:(OUT/'snapshot-preview-input.json').write_text(json.dumps(latest),encoding='utf-8')
print(json.dumps({'capturesChecked':len(checks),'tenRowsEach':True,'exactReadback':True,'delayGuard':True,'coverage':checks[0]['coverage']}))
