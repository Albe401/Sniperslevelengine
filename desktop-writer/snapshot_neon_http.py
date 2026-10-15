"""Small DB adapter around the official Neon driver, using atomic HTTP batches."""
import contextlib, json, pathlib, subprocess
ROOT=pathlib.Path(__file__).resolve().parents[1]

class Result:
    def __init__(self, rows): self.rows=rows
    def fetchone(self): return self.rows[0] if self.rows else None
    def fetchall(self): return self.rows

class Connection:
    def __init__(self, connection): self.connection=connection;self.queued=None
    def __enter__(self): return self
    def __exit__(self,*args): pass
    def send(self,queries):
        node=ROOT/'data/snapshots/runtime.json'
        config=json.loads(node.read_text(encoding='utf-8-sig'))
        result=subprocess.run([config['node'],str(ROOT/'scripts/snapshot-neon-query.mjs')],
            input=json.dumps({'connection':self.connection,'queries':queries}), capture_output=True,text=True,
            encoding='utf-8',timeout=30,creationflags=0x08000000)
        if result.returncode:
            try: safe=json.loads(result.stdout)
            except ValueError: safe={'error':'neon_http_unavailable'}
            raise RuntimeError(json.dumps(safe))
        values=json.loads(result.stdout)
        if not isinstance(values,list): raise RuntimeError('neon_http_invalid')
        return values
    def execute(self,query,params=()):
        # Only internal fixed SQL uses %s; no user-generated SQL is accepted.
        for index in range(len(params)): query=query.replace('%s',f'${index+1}',1)
        clean=[json.dumps(p.obj) if hasattr(p,'obj') else p for p in params]
        item={'query':query,'params':clean}
        if self.queued is not None: self.queued.append(item);return Result([])
        return Result(self.send([item])[0].get('rows',[]))
    @contextlib.contextmanager
    def transaction(self):
        if self.queued is not None: raise RuntimeError('nested_transaction_not_supported')
        self.queued=[]
        try:
            yield self
            if self.queued:self.send(self.queued)
        finally:self.queued=None
