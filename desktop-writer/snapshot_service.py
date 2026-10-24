"""Five-minute durable outbox -> Neon. No provider sockets or order execution."""
import argparse, datetime, hashlib, hmac, json, math, os, pathlib, secrets, socket, sqlite3, struct
import subprocess, sys, threading, time, uuid, ctypes, gzip, base64
from zoneinfo import ZoneInfo
from snapshot_price_structure import enrich_frames
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

ROOT = pathlib.Path(__file__).resolve().parents[1]
DATA = ROOT / 'data/snapshots'
sys.path.insert(0, str(ROOT / 'data/snapshot-runtime'))
SYMBOLS = ['AAPL','MSFT','AMZN','GOOGL','META','NVDA','TSLA','QQQ','SPY','IWM']
INTERVAL = 300_000
PORT = 36543
STATE = {'version':'snipers-snapshots-v1','database':'pending','feed':'pending','lastCapturedMs':None,'lastStoredMs':None,'pending':0}
LOCK = threading.Lock()
EXTRAS = {}
STOP = threading.Event()

def now_ms(): return time.time_ns() // 1_000_000
def encode(value): return json.dumps(value, ensure_ascii=False, separators=(',', ':'), sort_keys=True, allow_nan=False)
def digest(value): return hashlib.sha256(encode(value).encode()).hexdigest()
def connection(secret_file=None):
    # Windows DPAPI: never place the connection string in argv, UI or logs.
    class Blob(ctypes.Structure): _fields_=[('size',ctypes.c_uint32),('data',ctypes.POINTER(ctypes.c_ubyte))]
    encrypted=bytes.fromhex((secret_file or DATA/'connection.secret').read_text(encoding='utf-8-sig').strip())
    buffer=ctypes.create_string_buffer(encrypted)
    source=Blob(len(encrypted),ctypes.cast(buffer,ctypes.POINTER(ctypes.c_ubyte))); target=Blob()
    if not ctypes.windll.crypt32.CryptUnprotectData(ctypes.byref(source),None,None,None,None,0,ctypes.byref(target)):
        raise RuntimeError('credential_unavailable')
    try: return ctypes.string_at(target.data,target.size).decode('utf-16-le')
    finally:
        ctypes.memset(target.data,0,target.size);ctypes.windll.kernel32.LocalFree(target.data)

def connect_db():
    if now_ms()<STATE.get('databaseRetryAtMs',0):raise RuntimeError('database_quota_exceeded')
    from snapshot_neon_http import Connection
    web_secret=ROOT/'data/web-runtime/connection.secret'
    # The newly supplied database is the direct snapshot destination. This does
    # not alter Massive feed credentials or drive the selected desktop view.
    return Connection(connection(web_secret if web_secret.exists() else None))

def record_database_error(error):
    message=str(error).lower()
    quota=message=='database_quota_exceeded' or ('quota' in message and ('402' in message or 'exceed' in message))
    with LOCK:
        STATE['database']='quota_exceeded' if quota else 'unavailable'
        if quota and now_ms()>=STATE.get('databaseRetryAtMs',0):STATE['databaseRetryAtMs']=now_ms()+INTERVAL
    return quota

def local_db():
    DATA.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(DATA / 'outbox.sqlite', timeout=10)
    db.execute('PRAGMA journal_mode=WAL')
    db.execute('CREATE TABLE IF NOT EXISTS outbox(id TEXT PRIMARY KEY, slot INTEGER UNIQUE, captured INTEGER, payload TEXT, sha TEXT, sent INTEGER DEFAULT 0)')
    db.execute('CREATE INDEX IF NOT EXISTS outbox_captured_desc ON outbox(captured DESC)')
    db.execute('CREATE INDEX IF NOT EXISTS outbox_pending_captured ON outbox(sent,captured DESC)')
    db.execute('CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY, value TEXT)')
    db.execute('CREATE TABLE IF NOT EXISTS photo_jobs(id TEXT PRIMARY KEY, state TEXT DEFAULT \'pending\')')
    db.execute('INSERT OR IGNORE INTO metadata VALUES (?,?)', ('writer', str(uuid.uuid4())))
    db.commit()
    return db

def receive_exact(sock, count):
    data = bytearray()
    while len(data) < count:
        part = sock.recv(count-len(data))
        if not part: raise RuntimeError('ipc_incomplete')
        data.extend(part)
    return bytes(data)

def ipc(port, method):
    with socket.create_connection(('127.0.0.1', port), timeout=5) as sock:
        request = encode({'method':method}).encode()
        sock.sendall(struct.pack('>I', len(request)) + request)
        size = struct.unpack('>I', receive_exact(sock,4))[0]
        if size > 32*1024*1024: raise RuntimeError('ipc_size')
        value = json.loads(receive_exact(sock,size))
        if value.get('protocol')!='snipers-data-service-v1' or not value.get('ok'): raise RuntimeError('ipc_method_unavailable')
        return value['result']

def bundle():
    inbox=DATA/'inbox'
    for path in sorted(inbox.glob('capture-*.json'),reverse=True)[:3]:
        try:
            value=json.loads(path.read_text(encoding='utf-8'));validate_bundle(value)
            if 0<=now_ms()-value['capturedMs']<=INTERVAL+15000:
                return value,'native-file-handoff-v1'
        except (OSError,RuntimeError,ValueError):pass
    for port in [36541,36542]:
        try:
            value=ipc(port,'snapshot_bundle')
            validate_bundle(value)
            return value, f'local-ipc-{port}'
        except (OSError,RuntimeError,ValueError): pass
    # Older installed receivers can provide only their selected full frame.
    # Keep other assets explicitly absent; never fabricate full coverage.
    for port in [36541,36542]:
        try:
            frame=ipc(port,'snapshot')
            frames={s: {'symbol':s,'publishedMs':None,'snapshotUnavailable':True} for s in SYMBOLS}
            if frame.get('symbol') in frames: frames[frame['symbol']]=frame
            return {'version':'snipers-snapshot-bundle-v1','capturedMs':now_ms(),'symbols':SYMBOLS,'frames':frames,
                    'coverage':{'independentLanes':0,'allTicksCertified':False,'receiverUpgradeRequired':True}}, f'legacy-ipc-{port}'
        except (OSError,RuntimeError,ValueError): pass
    raise RuntimeError('feed_unavailable')

def validate_bundle(value):
    if set(value.get('frames',{})) != set(SYMBOLS): raise ValueError('asset_set')
    captured=value['capturedMs']
    if not isinstance(captured,int) or captured>now_ms()+1000: raise ValueError('capture_clock')
    for symbol,frame in value['frames'].items():
        if frame.get('symbol')!=symbol: raise ValueError('asset_identity')
        if isinstance(frame.get('publishedMs'),int) and frame['publishedMs']>captured: raise ValueError('future_frame')

def sections(frame, captured):
    result={}
    for key,value in frame.items():
        missing=value is None or value=={} or value==[]
        result[key]={'status':'missing' if missing else 'recorded','capturedMs':captured,'sourceField':key,'sha256':digest(value)}
        if missing:result[key]['value']=None
    return result

def archive_frame(frame):
    raw=encode(frame).encode('utf-8')
    return base64.b64encode(gzip.compress(raw,mtime=0)).decode(),hashlib.sha256(raw).hexdigest()

def decode_frame(value):
    if not isinstance(value,dict) or 'compressedBase64' not in value:return value
    raw=gzip.decompress(base64.b64decode(value['compressedBase64']))
    if hashlib.sha256(raw).hexdigest()!=value['sha256']:raise ValueError('database_frame_integrity')
    return json.loads(raw)

def gexbot_dominants(symbol, captured):
    """Only licensed reference prices; newest causal observation, including explicit absences."""
    if not isinstance(symbol,str) or not symbol.isascii() or not symbol.isalnum() or isinstance(captured,bool) or not isinstance(captured,int):return None
    try:
        date=datetime.datetime.fromtimestamp(captured/1000,datetime.timezone.utc).astimezone(ZoneInfo('America/New_York')).date().isoformat()
        path=ROOT/'data/gexbot-private'/symbol/'gex_full'/f'{date}.live.jsonl'
        with path.open('rb') as file:
            file.seek(0,2);file.seek(max(0,file.tell()-65536));raw_rows=file.read().splitlines()
        candidates=[]
        for raw in raw_rows:
            try:
                value=json.loads(raw);observed=value['generatedMs']
                if isinstance(observed,bool) or not isinstance(observed,int) or not 0<=captured-observed<=600000:continue
                candidates.append(value)
            except (ValueError,TypeError,KeyError):continue
        if not candidates:return None
        latest=max(value['generatedMs'] for value in candidates);keys=('positiveOi','negativeOi','positiveVolume','negativeVolume','zeroGamma');canonical=None
        for value in candidates:
            if value['generatedMs']!=latest:continue
            levels={key:value.get(key) for key in keys}
            if any(v is not None and (not isinstance(v,(int,float)) or isinstance(v,bool) or not math.isfinite(v) or v<=0) for v in levels.values()):return None
            if canonical is not None and canonical!=levels:return None
            canonical=levels
        if canonical is None or all(v is None for v in canonical.values()):return None
        return {'source':'Gexbot Classic','symbol':symbol,'category':'gex_full','generatedMs':latest,**canonical}
    except (OSError,OverflowError,ValueError):return None

def storage_layout(payload):
    # Large contract inventories belong to the lossless per-asset archive,
    # rather than an uncompressed JSONB batch-wide extras field.
    frames={symbol:dict(frame) for symbol,frame in payload['bundle']['frames'].items()}
    extras={}
    for key,record in payload.get('extras',{}).items():
        symbol=record.get('args',{}).get('symbol')
        if record.get('command')=='get_contract_inventory' and symbol in frames:
            frames[symbol]['contractChain']={'key':key,'observation':record}
        elif record.get('command')=='get_iv_history' and symbol in frames:
            frames[symbol]['ivHistory']={'key':key,'observation':record}
        else:extras[key]=record
    return frames,extras

def stage(db,value,source):
    validate_bundle(value)
    # Do not replace a useful saved batch with the ten empty startup lanes.
    # Recorded failures and partial observations still publish normally.
    if value.get('coverage',{}).get('independentLanes')==len(SYMBOLS) and all(
        not frame.get('chartHistory',{}).get('minutes')
        and frame.get('history',{}).get('phase','') in {'','pending','loading'}
        and frame.get('optionSnapshotStatus',{}).get('phase','') in {'','pending','loading'}
        for frame in value['frames'].values()
    ): raise RuntimeError('capture_warming_up')
    captured=value['capturedMs']; slot=captured//INTERVAL*INTERVAL
    allowed={'symbol','publishedMs','startedMs','stocks','options','ticks','bars','minutes','chartHistory','history','metrics','optionSnapshotStatus','selection','flow','flowExpiry','cvd','offExchange','dailyAtr','dailyRange','positionMaps','wallPull','wallFlow','adjustedReadings','adjustedAlignment','levelDeltas','magnetConfirmations','optionBaselineRecords','gapReport','priceAlerts','qqq_report','treasury','auctionMap','structuralPaths','directionOi','rawOptions','levelDynamics','intradayAtr','priceNow','snapshotUnavailable'}
    allowed.update({'rangeContext','localVolume','greekSupport','thetaVega','optionLiquidity','swingReport','neuroContext','qqqReport','optionTiming','etfBlocks','etfCvd','etfFlow','dataService'})
    frames={}
    for symbol,frame in value['frames'].items():
        saved={k:v for k,v in frame.items() if k in allowed and k!='gexbotDominants'}
        reading=gexbot_dominants(symbol,captured)
        if reading:saved['gexbotDominants']=reading
        frames[symbol]=saved
    frames=enrich_frames(frames,captured)
    value={**value,'frames':frames,'webSections':['quick','detailed','contract-chain'],'coverage':{**value.get('coverage',{}),'webSections':['quick','detailed','contract-chain'],'format':'json','photosEnabled':False}}
    with LOCK: extras=json.loads(encode({k:v for k,v in EXTRAS.items() if v['observedMs']<=captured and v['command'] in {'get_contract_inventory','get_iv_history','get_fmp_targets','preview_route_4h'}}))
    # Capture existing IV evidence without starting reconstruction jobs or market calls.
    for symbol in value['frames']:
        for directory in [pathlib.Path(os.environ.get('LOCALAPPDATA',''))/'com.snipers.workspace/iv-history',ROOT/'data/iv-history']:
            path=directory/f'{symbol}.json'
            try:
                before=path.stat(); observed=before.st_mtime_ns//1_000_000
                if observed>captured:continue
                history=json.loads(path.read_text(encoding='utf-8-sig'))
                if path.stat().st_mtime_ns!=before.st_mtime_ns or history.get('symbol')!=symbol:continue
                observations=history.get('observations')
                if not isinstance(observations,list) or any(not isinstance(row.get('at'),(int,float)) or row['at']>captured for row in observations):continue
                extras[f'local-iv-{symbol}']={'command':'get_iv_history','args':{'symbol':symbol},'observedMs':observed,'value':history}
                break
            except (OSError,ValueError,TypeError,AttributeError):continue
    payload={'bundle':secret_free(value),'source':source,'extras':extras}
    text=encode(payload); identity=str(uuid.uuid4())
    db.execute('INSERT OR IGNORE INTO outbox(id,slot,captured,payload,sha) VALUES(?,?,?,?,?)', (identity,slot,captured,text,digest(payload)))
    db.commit()
    return slot

def migrate():
    with connect_db() as conn:
        with conn.transaction():
            for statement in (ROOT/'scripts/snapshot-schema.sql').read_text().split(';'):
                if statement.strip(): conn.execute(statement)

def flush(db):
    rows=db.execute('SELECT id,slot,captured,payload,sha FROM outbox WHERE sent=0 ORDER BY captured DESC LIMIT 4').fetchall()
    if not rows: return 0
    from psycopg.types.json import Jsonb
    writer=db.execute("SELECT value FROM metadata WHERE key='writer'").fetchone()[0]
    with connect_db() as conn:
        for identity,slot,captured,text,sha in rows:
            payload=json.loads(text); value=payload['bundle']; validate_bundle(value)
            if digest(payload)!=sha: raise ValueError('outbox_integrity')
            frames,extras=storage_layout(payload)
            with conn.transaction():
                conn.execute('INSERT INTO snipers_snapshots.batches(id,writer_id,slot_ms,captured_ms,schema_version,source,asset_count,sha256,coverage,extras) VALUES(%s,%s,%s,%s,%s,%s,10,%s,%s,%s) ON CONFLICT(id) DO NOTHING',
                    (identity,writer,slot,captured,value['version'],payload['source'],sha,Jsonb(value['coverage']),Jsonb(extras)))
                for symbol,frame in frames.items():
                    compressed,frame_sha=archive_frame(frame)
                    index={'symbol':symbol,'publishedMs':frame.get('publishedMs'),'archivedLosslessly':True}
                    conn.execute("INSERT INTO snipers_snapshots.assets(batch_id,symbol,published_ms,payload,sections,payload_compressed,payload_sha256,payload_codec) VALUES(%s,%s,%s,%s,%s,decode(%s,'base64'),%s,'gzip-json-v1') ON CONFLICT(batch_id,symbol) DO NOTHING",
                        (identity,symbol,frame.get('publishedMs'),Jsonb(index),Jsonb(sections(frame,captured)),compressed,frame_sha))
            db.execute('UPDATE outbox SET sent=1 WHERE id=?',(identity,));db.commit()
            if value['coverage'].get('independentLanes')==10 and not (ROOT/'data/web-runtime/connection.secret').exists():
                db.execute('INSERT OR IGNORE INTO photo_jobs(id) VALUES(?)',(identity,));db.commit()
            with LOCK: STATE['lastStoredMs']=max(STATE.get('lastStoredMs') or 0,captured)
    return len(rows)

def latest(at=None):
    cutoff=(now_ms() if at is None else at)-INTERVAL
    # Read the batch and all ten assets in one SQL statement: no mixed batches.
    with connect_db() as conn:
        row=conn.execute('''SELECT b.id::text,b.captured_ms,b.coverage,b.extras,jsonb_object_agg(a.symbol,CASE WHEN a.payload_compressed IS NULL THEN a.payload ELSE jsonb_build_object('compressedBase64',encode(a.payload_compressed,'base64'),'sha256',a.payload_sha256) END),
            (SELECT coalesce(jsonb_object_agg(i.symbol,jsonb_build_object('base64',encode(i.image,'base64'),'mimeType',i.mime_type,'sha256',i.image_sha256,'sourceSha256',i.source_sha256)), '{}'::jsonb) FROM snipers_snapshots.images i WHERE i.batch_id=b.id)
            FROM snipers_snapshots.batches b JOIN snipers_snapshots.assets a ON a.batch_id=b.id
            WHERE b.id=(SELECT id FROM snipers_snapshots.batches WHERE captured_ms<=%s ORDER BY captured_ms DESC,id DESC LIMIT 1)
            GROUP BY b.id HAVING count(*)=10''',(cutoff,)).fetchone()
    if not row: return {'available':False,'minimumDelayMs':INTERVAL}
    return {'available':True,'id':row[0],'capturedMs':int(row[1]),'coverage':row[2],'extras':row[3],'frames':{s:decode_frame(v) for s,v in row[4].items()},'images':row[5],
            'minimumDelayMs':INTERVAL,'ageMs':now_ms()-int(row[1]),'mode':'snapshot'}

def safe_command(command):
    return command in {'get_contract_inventory','get_iv_history','get_fmp_targets','preview_route_4h'}

def secret_free(value):
    if isinstance(value,dict):
        return {k:secret_free(v) for k,v in value.items() if not any(x in k.lower() for x in ['password','secret','api_key','apikey','token','authorization','connectionstring'])}
    if isinstance(value,list): return [secret_free(v) for v in value]
    if isinstance(value,str) and ('postgresql://' in value or 'postgres://' in value): return '[REDACTED]'
    return value

def token():
    path=DATA/'api.token'
    if not path.exists(): path.write_text(secrets.token_urlsafe(32))
    return path.read_text().strip()

def write_cache(name,value):
    temporary=DATA/(name+'.'+uuid.uuid4().hex+'.tmp')
    try:
        temporary.write_text(encode(value),encoding='utf-8')
        for attempt in range(6):
            try:
                os.replace(temporary,DATA/name)
                return
            except PermissionError:
                if attempt==5:raise
                STOP.wait(0.05*(attempt+1))
    finally:
        try:temporary.unlink(missing_ok=True)
        except OSError:pass

def drain_observations():
    # Bound startup catch-up so capture/flush health is not starved by old observations.
    for path in sorted((DATA/'observations').glob('*.json'),reverse=True)[:8]:
        try:
            if path.stat().st_size>64*1024*1024:
                path.unlink();continue
            value=json.loads(path.read_text(encoding='utf-8'));command=value.get('command','')
            if safe_command(command):
                key=digest({'command':command,'args':value.get('args',{})})
                observed=int(path.name.split('-')[0])
                with LOCK:
                    if EXTRAS.get(key,{}).get('observedMs',-1)>=observed:
                        path.unlink();continue
                    EXTRAS[key]={'command':command,'args':secret_free(value.get('args',{})),'observedMs':observed,'value':secret_free(value.get('value'))}
                    while len(EXTRAS)>32 or len(encode(EXTRAS).encode())>256*1024*1024:EXTRAS.pop(next(iter(EXTRAS)))
                    archive=DATA/'sections';archive.mkdir(exist_ok=True)
                    temporary=archive/(key+'.tmp');temporary.write_text(encode(EXTRAS[key]),encoding='utf-8');os.replace(temporary,archive/(key+'.json'))
            path.unlink()
        except (OSError,ValueError):pass

def restore_observations():
    # Restarting the helper must not erase the last saved chain for each asset.
    for path in (DATA/'sections').glob('*.json'):
        try:
            if path.stat().st_size>64*1024*1024:continue
            record=json.loads(path.read_text(encoding='utf-8'))
            if safe_command(record.get('command','')) and record.get('observedMs',now_ms()+1)<=now_ms():EXTRAS[path.stem]=record
        except (OSError,ValueError):pass
    restored_symbols={r.get('args',{}).get('symbol') for r in EXTRAS.values() if r.get('command')=='get_contract_inventory'}
    if set(SYMBOLS)<=restored_symbols and any(r.get('command')=='get_fmp_targets' for r in EXTRAS.values()):return
    db=local_db()
    try:
        for row in db.execute('SELECT payload FROM outbox ORDER BY captured DESC LIMIT 5'):
            for key,record in json.loads(row[0]).get('extras',{}).items():
                if key not in EXTRAS and safe_command(record.get('command','')) and record.get('observedMs',now_ms()+1)<=now_ms():EXTRAS[key]=record
    finally:db.close()

def cache_loop():
    last_id=None
    while not STOP.is_set():
        try:
            # Check only a tiny ID first. Download/decode the large ten-asset
            # bundle once per new eligible batch, never on every UI refresh.
            with connect_db() as conn:
                row=conn.execute('SELECT b.id::text,(SELECT count(*) FROM snipers_snapshots.images i WHERE i.batch_id=b.id) FROM snipers_snapshots.batches b WHERE captured_ms<=%s AND asset_count=10 ORDER BY captured_ms DESC LIMIT 1',(now_ms()-INTERVAL,)).fetchone()
            if row and tuple(row)!=last_id:
                value=latest()
                if value.get('available'):
                    write_cache('latest-from-neon.json',value);last_id=(value['id'],len(value.get('images',{})))
        except Exception as error:record_database_error(error)
        STOP.wait(30)

def render_photos(db,identity,text,sha):
    payload=json.loads(text);folder=DATA/'photos'/identity;folder.mkdir(parents=True,exist_ok=True)
    input_file=folder/'source.json';input_file.write_text(text,encoding='utf-8')
    config=json.loads((DATA/'runtime.json').read_text(encoding='utf-8-sig'))
    if not (folder/'images.json').exists():
        result=subprocess.run([config['node'],str(ROOT/'scripts/render-snapshot-images.mjs'),str(input_file),str(folder)],capture_output=True,text=True,encoding='utf-8',timeout=120,creationflags=0x08004000)
        if result.returncode:raise RuntimeError('snapshot_image_render_failed')
    images=json.loads((folder/'images.json').read_text(encoding='utf-8'))
    if {v['symbol'] for v in images['images']}!=set(SYMBOLS):raise ValueError('image_asset_set')
    with connect_db() as conn:
        with conn.transaction():
            for record in images['images']:
                raw=(folder/record['file']).read_bytes()
                if hashlib.sha256(raw).hexdigest()!=record['sha256'] or not raw.startswith(b'\xff\xd8'):raise ValueError('image_integrity')
                conn.execute("INSERT INTO snipers_snapshots.images(batch_id,symbol,mime_type,image,image_sha256,source_sha256,generated_ms,renderer_version) VALUES(%s,%s,'image/jpeg',decode(%s,'base64'),%s,%s,%s,'detailed-reading-image-v1') ON CONFLICT(batch_id,symbol) DO NOTHING",
                    (identity,record['symbol'],base64.b64encode(raw).decode(),record['sha256'],sha,now_ms()))
    db.execute("UPDATE photo_jobs SET state='stored' WHERE id=?",(identity,));db.commit()

def photo_loop():
    db=local_db()
    while not STOP.is_set():
        row=db.execute("SELECT o.id,o.payload,o.sha FROM photo_jobs p JOIN outbox o ON o.id=p.id WHERE p.state='pending' ORDER BY o.captured DESC LIMIT 1").fetchone()
        if row:
            try:render_photos(db,*row)
            except Exception:
                with LOCK:STATE['images']='pending_retry'
            else:
                with LOCK:STATE['images']='stored'
        STOP.wait(10)

class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args): pass
    def respond(self,status,value):
        body=encode(value).encode(); self.send_response(status)
        self.send_header('Content-Type','application/json');self.send_header('Cache-Control','no-store')
        self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def authorized(self): return hmac.compare_digest(self.headers.get('X-Snipers-Token',''),token())
    def do_GET(self):
        if not self.authorized(): self.respond(403,{'error':'unauthorized'});return
        route=urlparse(self.path).path
        try:
            if route=='/status':
                with LOCK: result=dict(STATE)
                self.respond(200,result)
            elif route=='/latest': self.respond(200,latest())
            else: self.respond(404,{'error':'not_found'})
        except Exception: self.respond(503,{'error':'snapshot_database_unavailable'})
    def do_POST(self):
        if not self.authorized(): self.respond(403,{'error':'unauthorized'});return
        try:
            size=int(self.headers.get('Content-Length','0'))
            if size<1 or size>64*1024*1024: self.respond(413,{'error':'size'});return
            value=json.loads(self.rfile.read(size)); command=value.get('command','')
            if self.path!='/observe' or not safe_command(command): self.respond(400,{'error':'command'});return
            with LOCK:
                if len(EXTRAS)>=256: EXTRAS.pop(next(iter(EXTRAS)))
                key=digest({'command':command,'args':value.get('args',{})})
                EXTRAS[key]={'command':command,'args':secret_free(value.get('args',{})),
                    'observedMs':now_ms(),'value':secret_free(value.get('value'))}
                while len(encode(EXTRAS).encode())>256*1024*1024 and EXTRAS: EXTRAS.pop(next(iter(EXTRAS)))
            self.respond(200,{'ok':True})
        except Exception: self.respond(400,{'error':'invalid_observation'})

def capture_loop():
    db=local_db(); migrated=False; last_slot=None
    row=db.execute('SELECT max(slot),max(captured),max(CASE WHEN sent=1 THEN captured END) FROM outbox').fetchone()
    if row and row[0] is not None:
        last_slot=row[0]
        with LOCK:STATE.update(lastCapturedMs=row[1],lastStoredMs=row[2])
    while not STOP.is_set():
        drain_observations()
        clock=now_ms(); slot=clock//INTERVAL*INTERVAL
        if last_slot is None or slot>last_slot:
            try:
                value,source=bundle(); last_slot=stage(db,value,source)
                with LOCK: STATE.update(feed='connected',lastCapturedMs=value['capturedMs'])
            except Exception as error:
                with LOCK: STATE['feed']='warming_up' if str(error)=='capture_warming_up' else 'unavailable'
        try:
            if not migrated: migrate(); migrated=True
            flush(db)
            with LOCK: STATE.update(database='connected',databaseRetryAtMs=0)
        except Exception as error:record_database_error(error)
        with LOCK: STATE['pending']=db.execute('SELECT count(*) FROM outbox WHERE sent=0').fetchone()[0]
        with LOCK: health=dict(STATE)
        health['writtenMs']=now_ms()
        try:write_cache('status.json',health)
        except OSError:
            # A Windows reader may temporarily deny replacement. Health-file
            # publication must never stop capture or durable database delivery.
            pass
        STOP.wait(5)

def main():
    if os.name=='nt':
        # Persistent below-normal scheduling also applies after automatic startup.
        ctypes.windll.kernel32.SetPriorityClass(ctypes.c_void_p(-1),0x4000)
    parser=argparse.ArgumentParser();parser.add_argument('--migrate',action='store_true');parser.add_argument('--once',action='store_true')
    args=parser.parse_args(); DATA.mkdir(parents=True,exist_ok=True)
    if args.migrate: migrate();print(encode({'schema':'ready'}));return
    if args.once:
        migrate();db=local_db();value,source=bundle();stage(db,value,source);count=flush(db)
        print(encode({'stored':count,'coverage':value['coverage'],'capturedMs':value['capturedMs']}));return
    restore_observations();token();server=ThreadingHTTPServer(('127.0.0.1',PORT),Handler)
    threading.Thread(target=capture_loop,daemon=True).start()
    threading.Thread(target=cache_loop,daemon=True).start()
    if not (ROOT/'data/web-runtime/connection.secret').exists():
        threading.Thread(target=photo_loop,daemon=True).start()
    try: server.serve_forever()
    finally: STOP.set();server.server_close()

if __name__=='__main__':
    try: main()
    except Exception as error:
        # Driver errors can contain connection strings. Expose only type.
        print(encode({'error':'snapshot_service_failed','type':type(error).__name__}));sys.exit(1)
