#!/usr/bin/env python3
"""Disposable services exit when their owning private bus ends; no user stores or mounts."""
import os, pathlib, shutil, signal, subprocess, tempfile, time
root=pathlib.Path(__file__).resolve().parents[1]
for repository,binary,name in [
    (root,'nyx-accountsd','io.github.design_nexus.nyx.Accounts.Preview'),
    (root.parent/'nyx-cloud','nyx-cloudd','io.github.design_nexus.nyx.Cloud1'),
]:
    with tempfile.TemporaryDirectory(prefix='nyx-service-lifecycle-') as temporary:
        base=pathlib.Path(temporary);(base/'bin').mkdir();(base/'runtime').mkdir(mode=0o700);(base/'home').mkdir(mode=0o700)
        shutil.copy2(repository/'target/debug'/binary,base/'bin'/binary)
        address='unix:path='+str(base/'bus')
        bus=subprocess.run(['dbus-daemon','--session','--fork','--nopidfile','--address='+address,'--print-pid=1'],capture_output=True,text=True,check=True)
        pid=int(bus.stdout.strip())
        env=dict(os.environ,DBUS_SESSION_BUS_ADDRESS=address,XDG_RUNTIME_DIR=str(base/'runtime'),XDG_DATA_HOME=str(base/'data'),XDG_CONFIG_HOME=str(base/'config'),XDG_CACHE_HOME=str(base/'cache'),XDG_STATE_HOME=str(base/'state'),HOME=str(base/'home'))
        service=subprocess.Popen([base/'bin'/binary],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        try:
            for _ in range(100):
                owner=subprocess.run(['gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.NameHasOwner',name],env=env,capture_output=True,text=True,check=True)
                if 'true' in owner.stdout and (binary=='nyx-accountsd' or (base/'runtime/nyx-cloud/rclone.sock').exists()):
                    # Introspection waits until the service's interface is exported, after rclone startup for Cloud.
                    try:
                        ready=subprocess.run(['gdbus','introspect','--session','--dest',name,'--object-path','/io/github/design_nexus/nyx/'+('Accounts' if binary=='nyx-accountsd' else 'Cloud1')],env=env,capture_output=True,text=True,timeout=3)
                    except subprocess.TimeoutExpired:
                        time.sleep(.1)
                        continue
                    if ready.returncode==0 and name.replace(".Preview", "1") in ready.stdout:break
                if service.poll() is not None:raise AssertionError(service.stderr.read().decode())
                time.sleep(.03)
            else:raise AssertionError(binary+' was not ready')
            os.kill(pid,signal.SIGTERM);pid=None
            assert service.wait(timeout=10)==0,binary+' did not shut down cleanly'
            print('PASS:',binary,'exits when its private bus closes')
        finally:
            if service.poll() is None:service.terminate();service.wait(timeout=10)
            if pid is not None:os.kill(pid,signal.SIGTERM)
