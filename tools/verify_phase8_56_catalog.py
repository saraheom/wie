from pathlib import Path
import re

src = Path('wie_wipi_c/src/api/net.rs').read_text(encoding='utf-8')
m = re.search(
    r'const INOTIA1_CASH_CMD30_CATALOG_SINGLE_PAGE_FREE: \[u8; 209\] = \[(.*?)\n\];',
    src,
    re.S,
)
if not m:
    raise SystemExit('Phase 8.56 cash catalog constant not found')

data = bytes(int(x, 16) for x in re.findall(r'0x([0-9a-fA-F]{2})', m.group(1)))
expected_header = bytes([0x00, 0xD1, 0x1E, 0x01, 0x00, 0x01, 0x0B])
if len(data) != 209 or data[:7] != expected_header:
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

expected = [
    ('스킬북', 1, 0),
    ('부활주문서', 1, 0),
    ('축복받은 부활주문서', 1, 0),
    ('상자 열쇠', 1, 0),
    ('축복받은 용사의 인장', 1, 0),
    ('16칸 가방', 1, 0),
    ('스킬 초기화', 1, 0),
    ('무기강화 주문서', 10, 0),
    ('방어구강화 주문서', 10, 0),
    ('힘의 조각', 10, 0),
    ('마법의 가지', 10, 0),
]
if records != expected:
    raise SystemExit(f'Unexpected catalog records: {records}')
if len(records) > 12:
    raise SystemExit(f'Catalog exceeds native capacity: {len(records)}')
if any(value != 0 for _, _, value in records):
    raise SystemExit('Catalog price/value must remain zero')
if any(name == '초보용 용사의 인장' for name, _, _ in records):
    raise SystemExit('Removed beginner seal unexpectedly present')

print('Phase 8.56 cash catalog verified:')
for record in records:
    print(record)
