"""Real portal only, isolated addresses/XDG, no host display/system bus or stubs."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile

p = argparse.ArgumentParser()
p.add_argument('--portal', required=True, help='actual xdg-desktop-portal executable')
p.add_argument('--gdbus', default='gdbus')
p.add_argument('--probe', required=True, help='compiled platform_linux_1_probe example')
p.add_argument('--output', required=True)
a = p.parse_args()
output = Path(a.output)
output.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='tg-private-real-portal-') as tmp:
    root = Path(tmp)
    for d in ['config', 'data', 'cache', 'runtime']:
        (root/d).mkdir(mode=0o700)
    config = root/'bus.conf'
    config.write_text(f'<busconfig><type>session</type><listen>unix:tmpdir={tmp}</listen><policy context="default"><allow own="*"/><allow send_destination="*"/><allow receive_sender="*"/></policy></busconfig>')
    bus = subprocess.Popen(['dbus-daemon', f'--config-file={config}', '--nofork', '--print-address=1'], stdout=subprocess.PIPE, text=True)
    try:
        address = bus.stdout.readline().strip()
        assert address.startswith('unix:')
        env = os.environ.copy()
        env.update(DBUS_SESSION_BUS_ADDRESS=address, DBUS_SYSTEM_BUS_ADDRESS='unix:path='+tmp+'/no-system-bus', XDG_CONFIG_HOME=tmp+'/config', XDG_CONFIG_DIRS=tmp+'/config', XDG_DATA_HOME=tmp+'/data', XDG_DATA_DIRS=tmp+'/data', XDG_CACHE_HOME=tmp+'/cache', XDG_RUNTIME_DIR=tmp+'/runtime')
        for name in ['DISPLAY', 'WAYLAND_DISPLAY', 'XDG_CURRENT_DESKTOP']:
            env.pop(name, None)
        with (output/'daemon.log').open('w') as log:
            portal = subprocess.Popen([a.portal, '--verbose'], env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                # D-Bus owner readiness barrier, no sleep/custom event loop.
                subprocess.run([a.gdbus, 'wait', '--session', '--timeout', '5', 'org.freedesktop.portal.Desktop'], env=env, check=True, timeout=7)
                intro = subprocess.run([a.gdbus, 'call', '--session', '--dest', 'org.freedesktop.portal.Desktop', '--object-path', '/org/freedesktop/portal/desktop', '--method', 'org.freedesktop.DBus.Introspectable.Introspect'], env=env, check=True, capture_output=True, text=True, timeout=7)
                (output/'introspection.txt').write_text(intro.stdout)
                probe = subprocess.run([a.probe, address], env=env, check=True, capture_output=True, text=True, timeout=20)
                (output/'calls.txt').write_text(probe.stdout+probe.stderr)
                print(probe.stdout)
            finally:
                portal.terminate()
                portal.wait(timeout=5)
    finally:
        bus.terminate()
        bus.wait(timeout=5)
