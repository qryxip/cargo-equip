import json
import urllib.request

PACKAGE_NAME = 'cargo-udeps'
VERSION_REQ_MAJOR = 0
VERSION_REQ_MINOR = 1

with urllib.request.urlopen(f'https://crates.io/api/v1/crates/{PACKAGE_NAME}') as res:
    versions = json.load(res)['versions']
matched = set()
for version in versions:
    major, minor, patch_pre_build = version['num'].split('.')
    major, minor = (int(major), int(minor))
    if ((major, VERSION_REQ_MAJOR) == (0, 0) and minor == VERSION_REQ_MINOR or major == VERSION_REQ_MAJOR and minor >= VERSION_REQ_MINOR) and patch_pre_build.isdecimal():
        matched.add((minor, int(patch_pre_build)))
if not matched:
    raise RuntimeError(f'No such package: `{PACKAGE_NAME} ^{VERSION_REQ_MAJOR}.{VERSION_REQ_MINOR}`')
minor, patch = max(matched)
print(f'{VERSION_REQ_MAJOR}.{minor}.{patch}')
