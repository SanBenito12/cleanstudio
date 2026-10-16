import hashlib
import json
import os
import plistlib
import shutil
import struct
import subprocess

ORIG_APP = "/Applications/Maestro Studio.app"
TARGET_APP = "/Users/ilove/Downloads/Clean Studio.app"
EXTRACTED_DIR = "/Users/ilove/.gemini/antigravity-cli/brain/f02969ec-056c-4ec6-a979-b930ebf7289a/scratch/extracted"
PATCHED_DIR = "/Users/ilove/Downloads/clean-studio/patched-studio-files"
TRANSLATIONS_FILE = "/Users/ilove/Downloads/clean-studio/translations_es.json"

os.makedirs(PATCHED_DIR, exist_ok=True)

with open(TRANSLATIONS_FILE, "r", encoding="utf-8") as f:
    trans = json.load(f)

# 1. Patch index-0XZa5ilO.js (only in Maestro Studio sections, preserving Monaco Editor)
with open(os.path.join(EXTRACTED_DIR, "index-0XZa5ilO.js"), "r", encoding="utf-8") as f:
    idx_js = f.read()

part_before = idx_js[:490000]
part_studio_1 = idx_js[490000:865000]
part_monaco = idx_js[865000:4350000]
part_studio_2 = idx_js[4350000:]

for k, v in trans["frontend"].items():
    part_studio_1 = part_studio_1.replace(k, v)
    part_studio_2 = part_studio_2.replace(k, v)

# Disable PostHog telemetry capturing by default
part_studio_1 = part_studio_1.replace(
    "disable_session_recording:!1",
    "disable_session_recording:!0,opt_out_capturing_by_default:!0"
)

patched_idx_js = (part_before + part_studio_1 + part_monaco + part_studio_2).encode("utf-8")
with open(os.path.join(PATCHED_DIR, "index-0XZa5ilO.js"), "wb") as f:
    f.write(patched_idx_js)

# 2. Patch main.js
with open(os.path.join(EXTRACTED_DIR, "main.js"), "r", encoding="utf-8") as f:
    main_js = f.read()

for k, v in trans["main"].items():
    main_js = main_js.replace(k, v)

patched_main_js = main_js.encode("utf-8")
with open(os.path.join(PATCHED_DIR, "main.js"), "wb") as f:
    f.write(patched_main_js)

# 3. Patch index.html
with open(os.path.join(EXTRACTED_DIR, "index.html"), "r", encoding="utf-8") as f:
    idx_html = f.read()
idx_html = idx_html.replace("Maestro Studio - Desktop", "Clean Studio — Automatización E2E en Español")
patched_idx_html = idx_html.encode("utf-8")
with open(os.path.join(PATCHED_DIR, "index.html"), "wb") as f:
    f.write(patched_idx_html)

# 4. Create Clean Studio SVG wordmark
clean_logo_svg = b"""<svg width="595" height="142" viewBox="0 0 595 142" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect x="8" y="16" width="110" height="110" rx="28" fill="url(#cs_grad)"/>
  <path d="M44 72L62 90L94 52" stroke="#020617" stroke-width="12" stroke-linecap="round" stroke-linejoin="round"/>
  <text x="142" y="92" fill="currentColor" font-family="-apple-system, BlinkMacSystemFont, 'SF Pro Display', Inter, sans-serif" font-size="68" font-weight="800" letter-spacing="-1.5">Clean Studio</text>
  <defs>
    <linearGradient id="cs_grad" x1="8" y1="16" x2="118" y2="126" gradientUnits="userSpaceOnUse">
      <stop stop-color="#34D399"/>
      <stop offset="1" stop-color="#38BDF8"/>
    </linearGradient>
  </defs>
</svg>"""
with open(os.path.join(PATCHED_DIR, "Maestro logo _ wordmark-CXPWZs15.svg"), "wb") as f:
    f.write(clean_logo_svg)

def calc_integrity(data: bytes):
    block_size = 4194304
    blocks = []
    for i in range(0, len(data), block_size):
        blocks.append(hashlib.sha256(data[i:i + block_size]).hexdigest())
    if not blocks:
        blocks.append(hashlib.sha256(b"").hexdigest())
    return {
        "algorithm": "SHA256",
        "hash": hashlib.sha256(data).hexdigest(),
        "blockSize": block_size,
        "blocks": blocks,
    }

print("Cloning original Maestro Studio.app to Clean Studio.app...")
if os.path.exists(TARGET_APP):
    shutil.rmtree(TARGET_APP)
subprocess.run(["ditto", ORIG_APP, TARGET_APP], check=True)

orig_asar = os.path.join(ORIG_APP, "Contents/Resources/app.asar")
target_asar = os.path.join(TARGET_APP, "Contents/Resources/app.asar")

with open(orig_asar, "rb") as f:
    header_pickle = f.read(16)
    _, header_size, _, header_str_len = struct.unpack("<4I", header_pickle)
    header_json = json.loads(f.read(header_str_len).decode("utf-8"))
    data_start_pos = 8 + header_size
    f.seek(0, os.SEEK_END)
    orig_file_size = f.tell()
    orig_payload_size = orig_file_size - data_start_pos

current_offset = orig_payload_size
append_chunks = []

def update_asar_entry(entry_dict, new_bytes: bytes):
    global current_offset
    entry_dict["size"] = len(new_bytes)
    entry_dict["offset"] = str(current_offset)
    entry_dict["integrity"] = calc_integrity(new_bytes)
    append_chunks.append(new_bytes)
    current_offset += len(new_bytes)

update_asar_entry(
    header_json["files"]["dist-electron"]["files"]["main.js"],
    patched_main_js,
)
update_asar_entry(
    header_json["files"]["dist"]["files"]["index.html"],
    patched_idx_html,
)
update_asar_entry(
    header_json["files"]["dist"]["files"]["assets"]["files"]["index-0XZa5ilO.js"],
    patched_idx_js,
)
update_asar_entry(
    header_json["files"]["dist"]["files"]["assets"]["files"]["Maestro logo _ wordmark-CXPWZs15.svg"],
    clean_logo_svg,
)

new_header_json_bytes = json.dumps(header_json, separators=(",", ":")).encode("utf-8")
new_header_hash = hashlib.sha256(new_header_json_bytes).hexdigest()
print("New ASAR Header SHA256:", new_header_hash)

str_len = len(new_header_json_bytes)
pad_len = (4 - (str_len % 4)) % 4
aligned_str_len = str_len + pad_len
new_header_size = aligned_str_len + 8
new_pickle = struct.pack("<4I", 4, new_header_size, aligned_str_len + 4, str_len)

with open(orig_asar, "rb") as fin, open(target_asar, "wb") as fout:
    fout.write(new_pickle)
    fout.write(new_header_json_bytes)
    if pad_len:
        fout.write(b"\x00" * pad_len)
    fin.seek(data_start_pos)
    shutil.copyfileobj(fin, fout, length=16 * 1024 * 1024)
    for chunk in append_chunks:
        fout.write(chunk)

# 5. Update Info.plist with new ElectronAsarIntegrity and Clean Studio display name
plist_path = os.path.join(TARGET_APP, "Contents/Info.plist")
with open(plist_path, "rb") as f:
    plist_data = plistlib.load(f)

plist_data["CFBundleDisplayName"] = "Clean Studio"
plist_data["ElectronAsarIntegrity"]["Resources/app.asar"]["hash"] = new_header_hash

with open(plist_path, "wb") as f:
    plistlib.dump(plist_data, f)

print("Signing Clean Studio.app...")
subprocess.run(["codesign", "--force", "--deep", "--sign", "-", TARGET_APP], check=True)
print("Clean Studio.app successfully built and signed!")
