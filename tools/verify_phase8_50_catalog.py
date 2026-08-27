from pathlib import Path
import re

src = Path('wie_wipi_c/src/api/net.rs').read_text(encoding='utf-8')
m = re.search(
    r'const INOTIA1_CASH_CMD30_CATALOG_SINGLE_PAGE_FREE: \[u8; 165\] = \[(.*?)\n\];',
    src,
    re.S,
)
if not m:
    raise SystemExit('Phase 8.50 cash catalog constant not found')

data = bytes(int(x, 16) for x in re.findall(r'0x([0-9a-fA-F]{2})', m.group(1)))
expected_header = bytes([0x00, 0xA5, 0x1E, 0x01, 0x00, 0x01, 0x09])
if len(data) != 165 or data[:7] != expected_header:
    raise SystemExit(f'Unexpected catalog framing len={len(data)} head={data[:7].hex()}')

pos = 7
records = []
for _ in range(data[6]):
    n = data[pos]
    pos += 1
    name = data[pos:pos+n].decode('euc_kr')
    pos += n
    quantity = data[pos]
    pos += 1
    value = int.from_bytes(data[pos:pos+4], 'big')
    pos += 4
    records.append((name, quantity, value))

if pos != len(data):
    raise SystemExit(f'Catalog parse ended at {pos}, len={len(data)}')

expected_names = [
    '스킬북',
    '부활주문서',
    '축복받은 부활주문서',
    '상자 열쇠',
    '축복받은 용사의 인장',
    '16칸 가방',
    '스킬 초기화',
    '힘의 조각',
    '마법의 가지',
]
if [r[0] for r in records] != expected_names:
    raise SystemExit(f'Unexpected catalog names: {records}')

quantities = {name: quantity for name, quantity, _ in records}
if quantities['힘의 조각'] != 10 or quantities['마법의 가지'] != 10:
    raise SystemExit(f'Resource bulk quantity mismatch: {records[-2:]}')
if any(value != 0 for _, _, value in records):
    raise SystemExit('Catalog price/value must remain zero')
if '초보용 용사의 인장' in expected_names:
    raise SystemExit('Removed beginner seal unexpectedly present')

print('Phase 8.50 cash catalog verified:')
for record in records:
    print(record)
