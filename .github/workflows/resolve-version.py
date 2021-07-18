from argparse import ArgumentParser
import json
import urllib.request
import re

VERSION_REQ = re.compile(r'\A\^([0-9]+)\.([0-9]+)\Z')

def parse_version(s):
    m = VERSION_REQ.match(s)
    if not m:
        raise RuntimeError(f'the version must be `{VERSION_REQ.pattern}`')
    return (int(m.group(1)), int(m.group(2)))

parser = ArgumentParser()
parser.add_argument('package_name')
parser.add_argument('version', type=parse_version)
args = parser.parse_args()
package_name, (version_req_major, version_req_minor) = args.package_name, args.version
with urllib.request.urlopen(f'https://crates.io/api/v1/crates/{package_name}') as res:
    versions = json.load(res)['versions']
matched = set()
for version in versions:
    major, minor, patch_pre_build = version['num'].split('.')
    major, minor = (int(major), int(minor))
    if ((major, version_req_major) == (0, 0) and minor == version_req_minor or major == version_req_major and minor >= version_req_minor) and patch_pre_build.isdecimal():
        matched.add((minor, int(patch_pre_build)))
if not matched:
    raise RuntimeError(f'No such package: `{package_name} ^{version_req_major}.{version_req_minor}`')
minor, patch = max(matched)
print(f'{version_req_major}.{minor}.{patch}')
