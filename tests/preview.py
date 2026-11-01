#!/usr/bin/env python3
"""Disposable fixture checks; no desktop app or user store is opened."""
import json, os, pathlib, shutil, sqlite3, subprocess, tempfile
root=pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='nyx-preview-fixture-') as temporary:
    base=pathlib.Path(temporary)
    prefix=base/'prefix';(prefix/'bin').mkdir(parents=True)
    shutil.copy2(root/'packaging/nyx-online-accounts-preview',prefix/'bin/nyx-online-accounts-preview')
    fake=prefix/'bin/nyx-accountsctl'
    fake.write_text('#!/usr/bin/python3\nimport os,json\nprint(json.dumps({key:os.environ[key] for key in ["XDG_RUNTIME_DIR","XDG_DATA_HOME","DBUS_SESSION_BUS_ADDRESS","NYX_ACCOUNTS_SECRET_BUS_ADDRESS","WAYLAND_DISPLAY"]}))\n')
    fake.chmod(0o755)
    runtime=base/'runtime';runtime.mkdir(mode=0o700)
    sentinel=runtime/'nyx-cloud';sentinel.mkdir();(sentinel/'rclone.sock').write_text('original-runtime-sentinel')
    address='unix:path='+str(runtime/'desktop-bus')
    env=dict(os.environ,HOME=str(base/'home'),XDG_RUNTIME_DIR=str(runtime),DBUS_SESSION_BUS_ADDRESS=address,WAYLAND_DISPLAY='wayland-fixture')
    for key in ['NYX_ACCOUNTS_SECRET_BUS_ADDRESS','NYX_ACCOUNTS_ORIGINAL_RUNTIME','NYX_ORIGINAL_DATA_HOME']:
        env.pop(key,None)
    result=subprocess.run([prefix/'bin/nyx-online-accounts-preview','nyx-accountsctl'],env=env,capture_output=True,text=True,check=True)
    values=json.loads(result.stdout)
    assert pathlib.Path(values['XDG_RUNTIME_DIR'])==runtime/'nyx-online-accounts-preview'
    assert values['NYX_ACCOUNTS_SECRET_BUS_ADDRESS']==address
    assert values['DBUS_SESSION_BUS_ADDRESS']!=address
    assert values['WAYLAND_DISPLAY']==str(runtime/'wayland-fixture')
    assert (sentinel/'rclone.sock').read_text()=='original-runtime-sentinel'
    source=base/'source/nyx-mail';source.mkdir(parents=True)
    with sqlite3.connect(source/'mail.sqlite') as db:
        db.execute('PRAGMA journal_mode=WAL');db.execute('CREATE TABLE fixture(value TEXT)');db.execute("INSERT INTO fixture VALUES('preserved')");db.commit()
        (source/'message-wal').write_text('ordinary-message-file')
        env['NYX_ORIGINAL_DATA_HOME']=str(base/'source')
        subprocess.run([root/'packaging/nyx-local-copy','nyx-mail'],env=env,capture_output=True,text=True,check=True)
    destination=base/'home/.local/share/nyx-online-accounts-preview/data/nyx-mail'
    with sqlite3.connect(destination/'mail.sqlite') as copied:
        assert copied.execute('SELECT value FROM fixture').fetchone()[0]=='preserved'
    assert (destination/'message-wal').read_text()=='ordinary-message-file'
    assert subprocess.run([root/'packaging/nyx-local-copy','nyx-mail'],env=env,capture_output=True).returncode!=0
    print('PASS: private runtime/bus, desktop keyring route, Wayland routing, untouched original runtime, SQLite snapshot, non-overwrite copy')
