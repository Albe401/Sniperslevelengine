"""Frozen structural evidence; bounded REST history cache, no orders or UI control."""
import datetime, hashlib, json, os, pathlib, queue, subprocess, threading, time, uuid
from zoneinfo import ZoneInfo

ROOT=pathlib.Path(__file__).resolve().parents[1]
CACHE=ROOT/'data/price-structure-snapshot'
_queue=queue.Queue(maxsize=187)
_pending=set()
_retry={}
_lock=threading.Lock()
_started=False

def _config():
    return json.loads((CACHE/'runtime.json').read_text(encoding='utf-8'))

def _current(history,now):
    """Refresh on date rollover, exchange close and after the 30-minute consolidation period."""
    stamp=history.get('asOfMs',0)
    day=lambda t:datetime.datetime.fromtimestamp(t/1000,datetime.timezone.utc).astimezone(ZoneInfo('America/New_York')).date()
    if not isinstance(stamp,int) or stamp>now or day(stamp)!=day(now):return False
    closes=[s['closeMs'] for s in history.get('sessions',[]) if s['closeMs']<=now]
    if not closes or history.get('throughMs')!=max(closes):return False
    return not (stamp<max(closes)+1800000<=now)

def _worker():
    while True:
        symbol=_queue.get()
        temporary=CACHE/(symbol+'.'+uuid.uuid4().hex+'.tmp')
        try:
            config=_config()
            result=subprocess.run([config['historyExe'],symbol,str(temporary)],cwd=ROOT,capture_output=True,timeout=180,creationflags=0x08000000 if os.name=='nt' else 0)
            if result.returncode:raise ValueError('history_capture_failed')
            value=json.loads(temporary.read_text(encoding='utf-8'))
            if value.get('symbol')!=symbol or not _current(value,time.time_ns()//1000000):raise ValueError('history_identity_or_clock')
            os.replace(temporary,CACHE/(symbol+'.json'))
        except Exception:
            # Provider or driver exception text must never leak credentials.
            with _lock:_retry[symbol]=time.monotonic()+60
        finally:
            temporary.unlink(missing_ok=True)
            with _lock:_pending.discard(symbol)
            _queue.task_done()

def history_path(symbol,captured):
    global _started
    if not isinstance(symbol,str) or not symbol or len(symbol)>15 or any(c not in 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.' for c in symbol):return None
    if not (CACHE/'runtime.json').exists():return None
    path=CACHE/(symbol+'.json')
    try:
        value=json.loads(path.read_text(encoding='utf-8'))
        current=value.get('symbol')==symbol and _current(value,captured)
    except (ValueError,OSError,KeyError,TypeError):current=False
    if not current:
        with _lock:
            if not _started:
                for _ in range(2):threading.Thread(target=_worker,daemon=True).start()
                _started=True
            if symbol not in _pending and time.monotonic()>=_retry.get(symbol,0):
                try:_queue.put_nowait(symbol);_pending.add(symbol)
                except queue.Full:pass
        return None
    return str(path)

def enrich_frames(frames,captured):
    """Only evidence acquired before the immutable snapshot may be included."""
    try:
        config=_config()
        paths={s:p for s in frames if (p:=history_path(s,captured))}
        data=json.dumps({'frames':frames,'capturedMs':captured,'histories':paths},allow_nan=False)
        result=subprocess.run([config['node'],config['kernel']],input=data.encode('utf-8'),capture_output=True,cwd=ROOT,timeout=30,creationflags=0x08000000 if os.name=='nt' else 0)
        if result.returncode:raise ValueError('structure_kernel_failed')
        readings=json.loads(result.stdout)
        if set(readings)!=set(frames):raise ValueError('structure_asset_set')
        return {s:{**frame,'priceStructureReading':readings[s]} for s,frame in frames.items()}
    except Exception:
        # Keep the snapshot pipeline available; the web explicitly shows missing long history.
        return {s:{k:v for k,v in frame.items() if k!='priceStructureReading'} for s,frame in frames.items()}
