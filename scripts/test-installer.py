"""Exercise the real Unix installer against local release files, not user config."""
import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading

target = sys.argv[1]
archive = Path('dist') / f'anki-auto-{target}.tar.gz'
checksum = archive.with_name(archive.name + '.sha256')
class Handler(SimpleHTTPRequestHandler):
    def log_message(self, *_): pass
    def do_GET(self):
        if self.path == '/latest':
            self.send_response(302)
            self.send_header('Location', '/tag/v0.1.0')
            self.end_headers()
        elif self.path == '/tag/v0.1.0':
            self.send_response(200)
            self.end_headers()
        else:
            self.path = '/' + self.path.rsplit('/', 1)[-1]
            super().do_GET()
server = ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Handler, directory=str(Path('dist').resolve())))
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with tempfile.TemporaryDirectory() as temp:
        env = {**os.environ, 'ANKI_AUTO_RELEASE_BASE': f'http://127.0.0.1:{server.server_port}',
               'ANKI_AUTO_INSTALL_DIR': str(Path(temp) / 'install'), 'ANKI_AUTO_NO_MODIFY_PATH': '1'}
        command = ['sh', 'scripts/install.sh']
        subprocess.run(command, env=env, check=True, timeout=30)
        binary = Path(env['ANKI_AUTO_INSTALL_DIR']) / 'anki-auto'
        subprocess.run([str(binary), '--version'], check=True)
        assert (binary.parent / 'basic.json').exists()
        # Tampered downloads must not replace an existing installation.
        previous = binary.read_bytes()
        original = checksum.read_bytes()
        try:
            checksum.write_text('0' * 64 + '  ' + archive.name + '\n')
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=30)
            assert result.returncode != 0 and 'Checksum mismatch' in result.stderr
            assert binary.read_bytes() == previous
        finally:
            checksum.write_bytes(original)
    print('Installer platform selection, download, checksum, version, and tamper rejection passed')
finally:
    server.shutdown()
    server.server_close()
