#!/usr/bin/env python3
"""Disposable fixture checks; no desktop app or user store is opened."""
import concurrent.futures, json, os, pathlib, shutil, signal, sqlite3, subprocess, tempfile
root=pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='nyx-preview-fixture-') as temporary:
    base=pathlib.Path(temporary)
    prefix=base/'prefix';(prefix/'bin').mkdir(parents=True)
    shutil.copy2(root/'packaging/nyx-online-accounts-preview',prefix/'bin/nyx-online-accounts-preview')
    shutil.copy2(root/'packaging/nyx-preview-session',prefix/'bin/nyx-preview-session')
    fake=prefix/'bin/nyx-accountsctl'
    fake.write_text('#!/usr/bin/python3\nimport os,json\nprint(json.dumps({key:os.environ[key] for key in ["XDG_RUNTIME_DIR","XDG_DATA_HOME","DBUS_SESSION_BUS_ADDRESS","NYX_ACCOUNTS_SECRET_BUS_ADDRESS","WAYLAND_DISPLAY"]}))\n')
    fake.chmod(0o755)
    runtime=base/'runtime';runtime.mkdir(mode=0o700)
    sentinel=runtime/'nyx-cloud';sentinel.mkdir();(sentinel/'rclone.sock').write_text('original-runtime-sentinel')
    address='unix:path='+str(runtime/'desktop-bus')
    env=dict(os.environ,HOME=str(base/'home'),XDG_RUNTIME_DIR=str(runtime),DBUS_SESSION_BUS_ADDRESS=address,WAYLAND_DISPLAY='wayland-fixture')
    # Appearance follows the desktop through copies, while other config stays private.
    config=base/'desktop-config'; state=base/'desktop-state'
    (config/'nyx').mkdir(parents=True);(state/'omarchy/current/theme').mkdir(parents=True)
    (config/'nyx/theme.toml').write_text('mode = "omarchy"\n')
    (config/'nyx/unrelated.toml').write_text('leave-on-desktop')
    (state/'omarchy/current/theme/colors.toml').write_text('accent = "#123456"\n')
    env.update(XDG_CONFIG_HOME=str(config),XDG_STATE_HOME=str(state))
    env.pop('NYX_ACCOUNTS_ORIGINAL_CONFIG_HOME',None);env.pop('NYX_ACCOUNTS_ORIGINAL_STATE_HOME',None)
    for key in ['NYX_ACCOUNTS_SECRET_BUS_ADDRESS','NYX_ACCOUNTS_ORIGINAL_RUNTIME','NYX_ORIGINAL_DATA_HOME']:
        env.pop(key,None)
    result=subprocess.run([prefix/'bin/nyx-online-accounts-preview','nyx-accountsctl'],env=env,capture_output=True,text=True,check=True)
    values=json.loads(result.stdout)
    copied_config=base/'home/.local/share/nyx-online-accounts-preview/config/nyx'
    copied_colors=base/'home/.local/share/nyx-online-accounts-preview/state/omarchy/current/theme/colors.toml'
    assert (copied_config/'theme.toml').read_text()==(config/'nyx/theme.toml').read_text()
    assert not (copied_config/'theme.toml').samefile(config/'nyx/theme.toml')
    assert not (copied_config/'unrelated.toml').exists()
    assert copied_colors.read_text()==(state/'omarchy/current/theme/colors.toml').read_text()

    assert pathlib.Path(values['XDG_RUNTIME_DIR'])==runtime/'nyx-online-accounts-preview'
    assert values['NYX_ACCOUNTS_SECRET_BUS_ADDRESS']==address
    assert values['DBUS_SESSION_BUS_ADDRESS']!=address
    assert values['WAYLAND_DISPLAY']==str(runtime/'wayland-fixture')
    assert (sentinel/'rclone.sock').read_text()=='original-runtime-sentinel'
    def launch(_):
        result=subprocess.run([prefix/'bin/nyx-online-accounts-preview','nyx-accountsctl'],env=env,capture_output=True,text=True)
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)['DBUS_SESSION_BUS_ADDRESS']
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        assert set(pool.map(launch,range(4)))=={values['DBUS_SESSION_BUS_ADDRESS']}
    bus_env=dict(env,DBUS_SESSION_BUS_ADDRESS=values['DBUS_SESSION_BUS_ADDRESS'])
    def bus_id():
        return subprocess.check_output(['gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.GetId'],env=bus_env,text=True)
    # Desktop services must never activate a second keyring/portal on this bus.
    activatable=subprocess.check_output(['gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.ListActivatableNames'],env=bus_env,text=True)
    assert 'org.freedesktop.secrets' not in activatable
    for name in ['org.freedesktop.secrets','org.freedesktop.portal.Desktop']:
        attempt=subprocess.run(['gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.StartServiceByName',name,'0'],env=bus_env,capture_output=True,text=True)
        assert attempt.returncode != 0 and 'ServiceUnknown' in attempt.stderr
    original_id=bus_id()
    launch(None)
    assert bus_id()==original_id
    os.kill(int((runtime/'nyx-online-accounts-preview/bus.pid').read_text()),signal.SIGTERM)
    # Reap only the private fixture bus; restart must recover from a stopped session.
    import time
    time.sleep(.1)
    launch(None)
    assert bus_id()!=original_id
    os.kill(int((runtime/'nyx-online-accounts-preview/bus.pid').read_text()),signal.SIGTERM)
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
    print('PASS: shared private bus, concurrent launch and restart, private runtime, desktop keyring route, Wayland routing, untouched original runtime, SQLite snapshot, non-overwrite copy')
