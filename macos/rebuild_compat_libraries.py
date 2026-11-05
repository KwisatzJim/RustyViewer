#!/usr/bin/env python3
"""Rebuild the two pinned Homebrew libraries privately for macOS 27.0."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

PROJECT = Path(__file__).resolve().parents[1]
ROOT = PROJECT / 'target/macos-27-libraries'
PACKAGES = {
    'svt-av1': ('4.3.0', 'https://gitlab.com/AOMediaCodec/SVT-AV1/-/archive/v4.3.0/SVT-AV1-v4.3.0.tar.bz2',
                '9223ce06e6172ad41357ed23b3b00f036e03794b86b0e20ea9ccb3f648b8955d'),
    'libarchive': ('3.8.11', 'https://www.libarchive.org/downloads/libarchive-3.8.11.tar.xz',
                   'a3dc0872b1b39da34dc18ef1b944f0f11ffd37425d70ae04be679279f1cf43d2'),
}


def main():
    cellar = Path(subprocess.check_output(['brew', '--cellar'], text=True).strip())
    env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET='27.0')
    prefix = Path(subprocess.check_output(['brew', '--prefix'], text=True).strip())
    env['PKG_CONFIG_PATH'] = ':'.join(str(prefix / 'opt' / p / 'lib/pkgconfig')
                                    for p in ('libb2', 'lz4', 'xz', 'zstd'))
    dependencies = ('libb2', 'lz4', 'xz', 'zstd')
    env['CPPFLAGS'] = ' '.join(f'-I{prefix}/opt/{p}/include' for p in dependencies)
    env['LDFLAGS'] = ' '.join(f'-L{prefix}/opt/{p}/lib' for p in dependencies)
    ROOT.mkdir(parents=True, exist_ok=True)
    for name, (version, url, expected) in PACKAGES.items():
        installed = cellar / name / version
        if not installed.is_dir():
            raise SystemExit(f'Requires installed provenance: {installed}')
        archive = ROOT / url.rsplit('/', 1)[1]
        if not archive.exists():
            subprocess.run(['curl', '--fail', '--location', '--retry', '2',
                            '--connect-timeout', '30', '--max-time', '600',
                            '--proto', '=https', '--proto-redir', '=https',
                            '--output', str(archive), url], check=True)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != expected:
            raise RuntimeError(f'Checksum mismatch: {archive}')
        source = ROOT / (name + '-source')
        if source.exists():
            shutil.rmtree(source)
        source.mkdir()
        with tarfile.open(archive) as packed:
            packed.extractall(source, filter='data')
        tree, = source.iterdir()
        output = ROOT / name / version
        if output.exists():
            shutil.rmtree(output)
        output.mkdir(parents=True)
        def run(*args):
            subprocess.run(args, cwd=tree, env=env, check=True)
        if name == 'svt-av1':
            run('cmake', '-S', '.', '-B', 'build', '-DCMAKE_BUILD_TYPE=Release',
                '-DCMAKE_OSX_DEPLOYMENT_TARGET=27.0', f'-DCMAKE_INSTALL_PREFIX={output}',
                '-DBUILD_SHARED_LIBS=ON')
            run('cmake', '--build', 'build', '--parallel', '4')
            run('cmake', '--install', 'build')
        else:
            run('./configure', f'--prefix={output}', '--without-lzo2', '--without-nettle',
                '--without-xml2', '--without-openssl', '--with-expat')
            run('make', '-j4')
            run('make', 'install')
        # Retain upstream source pins, recipe and license notices alongside the rebuild.
        for item in installed.iterdir():
            if item.is_file() and (item.name in ('INSTALL_RECEIPT.json', 'sbom.spdx.json') or
                    item.name.upper().startswith(('COPYING', 'LICENSE', 'LICENCE', 'NOTICE', 'AUTHORS', 'COPYRIGHT'))):
                shutil.copy2(item, output / item.name)
        shutil.copytree(installed / '.brew', output / '.brew')
        (output / 'RUSTYVIEWER_BUILD.json').write_text(json.dumps({
            'deployment_target': '27.0', 'source_url': url, 'source_sha256': expected,
            'note': 'Private source rebuild; Homebrew receipt describes source bottle provenance only.',
        }, indent=2) + '\n')
        print(f'Rebuilt {name} {version} for macOS 27.0', flush=True)


if __name__ == '__main__':
    main()
