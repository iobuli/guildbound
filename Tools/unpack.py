"""Extract .unitypackage files into the project without running the editor.

A .unitypackage is a gzip tar holding one folder per asset GUID with `pathname`, `asset` and `asset.meta`.
Code files are skipped: we write our own systems and never ship pack scripts.
Usage: python -I Tools/unpack.py <project_root> <list_file>   (list lines: <id>\t<state>\t<package path>)
"""
import os
import sys
import tarfile

SKIP = ('.cs', '.js', '.dll', '.asmdef', '.asmref', '.rsp', '.pdb', '.mdb', '.so', '.bundle', '.jslib')

def unpack(package, root):
    assets = os.path.realpath(os.path.join(root, 'Assets'))
    written, skipped = [], []
    with tarfile.open(package, 'r:gz') as tar:
        entries = {}
        for member in tar.getmembers():
            parts = member.name.replace('\\', '/').strip('./').split('/')
            if len(parts) == 2 and member.isfile():
                entries.setdefault(parts[0], {})[parts[1]] = member
        for guid, files in entries.items():
            if 'pathname' not in files:
                continue
            path = tar.extractfile(files['pathname']).read().decode('utf-8').splitlines()[0].strip()
            target = os.path.realpath(os.path.join(root, path))
            if not target.startswith(assets + os.sep):
                skipped.append(path + ' (outside Assets)')
                continue
            if path.lower().endswith(SKIP):
                skipped.append(path)
                continue
            if 'asset' in files:
                os.makedirs(os.path.dirname(target), exist_ok=True)
                with open(target, 'wb') as out:
                    out.write(tar.extractfile(files['asset']).read())
            else:
                os.makedirs(target, exist_ok=True)
            if 'asset.meta' in files:
                with open(target + '.meta', 'wb') as out:
                    out.write(tar.extractfile(files['asset.meta']).read())
            written.append(path)
    return written, skipped

def main():
    root, listing = sys.argv[1], sys.argv[2]
    report = []
    for line in open(listing, encoding='utf-8'):
        package = line.rstrip('\n').split('\t')[2]
        written, skipped = unpack(package, root)
        tops = sorted({'/'.join(p.split('/')[:2]) for p in written})
        report.append(f'{os.path.basename(package)}: {len(written)} written, {len(skipped)} skipped -> {", ".join(tops)}')
        report.extend('  skip ' + s for s in skipped)
    os.makedirs(os.path.join(root, 'Logs'), exist_ok=True)
    with open(os.path.join(root, 'Logs', 'unpack.txt'), 'w', encoding='utf-8') as out:
        out.write('\n'.join(report) + '\n')
    print('\n'.join(r for r in report if not r.startswith('  ')))

if __name__ == '__main__':
    main()
