"""Validate all submitted TTML, without fetching referenced URLs or media."""
from pathlib import Path
from lxml import etree as E
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {"http://www.w3.org/ns/ttml", "http://www.w3.org/ns/ttml#metadata",
           "http://www.w3.org/ns/ttml#parameter", "http://www.w3.org/XML/1998/namespace"}
parser = E.XMLParser(resolve_entities=False, load_dtd=False, no_network=True)
schema = E.XMLSchema(E.parse(str(ROOT / "schemas/w3c-ttml2/ttml2.xsd"), parser))
files = sorted((ROOT / "songs").rglob("*.ttml"))
if not files:
    raise SystemExit("No TTML files found")
for path in files:
    raw = path.read_bytes()
    if len(raw) > 4_000_000 or b"<!DOCTYPE" in raw.upper() or b"<!ENTITY" in raw.upper():
        raise SystemExit(f"{path}: oversized document or prohibited DTD/entity")
    tree = E.ElementTree(E.fromstring(raw, parser))
    for elem in list(tree.iter()):
        if not isinstance(elem.tag, str):
            continue
        if E.QName(elem).namespace not in ALLOWED:
            elem.getparent().remove(elem)
            continue
        for key in list(elem.attrib):
            if key.startswith("{") and E.QName(key).namespace not in ALLOWED:
                del elem.attrib[key]
    schema.assertValid(tree)
    subprocess.run(["cargo", "run", "--locked", "--quiet", "--", str(path)], cwd=ROOT, check=True)
print(f"Validated {len(files)} TTML file(s): W3C XSD plus LyricsLab domain and submission rules")
