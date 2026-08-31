from pathlib import Path
r=Path(__file__).resolve().parents[1]
checks={
'w ie_lgt/src/emulator.rs'.replace(' ', ''):'PHASE8_62_OZ_HANG_PROBE',
'wie_wipi_java/src/classes/org/kwis/msf/io/network.rs':'PHASE8_62_OZ_NETWORK_CONNECT',
'wie_wipi_java/src/classes/org/kwis/msf/io/url.rs':'PHASE8_62_OZ_URL_FIND',
'wie_wipi_c/src/api/net.rs':'PHASE8_62_OZ_MC_NET_CONNECT',
}
for f,t in checks.items():
    assert t in (r/f).read_text(), (f,t)
assert '0.1.62' in (r/'wie_app/tauri.conf.json').read_text()
print('Phase 8.62 verification OK')
