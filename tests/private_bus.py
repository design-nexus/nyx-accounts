#!/usr/bin/env python3
"""Credential-free IPC check; run under dbus-run-session after cargo build."""
import ast, socket, json, os, pathlib, shutil, subprocess, tempfile, time
root = pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='nyx-accounts-ipc-') as temporary:
    prefix = pathlib.Path(temporary)
    (prefix / 'bin').mkdir()
    for binary in ('nyx-accountsd', 'nyx-accountsctl'):
        shutil.copy2(root / 'target/debug' / binary, prefix / 'bin' / binary)
    env = dict(os.environ, XDG_DATA_HOME=str(prefix/'data'), XDG_RUNTIME_DIR=str(prefix/'runtime'))
    (prefix/'runtime').mkdir(mode=0o700)
    daemon = subprocess.Popen([prefix/'bin/nyx-accountsd'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    def cli(*args, data=None, check=True):
        return subprocess.run([prefix/'bin/nyx-accountsctl', *args], input=data, text=True, capture_output=True, env=env, check=check)
    try:
        for _ in range(100):
            result = cli('list', check=False)
            if result.returncode == 0:
                break
            if daemon.poll() is not None:
                raise AssertionError('daemon exited before accepting IPC')
            time.sleep(.03)
        assert json.loads(cli('list').stdout) == []
        metadata = dict(id='', provider='imap_smtp', identity='fixture@example.test', label='Fixture', enabled=['mail'], status='', client_id=None, endpoints={'imap':'imaps://mail.example.test:993'}, legacy_id=None)
        account_id = cli('register', data=json.dumps(metadata)).stdout.strip()
        account = json.loads(cli('list').stdout)[0]
        assert account['id'] == account_id and account['status'] == 'needs_sign_in'
        # An ordinary same-user executable must not become an account manager.
        denied = subprocess.run(['gdbus','call','--session','--dest','io.github.design_nexus.nyx.Accounts.Preview','--object-path','/io/github/design_nexus/nyx/Accounts','--method','io.github.design_nexus.nyx.Accounts1.Register',json.dumps(metadata)],env=env,capture_output=True,text=True)
        assert denied.returncode != 0 and 'AccessDenied' in denied.stderr
        handle = subprocess.run(['gdbus','call','--session','--dest','io.github.design_nexus.nyx.Accounts.Preview','--object-path','/io/github/design_nexus/nyx/Accounts','--method','io.github.design_nexus.nyx.Accounts1.BeginAccess',account_id],env=env,capture_output=True,text=True,check=True)
        path = ast.literal_eval(handle.stdout.strip())[0]
        with socket.socket(socket.AF_UNIX) as session:
            session.settimeout(3)
            session.connect(path)
            try:
                session.sendall(b'{"service":"mail"}\n')
                assert session.recv(4096) == b''
            except ConnectionResetError:
                pass
        metadata['provider']='exchange' 
        assert cli('register',data=json.dumps(metadata),check=False).returncode != 0
        assert len(json.loads(cli('list').stdout)) == 1
        print('PASS: metadata registration/listing, manager validation, unsupported provider rejection, isolated storage')
    finally:
        daemon.terminate()
        daemon.wait(timeout=10)
