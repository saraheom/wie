from pathlib import Path
root=Path(__file__).resolve().parents[1]
f=(root/'wie_jvm_support/src/runtime/file.rs').read_text()
assert 'MAX_READ_CHUNK: usize = 64 * 1024' in f
assert 'PHASE8_69_OZ_FILE_READ_BEGIN' in f
assert 'PHASE8_69_OZ_FILE_READ_RETURN' in f
assert 'requested={} chunk={}' in f
assert '"version": "0.1.69"' in (root/'wie_app/tauri.conf.json').read_text()
assert (root/'wie_web/public/phase8_69_build.txt').read_text().strip() == 'PHASE8_69_BUILD_SENTINEL=WIPI_PLAYER_0.1.69_OZ_CHUNKED_FILEINPUTSTREAM_READ'
wf=(root/'.github/workflows/ios-testflight.yml').read_text()
assert 'phase8_69_build.txt' in wf
assert 'PHASE8_69_BUILD_SENTINEL=WIPI_PLAYER_0.1.69_OZ_CHUNKED_FILEINPUTSTREAM_READ' in wf
print('phase 8.69 verifier: OK')
