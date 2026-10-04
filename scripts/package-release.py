"""Smoke-test, scan and package only public release files on GitHub runners."""
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile

target = sys.argv[1]
windows = target.endswith("windows-msvc")
name = "anki-auto.exe" if windows else "anki-auto"
binary = Path("target") / target / "release" / name
version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
assert subprocess.check_output([str(binary), "--version"], text=True).strip() == f"anki-auto {version}"
# These checks never contact or launch Anki, even with --sync present.
subprocess.run([str(binary), "import", "tests/fixtures/basic.json", "--model", "Basic", "--dry-run", "--sync"], check=True)
subprocess.run([str(binary), "import", "tests/fixtures/single.json", "--dry-run"], check=True)
raw = binary.read_bytes()
for value in [str(Path.cwd()), Path.cwd().as_posix(), str(Path.home()), Path.home().as_posix()]:
    for encoding in ["utf-8", "utf-16-le"]:
        if value.encode(encoding) in raw:
            raise SystemExit("Release contains a build workspace/home path; refusing to package")
for pattern in [rb"/Users/[^\s\x00]+", rb"/home/runner/", rb"[A-Za-z]:\\Users\\", rb"[A-Za-z]:\\a\\anki-auto\\"]:
    if re.search(pattern, raw):
        raise SystemExit("Release contains an absolute user/runner path; refusing to package")
if "linux-musl" in target:
    info = subprocess.check_output(["file", str(binary)], text=True)
    print(info.strip())
    if "static" not in info:
        raise SystemExit("Linux release must be statically linked")
    headers = subprocess.check_output(["readelf", "-l", str(binary)], text=True)
    assert "INTERP" not in headers, "Unexpected dynamic interpreter"
if "apple-darwin" in target:
    libraries = subprocess.check_output(["otool", "-L", str(binary)], text=True)
    print(libraries.strip())
    for line in libraries.splitlines()[1:]:
        library = line.strip().split(" ")[0]
        assert library.startswith(("/usr/lib/", "/System/Library/")), f"Unexpected non-system dependency: {library}"
print("Version, dry runs, linkage and embedded build-path checks passed")
out = Path("dist")
out.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory() as temp:
    staging = Path(temp)
    shutil.copy2(binary, staging / name)
    shutil.copy("LICENSE", staging / "LICENSE")
    shutil.copy("tests/fixtures/basic.json", staging / "basic.json")
    (staging / "INSTALL.txt").write_text(
        "anki-auto " + version + "\n\n"
        "Run ./anki-auto --help (Windows: .\\anki-auto.exe --help).\n"
        "Install Anki and AnkiConnect, then run anki-auto ping.\n"
        "Try: anki-auto import basic.json --deck Geography --model Basic --dry-run\n"
        "Remove --dry-run to import. AnkiWeb sync is optional.\n\n"
        "Full instructions: https://github.com/moshiroi/anki-auto\n"
        "Built on GitHub-hosted runners; debug data stripped and build paths remapped.\n"
    )
    archive = out / (f"anki-auto-{target}" + (".zip" if windows else ".tar.gz"))
    if windows:
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as handle:
            for file in sorted(staging.iterdir()):
                handle.write(file, file.name)
    else:
        with tarfile.open(archive, "w:gz") as handle:
            for file in sorted(staging.iterdir()):
                info = handle.gettarinfo(file, arcname=file.name)
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                info.mtime = 0
                with file.open("rb") as data:
                    handle.addfile(info, data)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{checksum}  {archive.name}\n")
print(f"Packaged {archive.name}")
